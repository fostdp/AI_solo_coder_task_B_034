use chrono::Utc;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};
use tracing::{debug, info, warn};

use crate::config::RenewableCouplingConfig;
use crate::models::*;

#[derive(Debug, Clone)]
pub struct RenewableCouplingRequest {
    pub electrolyzer_id: u8,
    pub renewable_power: f64,
    pub grid_power_available: f64,
    pub current_power: f64,
    pub current_density: f64,
    pub power_history: Vec<f64>,
}

#[derive(Debug)]
struct MPCComputeRequest {
    current_power: f64,
    target_power: f64,
    predictions: Vec<f64>,
    previous_solution: Option<f64>,
    response_tx: oneshot::Sender<f64>,
}

struct MPCTask {
    config: RenewableCouplingConfig,
    rx: mpsc::Receiver<MPCComputeRequest>,
}

impl MPCTask {
    fn new(config: RenewableCouplingConfig, rx: mpsc::Receiver<MPCComputeRequest>) -> Self {
        Self { config, rx }
    }

    async fn run(mut self) {
        debug!("MPC task started, ready to process compute requests");
        while let Some(req) = self.rx.recv().await {
            let result = self.solve_mpc(
                req.current_power,
                req.target_power,
                &req.predictions,
                req.previous_solution,
            );
            if req.response_tx.send(result).is_err() {
                warn!("MPC task: receiver dropped before sending result");
            }
        }
        debug!("MPC task shutting down");
    }

    fn solve_mpc(
        &self,
        current_power: f64,
        target_power: f64,
        predictions: &[f64],
        previous_solution: Option<f64>,
    ) -> f64 {
        let horizon = self.config.mpc_horizon as usize;
        let control_weight = self.config.mpc_control_weight;
        let rate_weight = self.config.mpc_rate_weight;
        let max_iterations = self.config.mpc_max_iterations as usize;

        let mut best_control = current_power;
        let mut min_cost = f64::INFINITY;

        let search_range = (self.config.max_power_kw - self.config.min_power_kw) * 0.5;
        let center = if self.config.hot_start_enabled && previous_solution.is_some() {
            let prev = previous_solution.unwrap();
            let warm_start_center = prev * self.config.warm_start_gain
                + target_power * (1.0 - self.config.warm_start_gain);
            warm_start_center.clamp(
                current_power - search_range,
                current_power + search_range,
            )
        } else {
            target_power.clamp(
                current_power - search_range,
                current_power + search_range,
            )
        };

        let step = search_range / max_iterations.max(1) as f64;

        let mut u = (center - search_range).max(self.config.min_power_kw);
        let mut iteration = 0;
        while u <= (center + search_range).min(self.config.max_power_kw)
            && iteration < max_iterations
        {
            let mut cost = 0.0;
            let mut prev_power = current_power;

            for k in 0..horizon {
                let pred_target = predictions.get(k).copied().unwrap_or(target_power);

                let power_error = u - pred_target;
                cost += control_weight * power_error * power_error;

                let rate = u - prev_power;
                cost += rate_weight * rate * rate;

                prev_power = u;
            }

            if cost < min_cost {
                min_cost = cost;
                best_control = u;
            }

            u += step;
            iteration += 1;
        }

        best_control
    }

    fn approximate_solve(
        &self,
        current_power: f64,
        target_power: f64,
        previous_solution: Option<f64>,
    ) -> f64 {
        let ramp_rate = self.config.power_ramp_rate_per_sec * self.config.control_interval_secs as f64;
        let max_delta = ramp_rate * self.config.approximate_solve_threshold;

        let base_target = if self.config.hot_start_enabled && previous_solution.is_some() {
            let prev = previous_solution.unwrap();
            prev * self.config.warm_start_gain + target_power * (1.0 - self.config.warm_start_gain)
        } else {
            target_power
        };

        let delta = base_target - current_power;
        let clamped_delta = delta.clamp(-max_delta, max_delta);
        let approximate = current_power + clamped_delta;

        approximate.clamp(self.config.min_power_kw, self.config.max_power_kw)
    }
}

