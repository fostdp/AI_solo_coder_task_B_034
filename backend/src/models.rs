use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensorType {
    Voltage,
    CurrentDensity,
    HydrogenFlow,
    OxygenFlow,
    WaterTemp,
    MembraneConductivity,
    HydrogenPurity,
    CellVoltage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Location {
    Anode,
    Cathode,
    Membrane,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorData {
    pub timestamp: DateTime<Utc>,
    pub electrolyzer_id: u8,
    pub sensor_id: u16,
    pub sensor_type: SensorType,
    pub location: Location,
    pub value: f64,
    pub rated_value: f64,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfinetPacket {
    pub timestamp: f64,
    pub electrolyzer_id: u8,
    pub sensors: Vec<SensorReading>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorReading {
    pub sensor_id: u16,
    pub sensor_type: String,
    pub location: String,
    pub value: f64,
    pub rated_value: f64,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectrolyzerStatus {
    pub timestamp: DateTime<Utc>,
    pub electrolyzer_id: u8,
    pub total_hydrogen_production: f64,
    pub average_efficiency: f64,
    pub total_power_consumption: f64,
    pub cell_voltage: Vec<f64>,
    pub current_density: f64,
    pub water_temp: f64,
    pub hydrogen_purity: f64,
    pub membrane_conductivity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertLevel {
    Level1,
    Level2,
    Level3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub electrolyzer_id: u8,
    pub alert_level: AlertLevel,
    pub alert_type: String,
    pub message: String,
    pub value: f64,
    pub threshold: f64,
    pub acknowledged: bool,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationSuggestion {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub electrolyzer_id: u8,
    pub current_efficiency: f64,
    pub optimized_current_density: f64,
    pub optimized_water_temp: f64,
    pub expected_efficiency: f64,
    pub applied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EfficiencyHistory {
    pub timestamp: DateTime<Utc>,
    pub electrolyzer_id: u8,
    pub current_density: f64,
    pub cell_voltage: f64,
    pub efficiency: f64,
    pub water_temp: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSummary {
    pub timestamp: DateTime<Utc>,
    pub total_hydrogen: f64,
    pub avg_efficiency: f64,
    pub total_power: f64,
    pub active_electrolyzers: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorTrendData {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorDetail {
    pub sensor_id: u16,
    pub sensor_type: String,
    pub location: String,
    pub current_value: f64,
    pub rated_value: f64,
    pub deviation_percent: f64,
    pub x: f64,
    pub y: f64,
    pub trend_data: Vec<SensorTrendData>,
    pub efficiency_data: Vec<SensorTrendData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectrolyzerDetail {
    pub id: u8,
    pub status: String,
    pub current_density: f64,
    pub water_temp: f64,
    pub efficiency: f64,
    pub hydrogen_purity: f64,
    pub membrane_conductivity: f64,
    pub sensors: Vec<SensorDetail>,
    pub alerts: Vec<Alert>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub profinet_port: u16,
    pub api_port: u16,
    pub clickhouse_url: String,
    pub clickhouse_user: String,
    pub clickhouse_password: String,
    pub clickhouse_database: String,
    pub opcua_server_url: String,
    pub electrolyzer_count: u8,
    pub sensors_per_electrolyzer: u16,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            profinet_port: 34567,
            api_port: 8080,
            clickhouse_url: "http://localhost:8123".to_string(),
            clickhouse_user: "default".to_string(),
            clickhouse_password: "".to_string(),
            clickhouse_database: "pem_electrolyzer".to_string(),
            opcua_server_url: "opc.tcp://localhost:4840".to_string(),
            electrolyzer_count: 10,
            sensors_per_electrolyzer: 50,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationParams {
    pub current_density: f64,
    pub water_temp: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationResult {
    pub params: OptimizationParams,
    pub expected_efficiency: f64,
    pub generations: u32,
    pub fitness: f64,
}
