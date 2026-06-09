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
