use crate::config::{EfficiencyModelConfig, OptimizationConfig, SystemConfig};
use crate::models::*;
use chrono::{DateTime, Utc};
use std::f64::consts::E;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::debug;

const FARADAY_CONSTANT: f64 = 96485.3321;
const MOLAR_MASS_H2: f64 = 2.01588e-3;
const CELL_COUNT: f64 = 100.0;

#[derive(Debug, Clone)]
pub struct EfficiencyResult {
    pub electrolyzer_id: u8,
    pub timestamp: DateTime<Utc>,
    pub current_density: f64,
    pub cell_voltage: f64,
    pub water_temp: f64,
    pub efficiency: f64,
    pub hydrogen_production: f64,
    pub power_consumption: f64,
    pub needs_optimization: bool,
}

#[derive(Clone)]
pub struct EfficiencyModel {
    a: f64,
    b: f64,
    r: f64,
    exchange_current_density: f64,
    transfer_coefficient: f64,
}

impl EfficiencyModel {
    pub fn from_config(config: &EfficiencyModelConfig) -> Self {
        Self {
            a: config.a,
            b: config.b,
            r: config.r,
            exchange_current_density: config.exchange_current_density,
            transfer_coefficient: config.transfer_coefficient,
        }
    }

    pub fn calculate_polarization_voltage(&self, current_density: f64, temperature: f64) -> f64 {
        let temp_k = temperature + 273.15;
        let reversible_voltage = 1.229 - 0.0009 * (temp_k - 298.15);

        if current_density <= 0.0 {
            return reversible_voltage;
        }

        let activation_loss = self.a
            * (current_density / self.exchange_current_density)
                .ln()
                .max(0.0);

        let concentration_loss = self.b * (1.0 - E.powf(-current_density / self.b));

        let ohmic_loss = self.r * current_density;

        reversible_voltage + activation_loss + concentration_loss + ohmic_loss
    }

    pub fn calculate_voltage_efficiency(&self, _current_density: f64, cell_voltage: f64) -> f64 {
        let thermoneutral_voltage = 1.481;
        if cell_voltage <= 0.0 {
            return 0.0;
        }
        (thermoneutral_voltage / cell_voltage) * 100.0
    }

    pub fn calculate_efficiency(
        &self,
        current_density: f64,
        cell_voltage: f64,
        temperature: f64,
    ) -> f64 {
        let polarization_voltage = self.calculate_polarization_voltage(current_density, temperature);
        let voltage_efficiency = self.calculate_voltage_efficiency(current_density, cell_voltage);
        
        let faradaic_efficiency = 95.0 + 5.0 / (1.0 + E.powf(-(current_density - 1.0) / 0.2));
        
        voltage_efficiency * faradaic_efficiency / 100.0
    }

    pub fn calculate_hydrogen_production_rate(
        &self,
        current_density: f64,
        active_area: f64,
    ) -> f64 {
        let current = current_density * active_area * 10000.0;
        let production_rate = (current * MOLAR_MASS_H2) / (2.0 * FARADAY_CONSTANT);
        production_rate * 3600.0
    }

    pub fn calculate_power_consumption(
        &self,
        current_density: f64,
        cell_voltage: f64,
        active_area: f64,
    ) -> f64 {
        let current = current_density * active_area * 10000.0;
        (current * cell_voltage * CELL_COUNT) / 1000.0
    }
}

#[derive(Clone)]
pub struct EfficiencyAnalyzer {
    model: EfficiencyModel,
    optimization_config: OptimizationConfig,
    system_config: SystemConfig,
    result_tx: mpsc::Sender<EfficiencyResult>,
}

impl EfficiencyAnalyzer {
    pub fn new(
        model_config: &EfficiencyModelConfig,
        optimization_config: OptimizationConfig,
        system_config: SystemConfig,
    ) -> (Self, mpsc::Receiver<EfficiencyResult>) {
        let (result_tx, result_rx) = mpsc::channel(1000);
        (
            Self {
                model: EfficiencyModel::from_config(model_config),
                optimization_config,
                system_config,
                result_tx,
            },
            result_rx,
        )
    }