#[derive(Debug, Clone)]
pub struct RenewableIntegrator {
    config: RenewableCouplingConfig,
    state: Arc<Mutex<MPCStateInternal>>,
    mpc_tx: mpsc::Sender<MPCComputeRequest>,
}

#[derive(Debug, Clone)]
struct MPCStateInternal {
    power_history: VecDeque<f64>,
    control_history: VecDeque<f64>,
    last_start_time: f64,
    start_stop_count: u32,
    operating_hours: f64,
    is_running: bool,
    prediction_buffer: VecDeque<f64>,
    previous_solution: Option<f64>,
    last_power_change: f64,
}

pub type MPCWorker = RenewableIntegrator;

impl RenewableIntegrator {
    pub fn new(config: RenewableCouplingConfig) -> (Self, mpsc::Receiver<RenewableCouplingStatus>) {
        let (status_tx, status_rx) = mpsc::channel(100);
        let (mpc_tx, mpc_rx) = mpsc::channel::<MPCComputeRequest>(100);

        let state = MPCStateInternal {
            power_history: VecDeque::with_capacity(100),
            control_history: VecDeque::with_capacity(100),
            last_start_time: 0.0,
            start_stop_count: 0,
            operating_hours: 0.0,
            is_running: false,
            prediction_buffer: VecDeque::with_capacity(50),
            previous_solution: None,
            last_power_change: 0.0,
        };

        let mpc_task = MPCTask::new(config.clone(), mpc_rx);
        tokio::spawn(async move {
            mpc_task.run().await;
        });

        let controller = Self {
            config,
            state: Arc::new(Mutex::new(state)),
            mpc_tx,
        };

        (controller, status_rx)
    }

