use crate::config::ProfinetConfig;
use crate::db::Database;
use crate::models::*;
use byteorder::{BigEndian, ReadBytesExt};
use chrono::{DateTime, TimeZone, Utc};
use crc::{Crc, CRC_32_ISCSI};
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use std::time::Instant;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

pub const PROFINET_CRC: Crc<u32> = Crc::<u32>::new(&CRC_32_ISCSI);
const PACKET_HEADER_SIZE: usize = 8;
const CRC_SIZE: usize = 4;

#[derive(Debug, Clone)]
pub struct SensorDataBatch {
    pub electrolyzer_id: u8,
    pub timestamp: DateTime<Utc>,
    pub sensors: Vec<SensorData>,
    pub avg_voltage: f64,
    pub avg_current_density: f64,
    pub avg_hydrogen_flow: f64,
    pub avg_water_temp: f64,
    pub avg_hydrogen_purity: f64,
    pub avg_membrane_conductivity: f64,
    pub cell_voltages: Vec<f64>,
}

#[derive(Debug, Clone)]
pub struct ProfinetDriver {
    config: ProfinetConfig,
    db: Database,
    data_tx: mpsc::Sender<SensorDataBatch>,
}

impl ProfinetDriver {
    pub fn new(
        config: ProfinetConfig,
        db: Database,
    ) -> (Self, mpsc::Receiver<SensorDataBatch>) {
        let (data_tx, data_rx) = mpsc::channel(config.channel_capacity);
        (
            Self {
                config,
                db,
                data_tx,
            },
            data_rx,
        )
    }

    pub async fn run(&self, port: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr = format!("0.0.0.0:{}", port);
        let socket = Arc::new(UdpSocket::bind(&addr).await?);
        log::info!("Profinet driver listening on {}", addr);

        let mut buf = vec![0u8; 65535];
        let mut invalid_packet_count = 0u64;
        let mut last_invalid_log = Instant::now();

        loop {
            let (len, src) = socket.recv_from(&mut buf).await?;
            
            if len < self.config.min_packet_size {
                invalid_packet_count += 1;
                if last_invalid_log.elapsed().as_secs() >= 60 {
                    log::warn!(
                        "Profinet packet too short from {}: {} bytes (min {}), total invalid: {}",
                        src, len, self.config.min_packet_size, invalid_packet_count
                    );
                    last_invalid_log = Instant::now();
                }
                continue;
            }

            let packet_data = &buf[..len];
            
            match self.parse_packet(packet_data).await {
                Ok(batch) => {
                    let db = self.db.clone();
                    let sensors = batch.sensors.clone();
                    
                    tokio::spawn(async move {
                        if let Err(e) = db.insert_sensor_data(&sensors).await {
                            log::error!("Failed to insert sensor data: {}", e);
                        }
                    });

                    if let Err(e) = self.data_tx.send(batch).await {
                        log::error!("Failed to send data batch: {}", e);
                    }
                }
                Err(e) => {
                    invalid_packet_count += 1;
                    
                    log::error!(
                        "⚠️  Profinet packet parse error from {} ({} bytes): {}\n\
                         Packet hex dump (first 64 bytes): {}",
                        src, len, e,
                        packet_data.iter()
                            .take(64)
                            .map(|b| format!("{:02X}", b))
                            .collect::<Vec<_>>()
                            .join(" ")
                    );
                    
                    if last_invalid_log.elapsed().as_secs() >= 60 {
                        log::warn!("Total invalid Profinet packets in last minute: {}", invalid_packet_count);
                        invalid_packet_count = 0;
                        last_invalid_log = Instant::now();
                    }
                }
            }
        }
    }

    async fn parse_packet(
        &self,
        data: &[u8],
    ) -> Result<SensorDataBatch, Box<dyn std::error::Error + Send + Sync>> {
        let mut cursor = Cursor::new(data);
        
        let magic = cursor.read_u32::<BigEndian>()?;
        
        if magic != self.config.packet_magic {
            return Err(format!(
                "Invalid magic number: 0x{:08X}, expected: 0x{:08X} (PRON)",
                magic, self.config.packet_magic
            )
            .into());
        }
        
        let payload_len = cursor.read_u32::<BigEndian>()? as usize;
        
        let expected_total_len = PACKET_HEADER_SIZE + payload_len + CRC_SIZE;
        if data.len() != expected_total_len {
            return Err(format!(
                "Packet length mismatch: actual {} bytes, expected {} bytes (header: {} + payload: {} + crc: {})",
                data.len(), expected_total_len, PACKET_HEADER_SIZE, payload_len, CRC_SIZE
            )
            .into());
        }
        
        let payload_start = cursor.position() as usize;
        let payload_end = payload_start + payload_len;
        
        if payload_end + CRC_SIZE > data.len() {
            return Err(format!(
                "Payload overflow: payload_end={}, crc_end={}, data_len={}",
                payload_end, payload_end + CRC_SIZE, data.len()
            )
            .into());
        }
        
        let payload = &data[payload_start..payload_end];
        
        let crc_start = payload_end;
        let crc_bytes = &data[crc_start..crc_start + CRC_SIZE];
        let mut crc_cursor = Cursor::new(crc_bytes);
        let received_crc = crc_cursor.read_u32::<BigEndian>()?;
        
        let calculated_crc = PROFINET_CRC.checksum(payload);
        if calculated_crc != received_crc {
            return Err(format!(
                "CRC check failed: received 0x{:08X}, calculated 0x{:08X}",
                received_crc, calculated_crc
            )
            .into());
        }
        
        let packet: ProfinetPacket = serde_json::from_slice(payload).map_err(|e| {
            format!(
                "JSON parse error (payload len: {}): {}, first 128 chars: {}",
                payload_len,
                e,
                std::str::from_utf8(&payload[..payload_len.min(128)]).unwrap_or("<invalid utf8>")
            )
        })?;
        
        let timestamp = Utc
            .timestamp_opt(
                packet.timestamp as i64,
                ((packet.timestamp % 1.0) * 1e9) as u32,
            )
            .single()
            .unwrap_or_else(|| Utc::now());

        self.clean_and_aggregate_sensors(packet, timestamp)
    }

