use chrono::{Duration, Utc};
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tracing::{debug, info, warn};

use crate::config::DegradationPredictionConfig;
use crate::models::*;

#[derive(Debug, Clone)]
pub struct DegradationPredictionRequest {
    pub electrolyzer_id: u8,
    pub history_data: Vec<VoltageCurrentPoint>,
}

#[derive(Debug, Clone)]
pub struct DegradationPredictor {
    config: DegradationPredictionConfig,
    semaphore: Arc<Semaphore>,
}

impl DegradationPredictor {
    pub fn new(config: DegradationPredictionConfig) -> (Self, mpsc::Receiver<DegradationPrediction>) {
        let (tx, rx) = mpsc::channel(100);
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_predictions));

        let predictor = Self {
            config,
            semaphore,
        };

        (predictor, rx)
    }

    pub async fn predict_degradation(
        &self,
        request: DegradationPredictionRequest,
    ) -> Result<DegradationPrediction, Box<dyn std::error::Error + Send + Sync>> {
        let _permit = self.semaphore.acquire().await?;

        debug!(
            "Starting degradation prediction for electrolyzer {} with {} history points",
            request.electrolyzer_id,
            request.history_data.len()
        );

        if request.history_data.len() < self.config.min_history_points {
            warn!(
                "Insufficient history data for electrolyzer {}: {}/{} points required",
                request.electrolyzer_id,
                request.history_data.len(),
                self.config.min_history_points
            );
        }

        let features = self.extract_degradation_features(&request.history_data)?;

        let predictions =
            self.run_gaussian_process_regression(&request.history_data, &features)?;

        let (rul, rul_lower, rul_upper) = self.calculate_remaining_useful_life(
            &predictions,
            &features,
            &request.history_data,
        );

        let current_degradation_rate = self.calculate_degradation_rate(&request.history_data);

        let result = DegradationPrediction {
            electrolyzer_id: request.electrolyzer_id,
            timestamp: Utc::now(),
            features,
            predictions,
            remaining_useful_life: rul,
            rul_lower_bound: rul_lower,
            rul_upper_bound: rul_upper,
            current_degradation_rate,
        };

        info!(
            "Degradation prediction complete for electrolyzer {}: RUL={:.1} days, degradation_rate={:.4} V/1000h",
            request.electrolyzer_id, result.remaining_useful_life, result.current_degradation_rate
        );

        Ok(result)
    }

    pub fn extract_degradation_features(
        &self,
        history: &[VoltageCurrentPoint],
    ) -> Result<DegradationFeature, Box<dyn std::error::Error + Send + Sync>> {
        if history.len() < 2 {
            return Err("Insufficient history data for feature extraction".into());
        }

        let sorted_history: Vec<&VoltageCurrentPoint> = {
            let mut v: Vec<&VoltageCurrentPoint> = history.iter().collect();
            v.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
            v
        };

        let voltage_increase_rate = self.calculate_voltage_increase_rate(&sorted_history);
        let efficiency_decay_rate = self.calculate_efficiency_decay_rate(&sorted_history);
        let resistance_increase_rate = self.calculate_resistance_increase_rate(&sorted_history);
        let performance_index = self.calculate_performance_index(&sorted_history);

        let cumulative_operating_hours = self.calculate_cumulative_operating_hours(&sorted_history);
        let total_charge = self.calculate_total_charge(&sorted_history);
        let temperature_cycling_count = self.calculate_temperature_cycling_count(&sorted_history);
        let max_power_pct = self.calculate_max_power_utilization(&sorted_history);

        Ok(DegradationFeature {
            voltage_increase_rate,
            efficiency_decay_rate,
            resistance_increase_rate,
            performance_index,
            cumulative_operating_hours,
            total_charge,
            temperature_cycling_count,
            max_power_pct,
        })
    }

    fn calculate_voltage_increase_rate(&self, history: &[&VoltageCurrentPoint]) -> f64 {
        if history.len() < 2 {
            return 0.0;
        }

        let n = history.len() as f64;
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_xy = 0.0;
        let mut sum_x2 = 0.0;

        let t0 = history.first().unwrap().timestamp.timestamp() as f64;

        for (i, point) in history.iter().enumerate() {
            let t = (point.timestamp.timestamp() as f64 - t0) / 3600.0 / 1000.0;
            let v = point.cell_voltage;

            sum_x += t;
            sum_y += v;
            sum_xy += t * v;
            sum_x2 += t * t;
        }

        let denominator = n * sum_x2 - sum_x * sum_x;
        if denominator.abs() < 1e-10 {
            return 0.0;
        }

        (n * sum_xy - sum_x * sum_y) / denominator
    }

    fn calculate_efficiency_decay_rate(&self, history: &[&VoltageCurrentPoint]) -> f64 {
        if history.len() < 2 {
            return 0.0;
        }

        let n = history.len() as f64;
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_xy = 0.0;
        let mut sum_x2 = 0.0;

        let t0 = history.first().unwrap().timestamp.timestamp() as f64;

        for (i, point) in history.iter().enumerate() {
            let t = (point.timestamp.timestamp() as f64 - t0) / 3600.0 / 1000.0;
            let eff = point.efficiency;

            sum_x += t;
            sum_y += eff;
            sum_xy += t * eff;
            sum_x2 += t * t;
        }

        let denominator = n * sum_x2 - sum_x * sum_x;
        if denominator.abs() < 1e-10 {
            return 0.0;
        }

        -((n * sum_xy - sum_x * sum_y) / denominator)
    }

    fn calculate_resistance_increase_rate(&self, history: &[&VoltageCurrentPoint]) -> f64 {
        if history.len() < 2 {
            return 0.0;
        }

        let n = history.len() as f64;
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_xy = 0.0;
        let mut sum_x2 = 0.0;

        let t0 = history.first().unwrap().timestamp.timestamp() as f64;

        for (i, point) in history.iter().enumerate() {
            let t = (point.timestamp.timestamp() as f64 - t0) / 3600.0 / 1000.0;
            let resistance = if point.current_density > 1e-6 {
                point.cell_voltage / point.current_density
            } else {
                0.0
            };

            sum_x += t;
            sum_y += resistance;
            sum_xy += t * resistance;
            sum_x2 += t * t;
        }

        let denominator = n * sum_x2 - sum_x * sum_x;
        if denominator.abs() < 1e-10 {
            return 0.0;
        }

        (n * sum_xy - sum_x * sum_y) / denominator
    }

    fn calculate_performance_index(&self, history: &[&VoltageCurrentPoint]) -> f64 {
        if history.is_empty() {
            return 0.0;
        }

        let baseline_voltage = 1.8;
        let baseline_efficiency = 75.0;

        let avg_voltage: f64 = history.iter().map(|p| p.cell_voltage).sum::<f64>() / history.len() as f64;
        let avg_efficiency: f64 = history.iter().map(|p| p.efficiency).sum::<f64>() / history.len() as f64;

        let voltage_score = (baseline_voltage / avg_voltage.max(1.0)).min(1.0);
        let efficiency_score = (avg_efficiency / baseline_efficiency).min(1.0);

        (voltage_score + efficiency_score) / 2.0
    }

    fn calculate_cumulative_operating_hours(&self, history: &[&VoltageCurrentPoint]) -> f64 {
        if history.len() < 2 {
            return 0.0;
        }

        let first = history.first().unwrap();
        let last = history.last().unwrap();

        (last.timestamp - first.timestamp).num_seconds() as f64 / 3600.0
    }

    fn calculate_total_charge(&self, history: &[&VoltageCurrentPoint]) -> f64 {
        if history.len() < 2 {
            return 0.0;
        }

        let mut total_charge = 0.0;
        for i in 1..history.len() {
            let dt = (history[i].timestamp - history[i - 1].timestamp).num_seconds() as f64 / 3600.0;
            let avg_current =
                (history[i].current_density + history[i - 1].current_density) / 2.0;
            total_charge += avg_current * dt;
        }

        total_charge
    }

    fn calculate_temperature_cycling_count(&self, history: &[&VoltageCurrentPoint]) -> u32 {
        if history.len() < 3 {
            return 0;
        }

        let mut cycle_count = 0;
        let threshold = 5.0;
        let mut in_cycle = false;
        let mut cycle_start_temp = history[0].temperature;

        for i in 1..history.len() {
            let temp_change = history[i].temperature - cycle_start_temp;
            if !in_cycle && temp_change.abs() > threshold {
                in_cycle = true;
            } else if in_cycle && temp_change.abs() < threshold / 2.0 {
                cycle_count += 1;
                in_cycle = false;
                cycle_start_temp = history[i].temperature;
            }
        }

        cycle_count
    }

    fn calculate_max_power_utilization(&self, history: &[&VoltageCurrentPoint]) -> f64 {
        if history.is_empty() {
            return 0.0;
        }

        let max_current = history
            .iter()
            .map(|p| p.current_density)
            .fold(f64::NEG_INFINITY, f64::max);
        let rated_current = 4.0;

        (max_current / rated_current).min(1.0) * 100.0
    }

    pub fn run_gaussian_process_regression(
        &self,
        history: &[VoltageCurrentPoint],
        features: &DegradationFeature,
    ) -> Result<Vec<GPPredictionPoint>, Box<dyn std::error::Error + Send + Sync>> {
        if history.len() < 2 {
            return Err("Insufficient data for Gaussian Process Regression".into());
        }

        let sorted_history: Vec<&VoltageCurrentPoint> = {
            let mut v: Vec<&VoltageCurrentPoint> = history.iter().collect();
            v.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
            v
        };

        let n = sorted_history.len();
        let prediction_days = self.config.prediction_days as usize;

        let mut x_train = Vec::with_capacity(n);
        let mut y_train = Vec::with_capacity(n);

        let t0 = sorted_history.first().unwrap().timestamp.timestamp() as f64;
        for point in &sorted_history {
            let t = (point.timestamp.timestamp() as f64 - t0) / 86400.0;
            x_train.push(t);
            y_train.push(point.cell_voltage);
        }

        let l = self.config.gp_length_scale;
        let sigma_f = self.config.gp_signal_variance.sqrt();
        let sigma_n = self.config.gp_noise_variance.sqrt();

        let mut k = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in 0..n {
                let dist = (x_train[i] - x_train[j]) / l;
                k[i][j] = sigma_f * sigma_f * (-0.5 * dist * dist).exp();
                if i == j {
                    k[i][j] += sigma_n * sigma_n;
                }
            }
        }

        let k_inv = self.matrix_inverse(&k)?;

        let last_time = *x_train.last().unwrap_or(&0.0);
        let mut predictions = Vec::with_capacity(prediction_days);

        let z_factor = self.calculate_z_factor(self.config.confidence_level);

        for day in 1..=prediction_days {
            let x_star = last_time + day as f64;

            let mut k_star = vec![0.0; n];
            for i in 0..n {
                let dist = (x_star - x_train[i]) / l;
                k_star[i] = sigma_f * sigma_f * (-0.5 * dist * dist).exp();
            }

            let mut k_star_star = sigma_f * sigma_f;

            let mut mu = 0.0;
            for i in 0..n {
                for j in 0..n {
                    mu += k_star[i] * k_inv[i][j] * y_train[j];
                }
            }

            let mut variance = k_star_star;
            for i in 0..n {
                for j in 0..n {
                    variance -= k_star[i] * k_inv[i][j] * k_star[j];
                }
            }
            variance = variance.max(1e-10);

            let std_dev = variance.sqrt();
            let confidence = (1.0 - (std_dev / sigma_f)).max(0.0);

            predictions.push(GPPredictionPoint {
                days_ahead: day as u32,
                predicted_voltage: mu,
                lower_bound: mu - z_factor * std_dev,
                upper_bound: mu + z_factor * std_dev,
                confidence,
            });
        }

        Ok(predictions)
    }

    fn calculate_z_factor(&self, confidence_level: f64) -> f64 {
        let p = (1.0 + confidence_level) / 2.0;
        let a = [2.506628274631, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let b = [
            1.0, -1.861500861646, 1.628345486626, -0.910596513092, 0.256418911442,
            -0.029832859394, 0.001322874058,
        ];

        if p <= 0.5 {
            return 0.0;
        }

        let y = p - 0.5;
        let z = y.sqrt();

        let mut num = 0.0;
        let mut den = 0.0;
        for i in 0..7 {
            num += a[i] * z.powi(i as i32);
            den += b[i] * z.powi(i as i32);
        }

        if p < 0.5 {
            -(num / den)
        } else {
            num / den
        }
    }

    fn matrix_inverse(
        &self,
        matrix: &[Vec<f64>],
    ) -> Result<Vec<Vec<f64>>, Box<dyn std::error::Error + Send + Sync>> {
        let n = matrix.len();
        if n == 0 || matrix[0].len() != n {
            return Err("Invalid matrix dimensions for inversion".into());
        }

        let mut augmented = vec![vec![0.0; 2 * n]; n];
        for i in 0..n {
            for j in 0..n {
                augmented[i][j] = matrix[i][j];
            }
            augmented[i][i + n] = 1.0;
        }

        for col in 0..n {
            let mut max_row = col;
            let mut max_val = augmented[col][col].abs();
            for row in col + 1..n {
                if augmented[row][col].abs() > max_val {
                    max_val = augmented[row][col].abs();
                    max_row = row;
                }
            }

            if max_val < 1e-10 {
                return Err("Matrix is singular or near-singular".into());
            }

            if max_row != col {
                augmented.swap(col, max_row);
            }

            let pivot = augmented[col][col];
            for j in 0..2 * n {
                augmented[col][j] /= pivot;
            }

            for row in 0..n {
                if row != col {
                    let factor = augmented[row][col];
                    for j in 0..2 * n {
                        augmented[row][j] -= factor * augmented[col][j];
                    }
                }
            }
        }

        let mut inverse = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in 0..n {
                inverse[i][j] = augmented[i][j + n];
            }
        }

        Ok(inverse)
    }

    fn calculate_remaining_useful_life(
        &self,
        predictions: &[GPPredictionPoint],
        features: &DegradationFeature,
        history: &[VoltageCurrentPoint],
    ) -> (f64, f64, f64) {
        let voltage_threshold = self.config.voltage_failure_threshold;
        let efficiency_threshold = self.config.efficiency_failure_threshold;

        let mut rul_voltage = self.config.prediction_days as f64;
        let mut voltage_lower = self.config.prediction_days as f64;
        let mut voltage_upper = self.config.prediction_days as f64;

        for pred in predictions {
            if pred.predicted_voltage >= voltage_threshold {
                rul_voltage = pred.days_ahead as f64;
                voltage_lower = pred.days_ahead as f64
                    - (pred.predicted_voltage - voltage_threshold)
                        / (pred.predicted_voltage - pred.lower_bound).max(1e-6)
                        * pred.days_ahead as f64;
                voltage_upper = pred.days_ahead as f64
                    + (pred.upper_bound - pred.predicted_voltage)
                        / (pred.upper_bound - pred.predicted_voltage).max(1e-6)
                        * pred.days_ahead as f64;
                break;
            }
        }

        let avg_efficiency: f64 = if !history.is_empty() {
            history.iter().map(|p| p.efficiency).sum::<f64>() / history.len() as f64
        } else {
            75.0
        };

        let efficiency_degradation_rate = features.efficiency_decay_rate;
        let rul_efficiency = if efficiency_degradation_rate > 0.0 {
            ((avg_efficiency - efficiency_threshold) / efficiency_degradation_rate).max(0.0)
        } else {
            self.config.prediction_days as f64
        };

        let rul = rul_voltage.min(rul_efficiency);
        let rul_lower = voltage_lower.min(rul_efficiency * 0.8);
        let rul_upper = voltage_upper.max(rul_efficiency * 1.2);

        let performance_factor = features.performance_index;
        let adjusted_rul = rul * (0.5 + 0.5 * performance_factor);
        let adjusted_lower = rul_lower * (0.5 + 0.5 * performance_factor);
        let adjusted_upper = rul_upper * (0.5 + 0.5 * performance_factor);

        (
            adjusted_rul.max(0.0),
            adjusted_lower.max(0.0),
            adjusted_upper.max(adjusted_rul),
        )
    }

    fn calculate_degradation_rate(&self, history: &[VoltageCurrentPoint]) -> f64 {
        if history.len() < 2 {
            return 0.0;
        }

        let sorted_history: Vec<&VoltageCurrentPoint> = {
            let mut v: Vec<&VoltageCurrentPoint> = history.iter().collect();
            v.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
            v
        };

        let first = sorted_history.first().unwrap();
        let last = sorted_history.last().unwrap();

        let delta_v = last.cell_voltage - first.cell_voltage;
        let delta_t = (last.timestamp - first.timestamp).num_seconds() as f64 / 3600.0 / 1000.0;

        if delta_t.abs() < 1e-6 {
            0.0
        } else {
            delta_v / delta_t
        }
    }

    pub fn generate_maintenance_plan(
        predictions: &[DegradationPrediction],
    ) -> Result<MaintenancePlan, Box<dyn std::error::Error + Send + Sync>> {
        let mut items: Vec<MaintenancePlanItem> = Vec::new();

        for pred in predictions {
            let severity = if pred.remaining_useful_life < 15.0 {
                DegradationSeverity::Critical
            } else if pred.remaining_useful_life < 30.0 {
                DegradationSeverity::High
            } else if pred.remaining_useful_life < 60.0 {
                DegradationSeverity::Medium
            } else {
                DegradationSeverity::Low
            };

            let predicted_failure_date =
                Utc::now() + Duration::days(pred.remaining_useful_life.round() as i64);
            let recommended_maintenance_date = Utc::now()
                + Duration::days((pred.remaining_useful_life * 0.7).round() as i64);

            let (maintenance_type, description, estimated_cost) = match severity {
                DegradationSeverity::Critical => (
                    "紧急更换".to_string(),
                    "膜电极严重退化，建议立即停机更换".to_string(),
                    50000.0,
                ),
                DegradationSeverity::High => (
                    "大修".to_string(),
                    "性能显著下降，建议安排大修更换".to_string(),
                    35000.0,
                ),
                DegradationSeverity::Medium => (
                    "预防性维护".to_string(),
                    "中度退化，建议近期安排预防性检查".to_string(),
                    15000.0,
                ),
                DegradationSeverity::Low => (
                    "常规检查".to_string(),
                    "轻微退化，继续常规监测".to_string(),
                    2000.0,
                ),
            };

            items.push(MaintenancePlanItem {
                electrolyzer_id: pred.electrolyzer_id,
                priority: severity,
                predicted_failure_date,
                remaining_useful_life: pred.remaining_useful_life,
                recommended_maintenance_date,
                estimated_cost,
                maintenance_type,
                description,
            });
        }

        let severity_order = |s: &DegradationSeverity| match s {
            DegradationSeverity::Critical => 0,
            DegradationSeverity::High => 1,
            DegradationSeverity::Medium => 2,
            DegradationSeverity::Low => 3,
        };

        items.sort_by(|a, b| {
            severity_order(&a.priority)
                .cmp(&severity_order(&b.priority))
                .then(a.remaining_useful_life.partial_cmp(&b.remaining_useful_life).unwrap())
        });

        let total_estimated_cost = items.iter().map(|i| i.estimated_cost).sum();

        Ok(MaintenancePlan {
            timestamp: Utc::now(),
            items,
            total_estimated_cost,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use chrono::Duration;

    fn create_test_config() -> DegradationPredictionConfig {
        DegradationPredictionConfig {
            gp_length_scale: 30.0,
            gp_signal_variance: 0.01,
            gp_noise_variance: 1e-4,
            prediction_days: 90,
            confidence_level: 0.95,
            min_history_points: 10,
            voltage_failure_threshold: 2.2,
            efficiency_failure_threshold: 65.0,
            prediction_interval_secs: 3600,
            max_concurrent_predictions: 2,
        }
    }

    fn generate_test_history(
        days: u32,
        points_per_day: u32,
        degradation_rate: f64,
        noise: f64,
    ) -> Vec<VoltageCurrentPoint> {
        let n = (days * points_per_day) as usize;
        let base_voltage = 1.8;
        let base_current = 2.0;
        let base_efficiency = 75.0;
        let base_temp = 60.0;

        (0..n)
            .map(|i| {
                let t = Utc::now() - Duration::days((n - i - 1) as i64 / points_per_day as i64);
                let day_frac = i as f64 / points_per_day as f64;
                let voltage_drift = degradation_rate * day_frac / 1000.0;

                VoltageCurrentPoint {
                    timestamp: t,
                    current_density: base_current + (rand::random::<f64>() - 0.5) * 0.1,
                    cell_voltage: base_voltage
                        + voltage_drift
                        + if noise > 0.0 {
                            (rand::random::<f64>() - 0.5) * noise
                        } else {
                            0.0
                        },
                    efficiency: base_efficiency
                        - degradation_rate * day_frac * 0.5
                        + if noise > 0.0 {
                            (rand::random::<f64>() - 0.5) * noise * 5.0
                        } else {
                            0.0
                        },
                    temperature: base_temp
                        + if noise > 0.0 {
                            (rand::random::<f64>() - 0.5) * noise * 20.0
                        } else {
                            0.0
                        },
                }
            })
            .collect()
    }

    #[test]
    fn test_extract_degradation_features_normal() {
        let config = create_test_config();
        let (predictor, _) = DegradationPredictor::new(config);

        let history = generate_test_history(60, 24, 0.01, 0.001);
        let features = predictor.extract_degradation_features(&history).unwrap();

        assert!(features.voltage_increase_rate > 0.0);
        assert!(features.voltage_increase_rate < 0.1);
        assert!(features.efficiency_decay_rate >= 0.0);
        assert!(features.performance_index > 0.0);
        assert!(features.performance_index <= 1.0);
        assert!(features.cumulative_operating_hours > 0.0);
        assert!(features.total_charge > 0.0);
        assert!(features.max_power_pct > 0.0);
        assert!(features.max_power_pct <= 100.0);
    }

    #[test]
    fn test_extract_degradation_features_insufficient_data() {
        let config = create_test_config();
        let (predictor, _) = DegradationPredictor::new(config);

        let history = generate_test_history(1, 1, 0.01, 0.0);
        let result = predictor.extract_degradation_features(&history);

        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_voltage_increase_rate_linear() {
        let config = create_test_config();
        let (predictor, _) = DegradationPredictor::new(config);

        let history: Vec<VoltageCurrentPoint> = (0..100)
            .map(|i| {
                let t = Utc::now() - Duration::hours((99 - i) as i64 * 100);
                VoltageCurrentPoint {
                    timestamp: t,
                    current_density: 2.0,
                    cell_voltage: 1.8 + i as f64 * 0.001,
                    efficiency: 75.0,
                    temperature: 60.0,
                }
            })
            .collect();

        let sorted: Vec<&VoltageCurrentPoint> = history.iter().collect();
        let rate = predictor.calculate_voltage_increase_rate(&sorted);

        assert!(rate > 0.0);
        assert_relative_eq!(rate, 0.001, epsilon = 0.0001);
    }

    #[test]
    fn test_calculate_performance_index_high() {
        let config = create_test_config();
        let (predictor, _) = DegradationPredictor::new(config);

        let history: Vec<VoltageCurrentPoint> = (0..10)
            .map(|i| {
                let t = Utc::now() - Duration::hours(i as i64);
                VoltageCurrentPoint {
                    timestamp: t,
                    current_density: 2.0,
                    cell_voltage: 1.75,
                    efficiency: 78.0,
                    temperature: 60.0,
                }
            })
            .collect();

        let sorted: Vec<&VoltageCurrentPoint> = history.iter().collect();
        let index = predictor.calculate_performance_index(&sorted);

        assert!(index > 0.9);
        assert!(index <= 1.0);
    }

    #[test]
    fn test_calculate_performance_index_low() {
        let config = create_test_config();
        let (predictor, _) = DegradationPredictor::new(config);

        let history: Vec<VoltageCurrentPoint> = (0..10)
            .map(|i| {
                let t = Utc::now() - Duration::hours(i as i64);
                VoltageCurrentPoint {
                    timestamp: t,
                    current_density: 2.0,
                    cell_voltage: 2.1,
                    efficiency: 65.0,
                    temperature: 60.0,
                }
            })
            .collect();

        let sorted: Vec<&VoltageCurrentPoint> = history.iter().collect();
        let index = predictor.calculate_performance_index(&sorted);

        assert!(index < 0.9);
        assert!(index > 0.0);
    }

    #[test]
    fn test_calculate_cumulative_operating_hours() {
        let config = create_test_config();
        let (predictor, _) = DegradationPredictor::new(config);

        let history: Vec<VoltageCurrentPoint> = (0..24)
            .map(|i| {
                let t = Utc::now() - Duration::hours((23 - i) as i64);
                VoltageCurrentPoint {
                    timestamp: t,
                    current_density: 2.0,
                    cell_voltage: 1.8,
                    efficiency: 75.0,
                    temperature: 60.0,
                }
            })
            .collect();

        let sorted: Vec<&VoltageCurrentPoint> = history.iter().collect();
        let hours = predictor.calculate_cumulative_operating_hours(&sorted);

        assert_relative_eq!(hours, 23.0, epsilon = 0.1);
    }

    #[test]
    fn test_calculate_total_charge() {
        let config = create_test_config();
        let (predictor, _) = DegradationPredictor::new(config);

        let history: Vec<VoltageCurrentPoint> = (0..10)
            .map(|i| {
                let t = Utc::now() - Duration::hours((9 - i) as i64);
                VoltageCurrentPoint {
                    timestamp: t,
                    current_density: 2.0,
                    cell_voltage: 1.8,
                    efficiency: 75.0,
                    temperature: 60.0,
                }
            })
            .collect();

        let sorted: Vec<&VoltageCurrentPoint> = history.iter().collect();
        let charge = predictor.calculate_total_charge(&sorted);

        assert_relative_eq!(charge, 18.0, epsilon = 0.1);
    }

    #[test]
    fn test_calculate_max_power_utilization() {
        let config = create_test_config();
        let (predictor, _) = DegradationPredictor::new(config);

        let history: Vec<VoltageCurrentPoint> = vec![
            VoltageCurrentPoint {
                timestamp: Utc::now(),
                current_density: 3.0,
                cell_voltage: 1.8,
                efficiency: 75.0,
                temperature: 60.0,
            },
            VoltageCurrentPoint {
                timestamp: Utc::now(),
                current_density: 4.0,
                cell_voltage: 1.8,
                efficiency: 75.0,
                temperature: 60.0,
            },
            VoltageCurrentPoint {
                timestamp: Utc::now(),
                current_density: 2.0,
                cell_voltage: 1.8,
                efficiency: 75.0,
                temperature: 60.0,
            },
        ];

        let sorted: Vec<&VoltageCurrentPoint> = history.iter().collect();
        let utilization = predictor.calculate_max_power_utilization(&sorted);

        assert_relative_eq!(utilization, 100.0, epsilon = 1.0);
    }

    #[test]
    fn test_matrix_inverse_identity() {
        let config = create_test_config();
        let (predictor, _) = DegradationPredictor::new(config);

        let matrix = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let inverse = predictor.matrix_inverse(&matrix).unwrap();

        assert_relative_eq!(inverse[0][0], 1.0, epsilon = 1e-6);
        assert_relative_eq!(inverse[0][1], 0.0, epsilon = 1e-6);
        assert_relative_eq!(inverse[1][0], 0.0, epsilon = 1e-6);
        assert_relative_eq!(inverse[1][1], 1.0, epsilon = 1e-6);
    }

    #[test]
    fn test_matrix_inverse_simple() {
        let config = create_test_config();
        let (predictor, _) = DegradationPredictor::new(config);

        let matrix = vec![vec![4.0, 7.0], vec![2.0, 6.0]];
        let inverse = predictor.matrix_inverse(&matrix).unwrap();

        assert_relative_eq!(inverse[0][0], 0.6, epsilon = 0.01);
        assert_relative_eq!(inverse[0][1], -0.7, epsilon = 0.01);
        assert_relative_eq!(inverse[1][0], -0.2, epsilon = 0.01);
        assert_relative_eq!(inverse[1][1], 0.4, epsilon = 0.01);
    }

    #[test]
    fn test_matrix_inverse_singular() {
        let config = create_test_config();
        let (predictor, _) = DegradationPredictor::new(config);

        let matrix = vec![vec![1.0, 2.0], vec![2.0, 4.0]];
        let result = predictor.matrix_inverse(&matrix);

        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_z_factor_95() {
        let config = create_test_config();
        let (predictor, _) = DegradationPredictor::new(config);

        let z = predictor.calculate_z_factor(0.95);
        assert!(z > 1.6 && z < 2.0);
    }

    #[test]
    fn test_calculate_z_factor_99() {
        let config = create_test_config();
        let (predictor, _) = DegradationPredictor::new(config);

        let z = predictor.calculate_z_factor(0.99);
        assert!(z > 2.3 && z < 2.7);
    }

    #[test]
    fn test_run_gaussian_process_regression_monotonic() {
        let config = create_test_config();
        let (predictor, _) = DegradationPredictor::new(config.clone());

        let history = generate_test_history(30, 24, 0.005, 0.0005);
        let features = predictor.extract_degradation_features(&history).unwrap();
        let predictions = predictor
            .run_gaussian_process_regression(&history, &features)
            .unwrap();

        assert_eq!(predictions.len(), config.prediction_days as usize);

        for i in 1..predictions.len() {
            assert!(predictions[i].predicted_voltage >= predictions[i - 1].predicted_voltage - 0.001);
        }

        for pred in &predictions {
            assert!(pred.upper_bound > pred.predicted_voltage);
            assert!(pred.lower_bound < pred.predicted_voltage);
            assert!(pred.confidence >= 0.0 && pred.confidence <= 1.0);
        }
    }

    #[test]
    fn test_run_gaussian_process_regression_insufficient_data() {
        let config = create_test_config();
        let (predictor, _) = DegradationPredictor::new(config);

        let history = vec![VoltageCurrentPoint {
            timestamp: Utc::now(),
            current_density: 2.0,
            cell_voltage: 1.8,
            efficiency: 75.0,
            temperature: 60.0,
        }];

        let features = DegradationFeature {
            voltage_increase_rate: 0.0,
            efficiency_decay_rate: 0.0,
            resistance_increase_rate: 0.0,
            performance_index: 1.0,
            cumulative_operating_hours: 0.0,
            total_charge: 0.0,
            temperature_cycling_count: 0,
            max_power_pct: 50.0,
        };

        let result = predictor.run_gaussian_process_regression(&history, &features);
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_remaining_useful_life_short() {
        let config = create_test_config();
        let (predictor, _) = DegradationPredictor::new(config);

        let history = generate_test_history(60, 24, 0.1, 0.001);
        let features = predictor.extract_degradation_features(&history).unwrap();
        let predictions = predictor
            .run_gaussian_process_regression(&history, &features)
            .unwrap();

        let (rul, lower, upper) =
            predictor.calculate_remaining_useful_life(&predictions, &features, &history);

        assert!(rul > 0.0);
        assert!(rul <= config.prediction_days as f64);
        assert!(lower <= rul);
        assert!(upper >= rul);
        assert!(upper - lower > 0.0);
    }

    #[test]
    fn test_calculate_remaining_useful_life_long() {
        let config = create_test_config();
        let (predictor, _) = DegradationPredictor::new(config);

        let history = generate_test_history(30, 24, 0.001, 0.001);
        let features = predictor.extract_degradation_features(&history).unwrap();
        let predictions = predictor
            .run_gaussian_process_regression(&history, &features)
            .unwrap();

        let (rul, lower, upper) =
            predictor.calculate_remaining_useful_life(&predictions, &features, &history);

        assert!(rul > 30.0);
        assert!(lower <= rul);
        assert!(upper >= rul);
    }

    #[test]
    fn test_calculate_degradation_rate() {
        let config = create_test_config();
        let (predictor, _) = DegradationPredictor::new(config);

        let history = generate_test_history(30, 24, 0.01, 0.0);
        let rate = predictor.calculate_degradation_rate(&history);

        assert!(rate > 0.0);
        assert!(rate < 0.1);
    }

    #[test]
    fn test_calculate_degradation_rate_zero() {
        let config = create_test_config();
        let (predictor, _) = DegradationPredictor::new(config);

        let history: Vec<VoltageCurrentPoint> = (0..10)
            .map(|i| {
                let t = Utc::now() - Duration::hours((9 - i) as i64);
                VoltageCurrentPoint {
                    timestamp: t,
                    current_density: 2.0,
                    cell_voltage: 1.8,
                    efficiency: 75.0,
                    temperature: 60.0,
                }
            })
            .collect();

        let rate = predictor.calculate_degradation_rate(&history);
        assert_relative_eq!(rate, 0.0, epsilon = 1e-6);
    }

    #[tokio::test]
    async fn test_predict_degradation_full_pipeline() {
        let config = create_test_config();
        let (predictor, _rx) = DegradationPredictor::new(config.clone());

        let history = generate_test_history(60, 24, 0.005, 0.001);

        let request = DegradationPredictionRequest {
            electrolyzer_id: 1,
            history,
        };

        let result = predictor.predict_degradation(request).await.unwrap();

        assert_eq!(result.electrolyzer_id, 1);
        assert_eq!(result.predictions.len(), config.prediction_days as usize);
        assert!(result.remaining_useful_life > 0.0);
        assert!(result.rul_lower_bound <= result.remaining_useful_life);
        assert!(result.rul_upper_bound >= result.remaining_useful_life);
        assert!(result.current_degradation_rate >= 0.0);
        assert!(result.predictions.last().unwrap().predicted_voltage > 1.8);
    }

    #[test]
    fn test_generate_maintenance_plan_sorting() {
        let now = Utc::now();

        let pred1 = DegradationPrediction {
            electrolyzer_id: 1,
            timestamp: now,
            features: DegradationFeature {
                voltage_increase_rate: 0.001,
                efficiency_decay_rate: 0.001,
                resistance_increase_rate: 0.001,
                performance_index: 0.3,
                cumulative_operating_hours: 1000.0,
                total_charge: 5000.0,
                temperature_cycling_count: 10,
                max_power_pct: 80.0,
            },
            predictions: Vec::new(),
            remaining_useful_life: 10.0,
            rul_lower_bound: 5.0,
            rul_upper_bound: 15.0,
            current_degradation_rate: 0.01,
        };

        let pred2 = DegradationPrediction {
            electrolyzer_id: 2,
            timestamp: now,
            features: DegradationFeature {
                voltage_increase_rate: 0.0005,
                efficiency_decay_rate: 0.0005,
                resistance_increase_rate: 0.0005,
                performance_index: 0.8,
                cumulative_operating_hours: 500.0,
                total_charge: 2500.0,
                temperature_cycling_count: 5,
                max_power_pct: 60.0,
            },
            predictions: Vec::new(),
            remaining_useful_life: 50.0,
            rul_lower_bound: 40.0,
            rul_upper_bound: 60.0,
            current_degradation_rate: 0.005,
        };

        let pred3 = DegradationPrediction {
            electrolyzer_id: 3,
            timestamp: now,
            features: DegradationFeature {
                voltage_increase_rate: 0.002,
                efficiency_decay_rate: 0.002,
                resistance_increase_rate: 0.002,
                performance_index: 0.1,
                cumulative_operating_hours: 2000.0,
                total_charge: 10000.0,
                temperature_cycling_count: 20,
                max_power_pct: 90.0,
            },
            predictions: Vec::new(),
            remaining_useful_life: 5.0,
            rul_lower_bound: 2.0,
            rul_upper_bound: 8.0,
            current_degradation_rate: 0.02,
        };

        let plan = DegradationPredictor::generate_maintenance_plan(&[pred1, pred2, pred3]).unwrap();

        assert_eq!(plan.items.len(), 3);
        assert_eq!(plan.items[0].electrolyzer_id, 3);
        assert_eq!(plan.items[0].priority, DegradationSeverity::Critical);
        assert_eq!(plan.items[1].electrolyzer_id, 1);
        assert_eq!(plan.items[1].priority, DegradationSeverity::High);
        assert_eq!(plan.items[2].electrolyzer_id, 2);
        assert_eq!(plan.items[2].priority, DegradationSeverity::Low);

        assert!(plan.total_estimated_cost > 0.0);
        assert!(plan.items[0].estimated_cost > plan.items[1].estimated_cost);
        assert!(plan.items[1].estimated_cost > plan.items[2].estimated_cost);
    }

    #[test]
    fn test_generate_maintenance_plan_empty() {
        let plan = DegradationPredictor::generate_maintenance_plan(&[]).unwrap();

        assert!(plan.items.is_empty());
        assert_relative_eq!(plan.total_estimated_cost, 0.0, epsilon = 1e-6);
    }

    #[test]
    fn test_calculate_temperature_cycling_count() {
        let config = create_test_config();
        let (predictor, _) = DegradationPredictor::new(config);

        let mut history: Vec<VoltageCurrentPoint> = Vec::new();
        let base_temp = 60.0;
        for i in 0..100 {
            let cycle_phase = (i / 10) % 2;
            let temp = if cycle_phase == 0 {
                base_temp + (i % 10) as f64
            } else {
                base_temp + 9.0 - (i % 10) as f64
            };

            history.push(VoltageCurrentPoint {
                timestamp: Utc::now() - Duration::minutes((99 - i) as i64 * 10),
                current_density: 2.0,
                cell_voltage: 1.8,
                efficiency: 75.0,
                temperature: temp,
            });
        }

        let sorted: Vec<&VoltageCurrentPoint> = history.iter().collect();
        let cycles = predictor.calculate_temperature_cycling_count(&sorted);

        assert!(cycles >= 4);
        assert!(cycles <= 6);
    }

    #[tokio::test]
    async fn test_concurrent_predictions() {
        let config = create_test_config();
        let (predictor, _rx) = DegradationPredictor::new(config);
        let predictor_arc = std::sync::Arc::new(predictor);

        let mut handles = Vec::new();
        for i in 0..3 {
            let p = predictor_arc.clone();
            let handle = tokio::spawn(async move {
                let history = generate_test_history(30, 24, 0.005 + i as f64 * 0.001, 0.001);
                let request = DegradationPredictionRequest {
                    electrolyzer_id: i + 1,
                    history,
                };
                p.predict_degradation(request).await
            });
            handles.push(handle);
        }

        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }
    }
}