    pub async fn update_control(
        &self,
        request: RenewableCouplingRequest,
    ) -> Result<MPCState, Box<dyn std::error::Error + Send + Sync>> {
        debug!(
            "Updating MPC control for electrolyzer {}: renewable_power={:.2} kW, current_power={:.2} kW",
            request.electrolyzer_id, request.renewable_power, request.current_power
        );

        let mut state = self.state.lock().await;

        state.power_history.push_back(request.current_power);
        if state.power_history.len() > 100 {
            state.power_history.pop_front();
        }

        let predicted_power = self.predict_renewable_power(&request.power_history);

        for p in &predicted_power {
            state.prediction_buffer.push_back(*p);
        }
        if state.prediction_buffer.len() > 50 {
            state.prediction_buffer.drain(0..state.prediction_buffer.len() - 50);
        }

        let target_power = request.renewable_power;

        let deadzone = self.config.deadzone_percentage / 100.0 * self.config.max_power_kw;
        let power_deviation = (request.current_power - target_power).abs();

        state.last_power_change = power_deviation;

        let use_approximate = self.config.approximate_solve_enabled
            && power_deviation > self.config.power_change_threshold;

        let mut control_signal = if power_deviation <= deadzone {
            request.current_power
        } else if use_approximate {
            debug!(
                "Power change {:.2} kW exceeds threshold {:.2} kW, using approximate solve",
                power_deviation, self.config.power_change_threshold
            );
            self.approximate_solve_local(
                request.current_power,
                target_power,
                state.previous_solution,
            )
        } else {
            let start_time = std::time::Instant::now();

            let (resp_tx, resp_rx) = oneshot::channel();
            let compute_req = MPCComputeRequest {
                current_power: request.current_power,
                target_power,
                predictions: state.prediction_buffer.iter().cloned().collect(),
                previous_solution: state.previous_solution,
                response_tx: resp_tx,
            };

            if let Err(e) = self.mpc_tx.send(compute_req).await {
                warn!(
                    "Failed to send MPC compute request to task: {}, falling back to approximate",
                    e
                );
                self.approximate_solve_local(
                    request.current_power,
                    target_power,
                    state.previous_solution,
                )
            } else {
                let timeout_duration = std::time::Duration::from_micros(
                    self.config.mpc_solve_timeout_us,
                );
                match tokio::time::timeout(timeout_duration, resp_rx).await {
                    Ok(Ok(result)) => {
                        let elapsed = start_time.elapsed();
                        debug!(
                            "MPC task completed in {:?}, result={:.2} kW",
                            elapsed, result
                        );
                        result
                    }
                    Ok(Err(e)) => {
                        warn!(
                            "MPC task response error: {}, falling back to approximate",
                            e
                        );
                        self.approximate_solve_local(
                            request.current_power,
                            target_power,
                            state.previous_solution,
                        )
                    }
                    Err(_) => {
                        warn!(
                            "MPC solve timed out after {:?}, falling back to approximate",
                            start_time.elapsed()
                        );
                        self.approximate_solve_local(
                            request.current_power,
                            target_power,
                            state.previous_solution,
                        )
                    }
                }
            }
        };

        state.previous_solution = Some(control_signal);

        let current_time = Utc::now().timestamp() as f64;

        if !state.is_running && control_signal > self.config.min_power_kw {
            if current_time - state.last_start_time >= self.config.min_operation_time_secs as f64
                || state.start_stop_count == 0
            {
                state.is_running = true;
                state.last_start_time = current_time;
                state.start_stop_count += 1;
                info!(
                    "Electrolyzer {} started, start_stop_count={}",
                    request.electrolyzer_id, state.start_stop_count
                );
            } else {
                debug!(
                    "Electrolyzer {} prevented from starting due to min operation time constraint",
                    request.electrolyzer_id
                );
                control_signal = 0.0;
            }
        } else if state.is_running && control_signal < self.config.min_power_kw * 0.5 {
            if current_time - state.last_start_time >= self.config.min_operation_time_secs as f64 {
                state.is_running = false;
                state.operating_hours += (current_time - state.last_start_time) / 3600.0;
                info!(
                    "Electrolyzer {} stopped after {:.2} hours",
                    request.electrolyzer_id,
                    (current_time - state.last_start_time) / 3600.0
                );
            } else {
                debug!(
                    "Electrolyzer {} prevented from stopping due to min operation time constraint",
                    request.electrolyzer_id
                );
                control_signal = self.config.min_power_kw;
            }
        }

        if state.is_running {
            let max_delta = self.config.power_ramp_rate_per_sec * self.config.control_interval_secs as f64;
            control_signal = control_signal.clamp(
                request.current_power - max_delta,
                request.current_power + max_delta,
            );
            control_signal = control_signal.clamp(
                self.config.min_power_kw,
                self.config.max_power_kw,
            );
        } else {
            control_signal = 0.0;
        }

        state.control_history.push_back(control_signal);
        if state.control_history.len() > 100 {
            state.control_history.pop_front();
        }

        let tracking_error = if state.is_running {
            (control_signal - target_power).abs()
        } else {
            0.0
        };

        let current_density = self.power_to_current_density(control_signal);

        let mpc_state = MPCState {
            timestamp: Utc::now(),
            electrolyzer_id: request.electrolyzer_id,
            target_power,
            actual_power: control_signal,
            current_density,
            tracking_error,
            control_signal,
            start_stop_count: state.start_stop_count,
            operating_hours: state.operating_hours
                + if state.is_running {
                    (current_time - state.last_start_time) / 3600.0
                } else {
                    0.0
                },
        };

        if state.power_history.len() >= 10 {
            let recent_history: Vec<f64> = state.power_history.iter().rev().take(10).cloned().collect();
            let avg_recent = recent_history.iter().sum::<f64>() / recent_history.len() as f64;
            let renewable_utilization = if request.renewable_power > 0.0 {
                (control_signal.min(request.renewable_power) / request.renewable_power) * 100.0
            } else {
                0.0
            };
            let grid_supplementation = if control_signal > request.renewable_power {
                control_signal - request.renewable_power
            } else {
                0.0
            };
            let tracking_accuracy = if target_power > 0.0 && state.is_running {
                (1.0 - tracking_error / target_power).max(0.0) * 100.0
            } else {
                100.0
            };

            let status = RenewableCouplingStatus {
                electrolyzer_id: request.electrolyzer_id,
                timestamp: Utc::now(),
                renewable_utilization,
                grid_supplementation,
                tracking_accuracy,
                start_stop_count: state.start_stop_count,
                is_tracking: state.is_running,
                predicted_power: predicted_power.clone(),
            };

            debug!(
                "Renewable coupling status for electrolyzer {}: utilization={:.1}%, tracking_accuracy={:.1}%",
                request.electrolyzer_id, renewable_utilization, tracking_accuracy
            );
        }

        Ok(mpc_state)
    }

