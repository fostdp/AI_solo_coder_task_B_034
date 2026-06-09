use crate::db::Database;
use crate::models::*;
use byteorder::{BigEndian, ReadBytesExt};
use chrono::{DateTime, TimeZone, Utc};
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct ProfinetReceiver {
    port: u16,
    db: Database,
    data_tx: mpsc::Sender<SensorDataBatch>,
}

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

impl ProfinetReceiver {
    pub fn new(port: u16, db: Database) -> (Self, mpsc::Receiver<SensorDataBatch>) {
        let (data_tx, data_rx) = mpsc::channel(1000);
        (
            Self {
                port,
                db,
                data_tx,
            },
            data_rx,
        )
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr = format!("0.0.0.0:{}", self.port);
        let socket = Arc::new(UdpSocket::bind(&addr).await?);
        log::info!("Profinet receiver listening on {}", addr);

        let mut buf = vec![0u8; 65535];

        loop {
            let (len, _src) = socket.recv_from(&mut buf).await?;
            
            if len < 8 {
                continue;
            }

            match self.parse_packet(&buf[..len]).await {
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
                    log::warn!("Failed to parse Profinet packet: {}", e);
                }
            }
        }
    }

    async fn parse_packet(
        &self,
        data: &[u8],
    ) -> Result<SensorDataBatch, Box<dyn std::error::Error + Send + Sync>> {
        let mut cursor = Cursor::new(data);
        
        let _magic = cursor.read_u32::<BigEndian>()?;
        let payload_len = cursor.read_u32::<BigEndian>()? as usize;
        
        let payload_start = cursor.position() as usize;
        let payload_end = payload_start + payload_len;
        
        if payload_end > data.len() {
            return Err("Invalid packet length".into());
        }
        
        let payload = &data[payload_start..payload_end];
        let packet: ProfinetPacket = serde_json::from_slice(payload)?;
        
        let timestamp = Utc
            .timestamp_opt(
                packet.timestamp as i64,
                ((packet.timestamp % 1.0) * 1e9) as u32,
            )
            .single()
            .unwrap_or_else(|| Utc::now());

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
        let cond_count = &mut 0;

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
                    *cond_count += 1;
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
        let avg_membrane_conductivity = if *cond_count > 0 {
            membrane_conductivity_sum / *cond_count as f64
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
}
