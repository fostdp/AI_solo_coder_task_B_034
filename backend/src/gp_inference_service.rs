use tokio::sync::{mpsc, oneshot};
use tracing::{debug, warn};

use crate::config::DegradationPredictionConfig;
use crate::models::*;

#[derive(Debug)]
pub struct GPInferenceRequest {
    pub history: Vec<VoltageCurrentPoint>,
    pub bayesian_prior: Option<BayesianPrior>,
    pub transfer_info: Option<TransferLearningInfo>,
    pub response_tx: oneshot::Sender<Result<Vec<GPPredictionPoint>, String>>,
}

#[derive(Debug, Clone)]
pub struct GPInferenceHandle {
    pub tx: mpsc::Sender<GPInferenceRequest>,
}

impl GPInferenceHandle {
    pub fn new(tx: mpsc::Sender<GPInferenceRequest>) -> Self {
        Self { tx }
    }

    pub async fn infer(
        &self,
        history: Vec<VoltageCurrentPoint>,
        bayesian_prior: Option<BayesianPrior>,
        transfer_info: Option<TransferLearningInfo>,
    ) -> Result<Vec<GPPredictionPoint>, Box<dyn std::error::Error + Send + Sync>> {
        let (resp_tx, resp_rx) = oneshot::channel();

        let request = GPInferenceRequest {
            history,
            bayesian_prior,
            transfer_info,
            response_tx: resp_tx,
        };

        self.tx
            .send(request)
            .await
            .map_err(|e| format!("Failed to send GP inference request: {}", e))?;

        resp_rx
            .await
            .map_err(|e| format!("GP inference service dropped: {}", e))?
            .map_err(|e| e.into())
    }
}

pub struct GPInferenceService {
    config: DegradationPredictionConfig,
    rx: mpsc::Receiver<GPInferenceRequest>,
}

impl GPInferenceService {
    pub fn new(
        config: DegradationPredictionConfig,
        rx: mpsc::Receiver<GPInferenceRequest>,
    ) -> Self {
        Self { config, rx }
    }

    pub async fn run(mut self) {
        debug!("GP inference service started");
        while let Some(req) = self.rx.recv().await {
            let result = self.run_gaussian_process_regression(
                &req.history,
                req.bayesian_prior.as_ref(),
                req.transfer_info.as_ref(),
            );
            if req.response_tx.send(result).is_err() {
                warn!("GP inference service: receiver dropped before sending result");
            }
        }
        debug!("GP inference service shutting down");
    }