    pub fn predict_renewable_power(&self, history: &[f64]) -> Vec<f64> {
        let horizon = self.config.mpc_horizon as usize;
        let mut predictions = Vec::with_capacity(horizon);

        if history.len() < 2 {
            for _ in 0..horizon {
                predictions.push(history.last().copied().unwrap_or(0.0));
            }
            return predictions;
        }

        let recent: Vec<f64> = history.iter().rev().take(20).cloned().collect();
        let n = recent.len() as f64;

        let sum_x: f64 = (0..recent.len()).map(|i| i as f64).sum();
        let sum_y: f64 = recent.iter().sum();
        let sum_xy: f64 = recent
            .iter()
            .enumerate()
            .map(|(i, &y)| (recent.len() - 1 - i) as f64 * y)
            .sum();
        let sum_x2: f64 = (0..recent.len()).map(|i| ((recent.len() - 1 - i) as f64).powi(2)).sum();

        let denominator = n * sum_x2 - sum_x * sum_x;
        let slope = if denominator.abs() > 1e-10 {
            (n * sum_xy - sum_x * sum_y) / denominator
        } else {
            0.0
        };
        let intercept = (sum_y - slope * sum_x) / n;

        let last_value = *recent.first().unwrap_or(&0.0);
        let alpha = 0.3;
        let mut smoothed = last_value;

        for i in 0..horizon {
            let trend_prediction = intercept + slope * (recent.len() + i) as f64;
            smoothed = alpha * trend_prediction + (1.0 - alpha) * smoothed;

            let fluctuation = 0.05 * last_value * (i as f64 * 0.5).sin();
            let prediction = (smoothed + fluctuation).max(0.0);

            predictions.push(prediction);
        }

        predictions
    }

    pub fn power_to_current_density(&self, power: f64) -> f64 {
        let voltage = 1.8;
        let n_cells = 100.0;
        let active_area = 1.0;

        if power <= 0.0 {
            0.0
        } else {
            power * 1000.0 / (voltage * n_cells * active_area)
        }
    }

    fn approximate_solve_local(
        &self,
        current_power: f64,
        target_power: f64,
        previous_solution: Option<f64>,
    ) -> f64 {
        let ramp_rate = self.config.power_ramp_rate_per_sec * self.config.control_interval_secs as f64;
        let max_delta = ramp_rate * self.config.approximate_solve_threshold;

        let base_target = if self.config.hot_start_enabled && previous_solution.is_some() {
            let prev = previous_solution.unwrap();
            prev * self.config.warm_start_gain + target_power * (1.0 - self.config.warm_start_gain)
        } else {
            target_power
        };

        let delta = base_target - current_power;
        let clamped_delta = delta.clamp(-max_delta, max_delta);
        let approximate = current_power + clamped_delta;

        approximate.clamp(self.config.min_power_kw, self.config.max_power_kw)
    }

    pub async fn get_start_stop_count(&self) -> u32 {
        self.state.lock().await.start_stop_count
    }

    pub async fn get_operating_hours(&self) -> f64 {
        let state = self.state.lock().await;
        state.operating_hours
            + if state.is_running {
                (Utc::now().timestamp() as f64 - state.last_start_time) / 3600.0
            } else {
                0.0
            }
    }

