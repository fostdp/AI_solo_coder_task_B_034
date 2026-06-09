use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub system: SystemConfig,
    pub opcua: OpcUaConfig,
    pub alerts: AlertConfig,
    pub efficiency_model: EfficiencyModelConfig,
    pub optimization: OptimizationConfig,
    pub genetic_algorithm: GeneticAlgorithmConfig,
    pub profinet: ProfinetConfig,
    pub mea_diagnostics: MeaDiagnosticsConfig,
    pub leak_detection: LeakDetectionConfig,
    pub renewable_coupling: RenewableCouplingConfig,
    pub degradation_prediction: DegradationPredictionConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MeaDiagnosticsConfig {
    pub eis_fit_max_iterations: u32,
    pub eis_fit_tolerance: f64,
    pub membrane_dryout_threshold: f64,
    pub catalyst_poisoning_threshold: f64,
    pub contact_resistance_threshold: f64,
    pub conductivity_trend_window: u32,
    pub min_confidence: f64,
    pub diagnosis_interval_secs: u64,
    pub max_concurrent_diagnoses: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LeakDetectionConfig {
    pub ultrasound_frequency_min: f64,
    pub ultrasound_frequency_max: f64,
    pub leak_threshold_rms: f64,
    pub leak_threshold_peak: f64,
    pub sound_speed_hydrogen: f64,
    pub diffusion_coefficient: f64,
    pub trilateration_sensor_count: usize,
    pub min_leak_rate: f64,
    pub detection_interval_secs: u64,
    pub max_concurrent_detections: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RenewableCouplingConfig {
    pub mpc_horizon: u32,
    pub mpc_control_weight: f64,
    pub mpc_rate_weight: f64,
    pub min_operation_time_secs: u64,
    pub deadzone_percentage: f64,
    pub power_ramp_rate_per_sec: f64,
    pub prediction_horizon_secs: u64,
    pub control_interval_secs: u64,
    pub max_power_kw: f64,
    pub min_power_kw: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DegradationPredictionConfig {
    pub gp_length_scale: f64,
    pub gp_signal_variance: f64,
    pub gp_noise_variance: f64,
    pub prediction_days: u32,
    pub confidence_level: f64,
    pub min_history_points: usize,
    pub voltage_failure_threshold: f64,
    pub efficiency_failure_threshold: f64,
    pub prediction_interval_secs: u64,
    pub max_concurrent_predictions: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub profinet_port: u16,
    pub api_port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub clickhouse_url: String,
    pub clickhouse_user: String,
    pub clickhouse_password: String,
    pub clickhouse_database: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SystemConfig {
    pub electrolyzer_count: u8,
    pub sensors_per_electrolyzer: u16,
    pub active_area: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpcUaConfig {
    pub server_url: String,
    pub heartbeat_interval_secs: u64,
    pub heartbeat_timeout_secs: u64,
    pub initial_reconnect_delay_ms: u64,
    pub max_reconnect_delay_ms: u64,
    pub max_reconnect_attempts: u32,
    pub alert_queue_capacity: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlertConfig {
    pub voltage_threshold: f64,
    pub voltage_duration_seconds: i64,
    pub purity_threshold: f64,
    pub purity_duration_seconds: i64,
    pub conductivity_degradation_threshold: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EfficiencyModelConfig {
    pub a: f64,
    pub b: f64,
    pub r: f64,
    pub exchange_current_density: f64,
    pub transfer_coefficient: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OptimizationConfig {
    pub min_current_density: f64,
    pub max_current_density: f64,
    pub min_temp: f64,
    pub max_temp: f64,
    pub efficiency_threshold: f64,
    pub target_efficiency: f64,
    pub max_concurrent_optimizations: usize,
    pub queue_capacity: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeneticAlgorithmConfig {
    pub population_size: usize,
    pub mutation_rate: f64,
    pub crossover_rate: f64,
    pub max_generations: u32,
    pub elitism_count: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProfinetConfig {
    pub packet_magic: u32,
    pub min_packet_size: usize,
    pub channel_capacity: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                profinet_port: 34567,
                api_port: 8080,
            },
            database: DatabaseConfig {
                clickhouse_url: "http://localhost:8123".to_string(),
                clickhouse_user: "default".to_string(),
                clickhouse_password: "".to_string(),
                clickhouse_database: "pem_electrolyzer".to_string(),
            },
            system: SystemConfig {
                electrolyzer_count: 10,
                sensors_per_electrolyzer: 50,
                active_area: 1.0,
            },
            opcua: OpcUaConfig {
                server_url: "opc.tcp://localhost:4840".to_string(),
                heartbeat_interval_secs: 5,
                heartbeat_timeout_secs: 15,
                initial_reconnect_delay_ms: 1000,
                max_reconnect_delay_ms: 60000,
                max_reconnect_attempts: 0,
                alert_queue_capacity: 1000,
            },
            alerts: AlertConfig {
                voltage_threshold: 2.0,
                voltage_duration_seconds: 300,
                purity_threshold: 99.9,
                purity_duration_seconds: 180,
                conductivity_degradation_threshold: 20.0,
            },
            efficiency_model: EfficiencyModelConfig {
                a: 0.05,
                b: 0.03,
                r: 0.08,
                exchange_current_density: 0.001,
                transfer_coefficient: 0.5,
            },
            optimization: OptimizationConfig {
                min_current_density: 0.5,
                max_current_density: 4.0,
                min_temp: 40.0,
                max_temp: 80.0,
                efficiency_threshold: 75.0,
                target_efficiency: 78.0,
                max_concurrent_optimizations: 3,
                queue_capacity: 100,
            },
            genetic_algorithm: GeneticAlgorithmConfig {
                population_size: 100,
                mutation_rate: 0.1,
                crossover_rate: 0.8,
                max_generations: 100,
                elitism_count: 5,
            },
            profinet: ProfinetConfig {
                packet_magic: 0x50524F4E,
                min_packet_size: 12,
                channel_capacity: 1000,
            },
            mea_diagnostics: MeaDiagnosticsConfig {
                eis_fit_max_iterations: 100,
                eis_fit_tolerance: 1e-6,
                membrane_dryout_threshold: 0.15,
                catalyst_poisoning_threshold: 0.2,
                contact_resistance_threshold: 0.1,
                conductivity_trend_window: 100,
                min_confidence: 0.7,
                diagnosis_interval_secs: 300,
                max_concurrent_diagnoses: 3,
            },
            leak_detection: LeakDetectionConfig {
                ultrasound_frequency_min: 30000.0,
                ultrasound_frequency_max: 80000.0,
                leak_threshold_rms: 0.01,
                leak_threshold_peak: 0.05,
                sound_speed_hydrogen: 1310.0,
                diffusion_coefficient: 0.61e-4,
                trilateration_sensor_count: 4,
                min_leak_rate: 0.001,
                detection_interval_secs: 10,
                max_concurrent_detections: 5,
            },
            renewable_coupling: RenewableCouplingConfig {
                mpc_horizon: 10,
                mpc_control_weight: 1.0,
                mpc_rate_weight: 0.1,
                min_operation_time_secs: 1800,
                deadzone_percentage: 5.0,
                power_ramp_rate_per_sec: 0.01,
                prediction_horizon_secs: 300,
                control_interval_secs: 5,
                max_power_kw: 100.0,
                min_power_kw: 10.0,
            },
            degradation_prediction: DegradationPredictionConfig {
                gp_length_scale: 30.0,
                gp_signal_variance: 0.01,
                gp_noise_variance: 1e-4,
                prediction_days: 90,
                confidence_level: 0.95,
                min_history_points: 30,
                voltage_failure_threshold: 2.2,
                efficiency_failure_threshold: 65.0,
                prediction_interval_secs: 3600,
                max_concurrent_predictions: 2,
            },
        }
    }
}

impl AppConfig {
    pub fn load_from_file(path: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let config_path = Path::new(path);
        if !config_path.exists() {
            log::warn!("Config file not found at {}, using defaults", path);
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(config_path)?;
        let config: AppConfig = toml::from_str(&content)?;
        log::info!("Configuration loaded from {}", path);
        Ok(config)
    }

    pub fn load() -> Self {
        match Self::load_from_file("config.toml") {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Failed to load config: {}, using defaults", e);
                Self::default()
            }
        }
    }
}
