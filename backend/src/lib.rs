pub mod alarm_bridge;
pub mod api;
pub mod config;
pub mod db;
pub mod efficiency_analyzer;
pub mod metrics;
pub mod models;
pub mod optimization_engine;
pub mod profinet_driver;

pub use crate::alarm_bridge::{AlarmBridge, AlertStateSummary, OpcUaConnectionStatus};
pub use crate::config::AppConfig;
pub use crate::db::Database;
pub use crate::efficiency_analyzer::{
    EfficiencyAnalyzer, EfficiencyModel, EfficiencyResult, OptimizationEngineHandle,
    OptimizationResultMessage, OptimizationTask,
};
pub use crate::metrics::{init_metrics, MetricsTimer};
pub use crate::models::*;
pub use crate::optimization_engine::{GeneticAlgorithmOptimizer, OptimizationEngine};
pub use crate::profinet_driver::{ProfinetDriver, SensorDataBatch};