    fn clean_and_aggregate_sensors(
        &self,
        packet: ProfinetPacket,
        timestamp: DateTime<Utc>,
    ) -> Result<SensorDataBatch, Box<dyn std::error::Error + Send + Sync>> {
        let mut voltage_sum = 0.0;
        let mut current_density_sum = 0.0;
        let mut hydrogen_flow_sum = 0.0;
        let mut water_temp_sum = 0.0;
        let mut hydrogen_purity_sum = 0.0;
        let mut membrane_conductivity_sum = 0.0;
        let mut count = 0;
        let mut cell_voltages = Vec::new();
        let mut voltage_count = 0;
        let mut cd_count = 0;
        let mut h2_count = 0;
        let mut temp_count = 0;
        let mut purity_count = 0;
        let mut cond_count = 0;

        let mut sensor_data_map: HashMap<u16, SensorData> = HashMap::new();

        for reading in &packet.sensors {
            let sensor_type = match reading.sensor_type.as_str() {
                "voltage" => SensorType::Voltage,
                "current_density" => SensorType::CurrentDensity,
                "hydrogen_flow" => SensorType::HydrogenFlow,
                "oxygen_flow" => SensorType::OxygenFlow,
                "water_temp" => SensorType::WaterTemp,
                "membrane_conductivity" => SensorType::MembraneConductivity,
                "hydrogen_purity" => SensorType::HydrogenPurity,
                "cell_voltage" => SensorType::CellVoltage,
                _ => continue,
            };

            let location = match reading.location.as_str() {
                "anode" => Location::Anode,
                "cathode" => Location::Cathode,
                "membrane" => Location::Membrane,
                _ => continue,
            };

            if !Self::validate_sensor_value(sensor_type, reading.value) {
                log::warn!(
                    "Sensor value out of valid range: sensor_id={}, type={:?}, value={}",
                    reading.sensor_id, sensor_type, reading.value
                );
                continue;
            }

            match sensor_type {
                SensorType::Voltage | SensorType::CellVoltage => {
                    voltage_sum += reading.value;
                    voltage_count += 1;
                    if matches!(sensor_type, SensorType::CellVoltage) {
                        cell_voltages.push(reading.value);
                    }
                }
                SensorType::CurrentDensity => {
                    current_density_sum += reading.value;
                    cd_count += 1;
                }
                SensorType::HydrogenFlow => {
                    hydrogen_flow_sum += reading.value;
                    h2_count += 1;
                }
                SensorType::WaterTemp => {
                    water_temp_sum += reading.value;
                    temp_count += 1;
                }
                SensorType::HydrogenPurity => {
                    hydrogen_purity_sum += reading.value;
                    purity_count += 1;
                }
                SensorType::MembraneConductivity => {
                    membrane_conductivity_sum += reading.value;
                    cond_count += 1;
                }
                _ => {}
            }
            count += 1;

            let sensor_data = SensorData {
                timestamp,
                electrolyzer_id: packet.electrolyzer_id,
                sensor_id: reading.sensor_id,
                sensor_type,
                location,
                value: reading.value,
                rated_value: reading.rated_value,
                x: reading.x,
                y: reading.y,
            };

            sensor_data_map.insert(reading.sensor_id, sensor_data);
        }

        let sensors: Vec<SensorData> = sensor_data_map.into_values().collect();

        let avg_voltage = if voltage_count > 0 {
            voltage_sum / voltage_count as f64
        } else {
            0.0
        };
        let avg_current_density = if cd_count > 0 {
            current_density_sum / cd_count as f64
        } else {
            0.0
        };
        let avg_hydrogen_flow = if h2_count > 0 {
            hydrogen_flow_sum / h2_count as f64
        } else {
            0.0
        };
        let avg_water_temp = if temp_count > 0 {
            water_temp_sum / temp_count as f64
        } else {
            0.0
        };
        let avg_hydrogen_purity = if purity_count > 0 {
            hydrogen_purity_sum / purity_count as f64
        } else {
            0.0
        };
        let avg_membrane_conductivity = if cond_count > 0 {
            membrane_conductivity_sum / cond_count as f64
        } else {
            0.0
        };

        if count == 0 {
            return Err("No valid sensor readings in packet".into());
        }

        Ok(SensorDataBatch {
            electrolyzer_id: packet.electrolyzer_id,
            timestamp,
            sensors,
            avg_voltage,
            avg_current_density,
            avg_hydrogen_flow,
            avg_water_temp,
            avg_hydrogen_purity,
            avg_membrane_conductivity,
            cell_voltages,
        })
    }

    fn validate_sensor_value(sensor_type: SensorType, value: f64) -> bool {
        match sensor_type {
            SensorType::Voltage | SensorType::CellVoltage => value > 0.0 && value < 5.0,
            SensorType::CurrentDensity => value >= 0.0 && value <= 10.0,
            SensorType::HydrogenFlow | SensorType::OxygenFlow => value >= 0.0 && value < 1000.0,
            SensorType::WaterTemp => value >= 0.0 && value <= 100.0,
            SensorType::MembraneConductivity => value > 0.0 && value < 1.0,
            SensorType::HydrogenPurity => value >= 0.0 && value <= 100.0,
            _ => value.is_finite(),
        }
    }
}
