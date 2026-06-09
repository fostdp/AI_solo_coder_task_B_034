pub mod alarm_bridge;
pub mod api;
pub mod config;
pub mod db;
pub mod degradation_predictor;
pub mod efficiency_analyzer;
pub mod hydrogen_leak_detector;
pub mod mea_diagnostics;
pub mod metrics;
pub mod models;
pub mod optimization_engine;
pub mod profinet_driver;
pub mod renewable_coupler;

pub use crate::alarm_bridge::{AlarmBridge, AlertStateSummary, OpcUaConnectionStatus};
pub use crate::config::AppConfig;
pub use crate::db::Database;
pub use crate::degradation_predictor::{
    DegradationPredictionRequest, DegradationPredictor,
};
pub use crate::efficiency_analyzer::{
    EfficiencyAnalyzer, EfficiencyModel, EfficiencyResult, OptimizationEngineHandle,
    OptimizationResultMessage, OptimizationTask,
};
pub use crate::hydrogen_leak_detector::{HydrogenLeakDetector, LeakDetectionRequest};
pub use crate::mea_diagnostics::{MEADiagnosticRequest, MEADiagnostics};
pub use crate::metrics::{init_metrics, MetricsTimer};
pub use crate::models::*;
pub use crate::optimization_engine::{GeneticAlgorithmOptimizer, OptimizationEngine};
pub use crate::profinet_driver::{ProfinetDriver, SensorDataBatch};
pub use crate::renewable_coupler::{RenewableCoupler, RenewableCouplingRequest};