    fn run_gaussian_process_regression(
        &self,
        history: &[VoltageCurrentPoint],
        bayesian_prior: Option<&BayesianPrior>,
        transfer_info: Option<&TransferLearningInfo>,
    ) -> Result<Vec<GPPredictionPoint>, String> {
        if history.len() < 2 {
            return Err("Insufficient data for Gaussian Process Regression".to_string());
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

        let mut y_adjusted = y_train.clone();
        if let Some(prior) = bayesian_prior {
            let prior_weight = prior.strength / (prior.strength + n as f64);
            for i in 0..n {
                let prior_mean = 1.8 + prior.mean_voltage_rate * x_train[i] / 1000.0;
                y_adjusted[i] = y_train[i] * (1.0 - prior_weight) + prior_mean * prior_weight;
            }
        }

        if let Some(transfer) = transfer_info {
            let transfer_weight =
                transfer.weight * (1.0 - n as f64 / self.config.min_history_points as f64).max(0.0);
            for i in 0..n {
                let transfer_mean = 1.8 + transfer.adjusted_voltage_rate * x_train[i] / 1000.0;
                y_adjusted[i] = y_adjusted[i] * (1.0 - transfer_weight) + transfer_mean * transfer_weight;
            }
        }

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

        let k_inv = self.matrix_inverse(&k).map_err(|e| e.to_string())?;

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

            let k_star_star = sigma_f * sigma_f;

            let mut mu = 0.0;
            for i in 0..n {
                for j in 0..n {
                    mu += k_star[i] * k_inv[i][j] * y_adjusted[j];
                }
            }

            let mut variance = k_star_star;
            for i in 0..n {
                for j in 0..n {
                    variance -= k_star[i] * k_inv[i][j] * k_star[j];
                }
            }

            if let Some(prior) = bayesian_prior {
                let prior_variance = prior.std_voltage_rate * prior.std_voltage_rate;
                variance = variance * (1.0 - 1.0 / (prior.strength + 1.0))
                    + prior_variance / (prior.strength + 1.0);
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

    fn matrix_inverse(&self, matrix: &[Vec<f64>]) -> Result<Vec<Vec<f64>>, String> {
        let n = matrix.len();
        if n == 0 || matrix[0].len() != n {
            return Err("Matrix must be square and non-empty".to_string());
        }

        let mut aug = vec![vec![0.0; 2 * n]; n];
        for i in 0..n {
            for j in 0..n {
                aug[i][j] = matrix[i][j];
            }
            aug[i][i + n] = 1.0;
        }

        for col in 0..n {
            let mut max_row = col;
            for row in (col + 1)..n {
                if aug[row][col].abs() > aug[max_row][col].abs() {
                    max_row = row;
                }
            }

            if aug[max_row][col].abs() < 1e-10 {
                return Err("Matrix is singular".to_string());
            }

            aug.swap(col, max_row);

            let pivot = aug[col][col];
            for j in 0..2 * n {
                aug[col][j] /= pivot;
            }

            for row in 0..n {
                if row != col {
                    let factor = aug[row][col];
                    for j in 0..2 * n {
                        aug[row][j] -= factor * aug[col][j];
                    }
                }
            }
        }

        let mut inv = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in 0..n {
                inv[i][j] = aug[i][j + n];
            }
        }

        Ok(inv)
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
        let mut z = y;
        let mut num = 0.0;
        let mut den = 0.0;

        for i in 0..7 {
            num += a[i] * z.powi(i as i32);
            den += b[i] * z.powi(i as i32);
        }

        num / den
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DegradationPredictionConfig;
    use chrono::Utc;

    fn create_test_config() -> DegradationPredictionConfig {
        DegradationPredictionConfig {
            prediction_days: 90,
            min_history_points: 30,
            max_concurrent_predictions: 4,
            gp_length_scale: 30.0,
            gp_signal_variance: 0.01,
            gp_noise_variance: 0.001,
            confidence_level: 0.95,
            rul_threshold_voltage: 2.2,
            bayesian_prior_enabled: true,
            prior_mean_voltage_rate: 0.005,
            prior_std_voltage_rate: 0.002,
            transfer_learning_enabled: true,
            transfer_weight: 0.3,
            min_transfer_samples: 10,
            prior_strength: 10.0,
            divergence_threshold: 0.01,
        }
    }

    fn create_test_history(n: usize) -> Vec<VoltageCurrentPoint> {
        let mut history = Vec::with_capacity(n);
        let now = Utc::now();
        for i in 0..n {
            history.push(VoltageCurrentPoint {
                timestamp: now - chrono::Duration::days((n - i) as i64),
                cell_voltage: 1.8 + i as f64 * 0.001,
                current_density: 1.0,
                temperature: 60.0,
                efficiency: 0.75,
            });
        }
        history
    }

    #[test]
    fn test_gp_inference_basic() {
        let config = create_test_config();
        let (tx, rx) = mpsc::channel::<GPInferenceRequest>(10);
        let handle = GPInferenceHandle::new(tx);
        let service = GPInferenceService::new(config, rx);

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            tokio::spawn(async move {
                service.run().await;
            });

            let history = create_test_history(30);
            let result = handle.infer(history, None, None).await;

            assert!(result.is_ok());
            let predictions = result.unwrap();
            assert_eq!(predictions.len(), 90);
        });
    }

    #[test]
    fn test_gp_inference_with_prior() {
        let config = create_test_config();
        let (tx, rx) = mpsc::channel::<GPInferenceRequest>(10);
        let handle = GPInferenceHandle::new(tx);
        let service = GPInferenceService::new(config.clone(), rx);

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            tokio::spawn(async move {
                service.run().await;
            });

            let history = create_test_history(10);
            let prior = BayesianPrior {
                mean_voltage_rate: 0.005,
                std_voltage_rate: 0.002,
                strength: 10.0,
            };
            let result = handle.infer(history, Some(prior), None).await;

            assert!(result.is_ok());
            let predictions = result.unwrap();
            assert_eq!(predictions.len(), 90);
            assert!(predictions[0].predicted_voltage > 1.8);
        });
    }

    #[test]
    fn test_gp_inference_insufficient_data() {
        let config = create_test_config();
        let (tx, rx) = mpsc::channel::<GPInferenceRequest>(10);
        let handle = GPInferenceHandle::new(tx);
        let service = GPInferenceService::new(config, rx);

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            tokio::spawn(async move {
                service.run().await;
            });

            let history = create_test_history(1);
            let result = handle.infer(history, None, None).await;

            assert!(result.is_err());
        });
    }

    #[test]
    fn test_matrix_inverse_2x2() {
        let config = create_test_config();
        let (_, rx) = mpsc::channel::<GPInferenceRequest>(10);
        let service = GPInferenceService::new(config, rx);

        let matrix = vec![vec![4.0, 1.0], vec![2.0, 3.0]];
        let inv = service.matrix_inverse(&matrix).unwrap();

        assert_eq!(inv.len(), 2);
        assert!((inv[0][0] - 0.3).abs() < 0.01);
        assert!((inv[0][1] + 0.1).abs() < 0.01);
        assert!((inv[1][0] + 0.2).abs() < 0.01);
        assert!((inv[1][1] - 0.4).abs() < 0.01);
    }

    #[test]
    fn test_z_factor_95_confidence() {
        let config = create_test_config();
        let (_, rx) = mpsc::channel::<GPInferenceRequest>(10);
        let service = GPInferenceService::new(config, rx);

        let z = service.calculate_z_factor(0.95);
        assert!(z > 1.6);
        assert!(z < 2.0);
    }

    #[tokio::test]
    async fn test_gp_service_channel_communication() {
        let config = create_test_config();
        let (tx, rx) = mpsc::channel::<GPInferenceRequest>(10);
        let handle = GPInferenceHandle::new(tx);
        let service = GPInferenceService::new(config, rx);

        tokio::spawn(async move {
            service.run().await;
        });

        let history = create_test_history(30);
        let result = handle.infer(history, None, None).await;

        assert!(result.is_ok());
        let predictions = result.unwrap();
        assert_eq!(predictions.len(), 90);
        assert!(predictions[0].confidence > 0.0);
        assert!(predictions[0].lower_bound < predictions[0].predicted_voltage);
        assert!(predictions[0].upper_bound > predictions[0].predicted_voltage);
    }

    #[tokio::test]
    async fn test_gp_service_concurrent_inferences() {
        let config = create_test_config();
        let (tx, rx) = mpsc::channel::<GPInferenceRequest>(20);
        let handle = GPInferenceHandle::new(tx);
        let service = GPInferenceService::new(config, rx);

        tokio::spawn(async move {
            service.run().await;
        });

        let mut handles = Vec::new();
        for i in 0..5 {
            let handle_clone = handle.clone();
            let h = tokio::spawn(async move {
                let history = create_test_history(20 + i * 5);
                handle_clone.infer(history, None, None).await
            });
            handles.push(h);
        }

        for h in handles {
            let result = h.await.unwrap();
            assert!(result.is_ok());
            let predictions = result.unwrap();
            assert_eq!(predictions.len(), 90);
        }
    }

    #[tokio::test]
    async fn test_gp_service_with_transfer_learning() {
        let config = create_test_config();
        let (tx, rx) = mpsc::channel::<GPInferenceRequest>(10);
        let handle = GPInferenceHandle::new(tx);
        let service = GPInferenceService::new(config, rx);

        tokio::spawn(async move {
            service.run().await;
        });

        let history = create_test_history(10);
        let transfer_info = TransferLearningInfo {
            adjusted_voltage_rate: 0.005,
            weight: 0.5,
            source_count: 3,
        };
        let result = handle.infer(history, None, Some(transfer_info)).await;

        assert!(result.is_ok());
        let predictions = result.unwrap();
        assert_eq!(predictions.len(), 90);
        assert!(predictions[0].predicted_voltage > 1.8);
    }

    #[test]
    fn test_matrix_inverse_singular() {
        let config = create_test_config();
        let (_, rx) = mpsc::channel::<GPInferenceRequest>(10);
        let service = GPInferenceService::new(config, rx);

        let singular_matrix = vec![vec![1.0, 2.0], vec![2.0, 4.0]];
        let result = service.matrix_inverse(&singular_matrix);
        assert!(result.is_err());
    }

    #[test]
    fn test_z_factor_various_confidence_levels() {
        let config = create_test_config();
        let (_, rx) = mpsc::channel::<GPInferenceRequest>(10);
        let service = GPInferenceService::new(config, rx);

        let z_90 = service.calculate_z_factor(0.90);
        let z_95 = service.calculate_z_factor(0.95);
        let z_99 = service.calculate_z_factor(0.99);

        assert!(z_90 < z_95);
        assert!(z_95 < z_99);
        assert!(z_90 > 1.2);
        assert!(z_99 > 2.3);
    }
}