    pub async fn is_running(&self) -> bool {
        self.state.lock().await.is_running
    }

    pub async fn reset(&self) {
        let mut state = self.state.lock().await;
        state.power_history.clear();
        state.control_history.clear();
        state.last_start_time = 0.0;
        state.start_stop_count = 0;
        state.operating_hours = 0.0;
        state.is_running = false;
        state.prediction_buffer.clear();
        state.previous_solution = None;
        state.last_power_change = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn create_test_config() -> RenewableCouplingConfig {
        RenewableCouplingConfig {
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
            mpc_max_iterations: 100,
            mpc_solve_timeout_us: 1000000,
            hot_start_enabled: true,
            approximate_solve_enabled: true,
            approximate_solve_threshold: 0.1,
            power_change_threshold: 20.0,
            warm_start_gain: 0.8,
        }
    }

    #[test]
    fn test_power_to_current_density_normal() {
        let config = create_test_config();
        let (controller, _) = RenewableIntegrator::new(config);

        let current_density = controller.power_to_current_density(50.0);

        assert_relative_eq!(current_density, 50000.0 / (1.8 * 100.0), epsilon = 1.0);
        assert!(current_density > 0.0);
    }

    #[test]
    fn test_power_to_current_density_zero_power() {
        let config = create_test_config();
        let (controller, _) = RenewableIntegrator::new(config);

        let current_density = controller.power_to_current_density(0.0);
        assert_eq!(current_density, 0.0);
    }

    #[test]
    fn test_power_to_current_density_proportional() {
        let config = create_test_config();
        let (controller, _) = RenewableIntegrator::new(config);

        let cd1 = controller.power_to_current_density(25.0);
        let cd2 = controller.power_to_current_density(50.0);

        assert_relative_eq!(cd2 / cd1, 2.0, epsilon = 0.001);
    }

    #[test]
    fn test_predict_renewable_power_constant() {
        let config = create_test_config();
        let (controller, _) = RenewableIntegrator::new(config);

        let history: Vec<f64> = vec![50.0; 20];
        let predictions = controller.predict_renewable_power(&history);

        assert_eq!(predictions.len(), 10);
        for p in &predictions {
            assert_relative_eq!(*p, 50.0, epsilon = 5.0);
        }
    }

    #[test]
    fn test_predict_renewable_power_increasing() {
        let config = create_test_config();
        let (controller, _) = RenewableIntegrator::new(config);

        let history: Vec<f64> = (0..20).map(|i| 30.0 + i as f64 * 1.0).collect();
        let predictions = controller.predict_renewable_power(&history);

        assert!(predictions[0] > 45.0);
        assert!(predictions[9] > predictions[0]);
    }

    #[test]
    fn test_predict_renewable_power_decreasing() {
        let config = create_test_config();
        let (controller, _) = RenewableIntegrator::new(config);

        let history: Vec<f64> = (0..20).map(|i| 70.0 - i as f64 * 1.5).collect();
        let predictions = controller.predict_renewable_power(&history);

        assert!(predictions[9] < predictions[0]);
    }

    #[test]
    fn test_approximate_solve_local_basic() {
        let config = create_test_config();
        let (controller, _) = RenewableIntegrator::new(config);

        let current_power = 40.0;
        let target_power = 80.0;

        let result = controller.approximate_solve_local(current_power, target_power, None);

        assert!(result > current_power);
        assert!(result <= config.max_power_kw);
        assert!(result >= config.min_power_kw);
    }

    #[tokio::test]
    async fn test_update_control_initial_start() {
        let config = create_test_config();
        let (controller, _rx) = RenewableIntegrator::new(config);

        controller.reset().await;

        let request = RenewableCouplingRequest {
            electrolyzer_id: 1,
            renewable_power: 50.0,
            grid_power_available: 100.0,
            current_power: 0.0,
            current_density: 0.0,
            power_history: vec![0.0; 20],
        };

        let result = controller.update_control(request).await.unwrap();

        assert_eq!(result.electrolyzer_id, 1);
        assert!(result.actual_power > 0.0);
        assert!(result.current_density > 0.0);
        assert_eq!(result.start_stop_count, 1);
        assert!(controller.is_running().await);
    }

    #[tokio::test]
    async fn test_update_control_tracking_renewable() {
        let config = create_test_config();
        let (controller, _rx) = RenewableIntegrator::new(config);

        controller.reset().await;

        let request1 = RenewableCouplingRequest {
            electrolyzer_id: 1,
            renewable_power: 50.0,
            grid_power_available: 100.0,
            current_power: 0.0,
            current_density: 0.0,
            power_history: vec![50.0; 20],
        };
        controller.update_control(request1).await.unwrap();

        let request2 = RenewableCouplingRequest {
            electrolyzer_id: 1,
            renewable_power: 70.0,
            grid_power_available: 100.0,
            current_power: 50.0,
            current_density: 277.0,
            power_history: vec![50.0; 20],
        };
        let result = controller.update_control(request2).await.unwrap();

        assert!(result.actual_power > 50.0);
        assert!(result.actual_power <= 70.0 + 5.0);
        assert!(result.tracking_error < 20.0);
    }

    #[tokio::test]
    async fn test_update_control_deadzone() {
        let config = create_test_config();
        let (controller, _rx) = RenewableIntegrator::new(config);

        controller.reset().await;

        let request1 = RenewableCouplingRequest {
            electrolyzer_id: 1,
            renewable_power: 50.0,
            grid_power_available: 100.0,
            current_power: 0.0,
            current_density: 0.0,
            power_history: vec![50.0; 20],
        };
        controller.update_control(request1).await.unwrap();

        let request2 = RenewableCouplingRequest {
            electrolyzer_id: 1,
            renewable_power: 51.0,
            grid_power_available: 100.0,
            current_power: 50.0,
            current_density: 277.0,
            power_history: vec![50.0; 20],
        };
        let result = controller.update_control(request2).await.unwrap();

        assert_relative_eq!(result.actual_power, 50.0, epsilon = 1.0);
    }

    #[tokio::test]
    async fn test_update_control_start_stop_protection() {
        let config = create_test_config();
        let (controller, _rx) = RenewableIntegrator::new(config.clone());

        controller.reset().await;

        let request1 = RenewableCouplingRequest {
            electrolyzer_id: 1,
            renewable_power: 50.0,
            grid_power_available: 100.0,
            current_power: 0.0,
            current_density: 0.0,
            power_history: vec![50.0; 20],
        };
        controller.update_control(request1).await.unwrap();
        assert_eq!(controller.get_start_stop_count().await, 1);

        let request2 = RenewableCouplingRequest {
            electrolyzer_id: 1,
            renewable_power: 0.0,
            grid_power_available: 100.0,
            current_power: 50.0,
            current_density: 277.0,
            power_history: vec![0.0; 20],
        };
        let result = controller.update_control(request2).await.unwrap();

        assert!(result.actual_power > 0.0);
        assert_eq!(controller.get_start_stop_count().await, 1);
    }

    #[tokio::test]
    async fn test_update_control_ramp_rate_limit() {
        let mut config = create_test_config();
        config.power_ramp_rate_per_sec = 0.001;
        let (controller, _rx) = RenewableIntegrator::new(config.clone());

        controller.reset().await;

        let request1 = RenewableCouplingRequest {
            electrolyzer_id: 1,
            renewable_power: 50.0,
            grid_power_available: 100.0,
            current_power: 0.0,
            current_density: 0.0,
            power_history: vec![50.0; 20],
        };
        controller.update_control(request1).await.unwrap();

        let request2 = RenewableCouplingRequest {
            electrolyzer_id: 1,
            renewable_power: 100.0,
            grid_power_available: 100.0,
            current_power: 50.0,
            current_density: 277.0,
            power_history: vec![100.0; 20],
        };
        let result = controller.update_control(request2).await.unwrap();

        let max_delta = config.power_ramp_rate_per_sec * config.control_interval_secs as f64;
        assert!(result.actual_power <= 50.0 + max_delta + 1.0);
        assert!(result.actual_power >= 50.0 - max_delta - 1.0);
    }

    #[tokio::test]
    async fn test_update_control_power_bounds() {
        let config = create_test_config();
        let (controller, _rx) = RenewableIntegrator::new(config.clone());

        controller.reset().await;

        let request1 = RenewableCouplingRequest {
            electrolyzer_id: 1,
            renewable_power: 5.0,
            grid_power_available: 100.0,
            current_power: 0.0,
            current_density: 0.0,
            power_history: vec![5.0; 20],
        };
        let result = controller.update_control(request1).await.unwrap();

        assert!(result.actual_power <= config.max_power_kw);
        assert!(result.actual_power >= config.min_power_kw || result.actual_power == 0.0);
    }

    #[tokio::test]
    async fn test_mpc_worker_type_alias() {
        let config = create_test_config();
        let (worker, _rx) = MPCWorker::new(config);
        worker.reset().await;
        assert!(!worker.is_running().await);
    }

    #[test]
    fn test_approximate_solve_local_ramp_limited() {
        let mut config = create_test_config();
        config.power_ramp_rate_per_sec = 0.001;
        config.control_interval_secs = 5;
        let (controller, _) = RenewableIntegrator::new(config.clone());

        let current_power = 40.0;
        let target_power = 100.0;

        let result = controller.approximate_solve_local(current_power, target_power, None);

        let max_delta = config.power_ramp_rate_per_sec
            * config.control_interval_secs as f64
            * config.approximate_solve_threshold;
        assert!(result <= current_power + max_delta + 0.1);
    }

    #[tokio::test]
    async fn test_previous_solution_stored() {
        let config = create_test_config();
        let (controller, _rx) = RenewableIntegrator::new(config.clone());

        controller.reset().await;

        let request1 = RenewableCouplingRequest {
            electrolyzer_id: 1,
            renewable_power: 50.0,
            grid_power_available: 100.0,
            current_power: 0.0,
            current_density: 0.0,
            power_history: vec![50.0; 20],
        };
        let result1 = controller.update_control(request1).await.unwrap();

        let request2 = RenewableCouplingRequest {
            electrolyzer_id: 1,
            renewable_power: 55.0,
            grid_power_available: 100.0,
            current_power: result1.actual_power,
            current_density: result1.current_density,
            power_history: vec![50.0; 20],
        };
        let result2 = controller.update_control(request2).await.unwrap();

        assert!(result2.actual_power > 0.0);
        assert!(result2.tracking_error < 20.0);
    }

    #[test]
    fn test_approximate_solve_local_bounds() {
        let config = create_test_config();
        let (controller, _) = RenewableIntegrator::new(config);

        let result_low = controller.approximate_solve_local(5.0, 5.0, None);
        assert!(result_low >= config.min_power_kw || result_low == 0.0);

        let result_high = controller.approximate_solve_local(150.0, 150.0, None);
        assert!(result_high <= config.max_power_kw);
    }

    #[tokio::test]
    async fn test_reset_clears_previous_solution() {
        let config = create_test_config();
        let (controller, _rx) = RenewableIntegrator::new(config);

        controller.reset().await;

        let request1 = RenewableCouplingRequest {
            electrolyzer_id: 1,
            renewable_power: 50.0,
            grid_power_available: 100.0,
            current_power: 0.0,
            current_density: 0.0,
            power_history: vec![50.0; 20],
        };
        controller.update_control(request1).await.unwrap();

        let state = controller.state.lock().await;
        assert!(state.previous_solution.is_some());
        drop(state);

        controller.reset().await;

        let state = controller.state.lock().await;
        assert!(state.previous_solution.is_none());
        assert_relative_eq!(state.last_power_change, 0.0, epsilon = 1e-10);
    }

    #[tokio::test]
    async fn test_mpc_task_channel_communication() {
        let mut config = create_test_config();
        config.mpc_max_iterations = 10;
        let (controller, _rx) = RenewableIntegrator::new(config);

        controller.reset().await;

        let request = RenewableCouplingRequest {
            electrolyzer_id: 1,
            renewable_power: 50.0,
            grid_power_available: 100.0,
            current_power: 0.0,
            current_density: 0.0,
            power_history: vec![50.0; 20],
        };

        let result = controller.update_control(request).await.unwrap();

        assert!(result.actual_power > 0.0);
        assert!(result.current_density > 0.0);
    }

    #[tokio::test]
    async fn test_mpc_task_concurrent_requests() {
        let config = create_test_config();
        let (controller, _rx) = RenewableIntegrator::new(config);

        controller.reset().await;

        let mut handles = Vec::new();
        for i in 0..5 {
            let controller_clone = controller.clone();
            let handle = tokio::spawn(async move {
                let request = RenewableCouplingRequest {
                    electrolyzer_id: 1,
                    renewable_power: 50.0 + i as f64 * 10.0,
                    grid_power_available: 100.0,
                    current_power: 50.0,
                    current_density: 2.0,
                    power_history: vec![50.0 + i as f64 * 10.0; 20],
                };
                controller_clone.update_control(request).await
            });
            handles.push(handle);
        }

        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_mpc_task_warm_start_effectiveness() {
        let config = create_test_config();
        let (controller, _rx) = RenewableIntegrator::new(config);

        controller.reset().await;

        let request1 = RenewableCouplingRequest {
            electrolyzer_id: 1,
            renewable_power: 50.0,
            grid_power_available: 100.0,
            current_power: 0.0,
            current_density: 0.0,
            power_history: vec![50.0; 20],
        };
        let result1 = controller.update_control(request1).await.unwrap();

        let request2 = RenewableCouplingRequest {
            electrolyzer_id: 1,
            renewable_power: 55.0,
            grid_power_available: 100.0,
            current_power: result1.actual_power,
            current_density: result1.current_density,
            power_history: vec![55.0; 20],
        };
        let result2 = controller.update_control(request2).await.unwrap();

        assert!(result2.used_warm_start);
        assert_relative_eq!(result2.actual_power, 55.0, max_relative = 0.1);
    }

    #[tokio::test]
    async fn test_mpc_task_power_ramp_rates() {
        let mut config = create_test_config();
        config.power_ramp_rate_per_sec = 0.01;
        let (controller, _rx) = RenewableIntegrator::new(config);

        controller.reset().await;

        let request1 = RenewableCouplingRequest {
            electrolyzer_id: 1,
            renewable_power: 100.0,
            grid_power_available: 100.0,
            current_power: 0.0,
            current_density: 0.0,
            power_history: vec![100.0; 20],
        };
        let result1 = controller.update_control(request1).await.unwrap();

        let request2 = RenewableCouplingRequest {
            electrolyzer_id: 1,
            renewable_power: 10.0,
            grid_power_available: 100.0,
            current_power: result1.actual_power,
            current_density: result1.current_density,
            power_history: vec![10.0; 20],
        };
        let result2 = controller.update_control(request2).await.unwrap();

        let max_change = config.power_ramp_rate_per_sec * config.mpc_time_step_sec as f64;
        let actual_change = (result2.actual_power - result1.actual_power).abs();
        assert!(actual_change <= max_change * 1.01);
    }

    #[test]
    fn test_type_alias_mpc_worker_compatibility() {
        let config = create_test_config();
        let (worker, _rx) = MPCWorker::new(config);
        let (integrator, _rx2) = RenewableIntegrator::new(config);

        assert_eq!(std::mem::size_of::<MPCWorker>(), std::mem::size_of::<RenewableIntegrator>());
    }
}