    pub fn model(&self) -> &EfficiencyModel {
        &self.model
    }

    pub fn optimization_config(&self) -> &OptimizationConfig {
        &self.optimization_config
    }

    pub async fn analyze_batch(
        &self,
        batch: &super::profinet_driver::SensorDataBatch,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let efficiency = self.model.calculate_efficiency(
            batch.avg_current_density,
            batch.avg_voltage,
            batch.avg_water_temp,
        );

        let hydrogen_production = self.model.calculate_hydrogen_production_rate(
            batch.avg_current_density,
            self.system_config.active_area,
        );

        let power_consumption = self.model.calculate_power_consumption(
            batch.avg_current_density,
            batch.avg_voltage,
            self.system_config.active_area,
        );

        let needs_optimization = efficiency < self.optimization_config.efficiency_threshold;

        let result = EfficiencyResult {
            electrolyzer_id: batch.electrolyzer_id,
            timestamp: batch.timestamp,
            current_density: batch.avg_current_density,
            cell_voltage: batch.avg_voltage,
            water_temp: batch.avg_water_temp,
            efficiency,
            hydrogen_production,
            power_consumption,
            needs_optimization,
        };

        self.result_tx.send(result).await?;
        Ok(())
    }

    pub fn get_efficiency_curve(
        &self,
        current_density_range: std::ops::Range<f64>,
        steps: usize,
        water_temp: f64,
    ) -> Vec<(f64, f64)> {
        let mut curve = Vec::with_capacity(steps);
        let step = (current_density_range.end - current_density_range.start) / steps as f64;

        for i in 0..=steps {
            let cd = current_density_range.start + step * i as f64;
            let voltage = self.model.calculate_polarization_voltage(cd, water_temp);
            let efficiency = self.model.calculate_efficiency(cd, voltage, water_temp);
            curve.push((cd, efficiency));
        }

        curve
    }

    pub fn get_polarization_curve(
        &self,
        current_density_range: std::ops::Range<f64>,
        steps: usize,
        water_temp: f64,
    ) -> Vec<(f64, f64)> {
        let mut curve = Vec::with_capacity(steps);
        let step = (current_density_range.end - current_density_range.start) / steps as f64;

        for i in 0..=steps {
            let cd = current_density_range.start + step * i as f64;
            let voltage = self.model.calculate_polarization_voltage(cd, water_temp);
            curve.push((cd, voltage));
        }

        curve
    }
}

#[derive(Debug, Clone)]
pub struct OptimizationTask {
    pub electrolyzer_id: u8,
    pub current_density: f64,
    pub cell_voltage: f64,
    pub water_temp: f64,
    pub current_efficiency: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct OptimizationResultMessage {
    pub suggestion: OptimizationSuggestion,
}

pub struct OptimizationEngineHandle {
    task_tx: mpsc::Sender<OptimizationTask>,
    result_rx: Option<mpsc::Receiver<OptimizationResultMessage>>,
    pending_electrolyzers: Arc<std::sync::Mutex<std::collections::HashSet<u8>>>,
}

impl OptimizationEngineHandle {
    pub async fn submit_optimization(
        &self,
        task: OptimizationTask,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut pending = self.pending_electrolyzers.lock().unwrap();
        
        if pending.contains(&task.electrolyzer_id) {
            debug!(
                "Optimization for electrolyzer {} already in queue, skipping",
                task.electrolyzer_id
            );
            return Ok(());
        }
        
        pending.insert(task.electrolyzer_id);
        drop(pending);
        
        if let Err(e) = self.task_tx.send(task).await {
            let mut pending = self.pending_electrolyzers.lock().unwrap();
            pending.remove(&e.0.electrolyzer_id);
            return Err(format!("Failed to submit optimization task: {}", e).into());
        }
        
        Ok(())
    }

    pub fn poll_result(&mut self) -> Option<OptimizationSuggestion> {
        self.result_rx.as_mut().and_then(|rx| rx.try_recv().ok().map(|msg| msg.suggestion))
    }

    pub fn queue_depth(&self) -> usize {
        let pending = self.pending_electrolyzers.lock().unwrap();
        pending.len()
    }
}
