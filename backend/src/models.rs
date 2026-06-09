use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    UltrasonicSensor,
    AcousticEmission,
    SolarIrradiance,
    WindSpeed,
    RenewablePower,
    StepResponseVoltage,
    HighFreqImpedance,
    LowFreqImpedance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Location {
    Anode,
    Cathode,
    Membrane,
    InletManifold,
    OutletManifold,
    SealingGasket,
    EndPlate,
    BusBar,
    CoolingChannel,
    GasDiffusionLayer,
    CatalystLayer,
    MembraneElectrodeAssembly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DegradationMode {
    MembraneDryout,
    CatalystPoisoning,
    ContactResistanceIncrease,
    Normal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DegradationSeverity {
    Low,
    Medium,
    High,
    Critical,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EISDataPoint {
    pub frequency: f64,
    pub real_impedance: f64,
    pub imag_impedance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquivalentCircuitParams {
    pub ohmic_resistance: f64,
    pub charge_transfer_resistance: f64,
    pub double_layer_capacitance: f64,
    pub warburg_coefficient: f64,
    pub fit_error: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResponseData {
    pub time_points: Vec<f64>,
    pub voltage_points: Vec<f64>,
    pub current_step: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticIcon {
    pub x: f64,
    pub y: f64,
    pub degradation_mode: DegradationMode,
    pub severity: DegradationSeverity,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MEADiagnosticResult {
    pub electrolyzer_id: u8,
    pub timestamp: DateTime<Utc>,
    pub equivalent_circuit: EquivalentCircuitParams,
    pub degradation_mode: DegradationMode,
    pub severity: DegradationSeverity,
    pub confidence: f64,
    pub membrane_conductivity_trend: f64,
    pub step_response_overshoot: f64,
    pub step_response_settling_time: f64,
    pub recommendations: Vec<String>,
    pub icons: Vec<DiagnosticIcon>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcousticEmissionData {
    pub sensor_id: u16,
    pub timestamp: DateTime<Utc>,
    pub signal: Vec<f64>,
    pub sampling_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralFeatures {
    pub rms: f64,
    pub peak_frequency: f64,
    pub spectral_centroid: f64,
    pub kurtosis: f64,
    pub peak_amplitude: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeakLocation {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub uncertainty: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HydrogenLeak {
    pub id: Uuid,
    pub electrolyzer_id: u8,
    pub timestamp: DateTime<Utc>,
    pub location: LeakLocation,
    pub leak_rate: f64,
    pub diffusion_radius: f64,
    pub severity: DegradationSeverity,
    pub spectral_features: SpectralFeatures,
    pub acknowledged: bool,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeakAnimation {
    pub leak_id: Uuid,
    pub x: f64,
    pub y: f64,
    pub max_radius: f64,
    pub leak_rate: f64,
    pub severity: DegradationSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenewablePowerData {
    pub timestamp: DateTime<Utc>,
    pub solar_power: f64,
    pub wind_power: f64,
    pub total_power: f64,
    pub grid_power: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MPCState {
    pub timestamp: DateTime<Utc>,
    pub electrolyzer_id: u8,
    pub target_power: f64,
    pub actual_power: f64,
    pub current_density: f64,
    pub tracking_error: f64,
    pub control_signal: f64,
    pub start_stop_count: u32,
    pub operating_hours: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenewableCouplingStatus {
    pub electrolyzer_id: u8,
    pub timestamp: DateTime<Utc>,
    pub renewable_utilization: f64,
    pub grid_supplementation: f64,
    pub tracking_accuracy: f64,
    pub start_stop_count: u32,
    pub is_tracking: bool,
    pub predicted_power: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoltageCurrentPoint {
    pub timestamp: DateTime<Utc>,
    pub current_density: f64,
    pub cell_voltage: f64,
    pub efficiency: f64,
    pub temperature: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DegradationFeature {
    pub voltage_increase_rate: f64,
    pub efficiency_decay_rate: f64,
    pub resistance_increase_rate: f64,
    pub performance_index: f64,
    pub cumulative_operating_hours: f64,
    pub total_charge: f64,
    pub temperature_cycling_count: u32,
    pub max_power_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GPPredictionPoint {
    pub days_ahead: u32,
    pub predicted_voltage: f64,
    pub lower_bound: f64,
    pub upper_bound: f64,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DegradationPrediction {
    pub electrolyzer_id: u8,
    pub timestamp: DateTime<Utc>,
    pub features: DegradationFeature,
    pub predictions: Vec<GPPredictionPoint>,
    pub remaining_useful_life: f64,
    pub rul_lower_bound: f64,
    pub rul_upper_bound: f64,
    pub current_degradation_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenancePlanItem {
    pub electrolyzer_id: u8,
    pub priority: DegradationSeverity,
    pub predicted_failure_date: DateTime<Utc>,
    pub remaining_useful_life: f64,
    pub recommended_maintenance_date: DateTime<Utc>,
    pub estimated_cost: f64,
    pub maintenance_type: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenancePlan {
    pub timestamp: DateTime<Utc>,
    pub items: Vec<MaintenancePlanItem>,
    pub total_estimated_cost: f64,
}
