pub mod alarm_bridge;
pub mod api;
pub mod config;
pub mod db;
pub mod degradation_predictor;
pub mod efficiency_analyzer;
pub mod gp_inference_service;
pub mod hydrogen_leak_detector;
pub mod leak_detector;
pub mod mea_diagnoser;
pub mod mea_diagnostics;
pub mod metrics;
pub mod models;
pub mod optimization_engine;
pub mod profinet_driver;
pub mod renewable_coupler;
pub mod renewable_integrator;

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
pub use crate::gp_inference_service::{GPInferenceHandle, GPInferenceRequest, GPInferenceService};
pub use crate::hydrogen_leak_detector::{HydrogenLeakDetector, LeakDetectionRequest};
pub use crate::leak_detector::{LeakDetectionRequest, LeakDetector};
pub use crate::mea_diagnoser::{MEADiagnoser, MEADiagnosticRequest};
pub use crate::mea_diagnostics::{MEADiagnosticRequest, MEADiagnostics};
pub use crate::metrics::{init_metrics, MetricsTimer};
pub use crate::models::*;
pub use crate::optimization_engine::{GeneticAlgorithmOptimizer, OptimizationEngine};
pub use crate::profinet_driver::{ProfinetDriver, SensorDataBatch};
pub use crate::renewable_coupler::{RenewableCoupler, RenewableCouplingRequest};
pub use crate::renewable_integrator::{MPCWorker, RenewableCouplingRequest, RenewableIntegrator};
