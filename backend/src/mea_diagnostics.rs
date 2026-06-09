use chrono::Utc;
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::config::MeaDiagnosticsConfig;
use crate::models::*;

pub type MEADiagnostics = MEADiagnosticEngine;

#[derive(Debug, Clone)]
pub struct MEADiagnosticRequest {
    pub electrolyzer_id: u8,
    pub eis_data: Vec<EISDataPoint>,
    pub step_response: StepResponseData,
    pub conductivity_history: Vec<f64>,
    pub temperature: f64,
}

#[derive(Debug, Clone)]
pub struct MEADiagnosticEngine {
    config: MeaDiagnosticsConfig,
    semaphore: Arc<Semaphore>,
}

impl MEADiagnosticEngine {
    pub fn new(config: MeaDiagnosticsConfig) -> (Self, mpsc::Receiver<MEADiagnosticResult>) {
        let (tx, rx) = mpsc::channel(100);
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_diagnoses));

        let engine = Self {
            config,
            semaphore,
        };

        (engine, rx)
    }

    pub async fn run_diagnosis(
        &self,
        request: MEADiagnosticRequest,
    ) -> Result<MEADiagnosticResult, Box<dyn std::error::Error + Send + Sync>> {
        let _permit = self.semaphore.acquire().await?;

        debug!(
            "Starting MEA diagnosis for electrolyzer {}, temperature={:.1}°C",
            request.electrolyzer_id, request.temperature
        );

        let mut equivalent_circuit = self.fit_equivalent_circuit(&request.eis_data)?;
        equivalent_circuit = self.apply_temperature_compensation(equivalent_circuit, request.temperature);
        let step_features = self.analyze_step_response(&request.step_response);
        let conductivity_trend =
            self.calculate_conductivity_trend(&request.conductivity_history);

        let (degradation_mode, confidence) = self.classify_degradation(
            &equivalent_circuit,
            &step_features,
            conductivity_trend,
        );

        let severity = self.determine_severity(
            &degradation_mode,
            &equivalent_circuit,
            conductivity_trend,
        );

        let recommendations = self.generate_recommendations(
            &degradation_mode,
            &severity,
            &equivalent_circuit,
        );

        let icons = self.generate_diagnostic_icons(
            request.electrolyzer_id,
            &degradation_mode,
            &severity,
            confidence,
        );

        let result = MEADiagnosticResult {
            electrolyzer_id: request.electrolyzer_id,
            timestamp: Utc::now(),
            equivalent_circuit,
            degradation_mode,
            severity,
            confidence,
            membrane_conductivity_trend: conductivity_trend,
            step_response_overshoot: step_features.overshoot,
            step_response_settling_time: step_features.settling_time,
            recommendations,
            icons,
        };

        info!(
            "MEA diagnosis complete for electrolyzer {}: mode={:?}, severity={:?}, confidence={:.2}",
            request.electrolyzer_id, result.degradation_mode, result.severity, result.confidence
        );

        Ok(result)
    }

    pub fn fit_equivalent_circuit(
        &self,
        eis_data: &[EISDataPoint],
    ) -> Result<EquivalentCircuitParams, Box<dyn std::error::Error + Send + Sync>> {
        if eis_data.len() < 3 {
            return Err("Insufficient EIS data points for fitting".into());
        }

        let mut params = EquivalentCircuitParams {
            ohmic_resistance: 0.08,
            charge_transfer_resistance: 0.15,
            double_layer_capacitance: 0.02,
            warburg_coefficient: 0.05,
            fit_error: 0.0,
        };

        let mut lambda = 1e-3;
        let mut prev_error = f64::INFINITY;

        for iteration in 0..self.config.eis_fit_max_iterations {
            let error = self.calculate_fit_error(eis_data, &params);
            params.fit_error = error;

            if (prev_error - error).abs() < self.config.eis_fit_tolerance {
                debug!("EIS fit converged after {} iterations, error={:.6}", iteration, error);
                break;
            }
            prev_error = error;

            let gradient = self.calculate_gradient(eis_data, &params);

            let mut update = [0.0; 4];
            for i in 0..4 {
                update[i] = -gradient[i] / (gradient[i].abs() + lambda);
            }

            let new_params = EquivalentCircuitParams {
                ohmic_resistance: (params.ohmic_resistance + update[0]).max(0.001),
                charge_transfer_resistance: (params.charge_transfer_resistance + update[1]).max(0.001),
                double_layer_capacitance: (params.double_layer_capacitance + update[2]).max(1e-6),
                warburg_coefficient: (params.warburg_coefficient + update[3]).max(0.0),
                fit_error: 0.0,
            };

            let new_error = self.calculate_fit_error(eis_data, &new_params);
            if new_error < error {
                params = new_params;
                lambda *= 0.5;
            } else {
                lambda *= 2.0;
            }
        }

        Ok(params)
    }

    pub fn apply_temperature_compensation(
        &self,
        mut params: EquivalentCircuitParams,
        temperature: f64,
    ) -> EquivalentCircuitParams {
        let t_ref = self.config.reference_temperature;
        let t = temperature.clamp(
            self.config.min_temperature_compensation,
            self.config.max_temperature_compensation,
        );
        let delta_t = t - t_ref;

        let alpha_r = self.config.temperature_coefficient_ohmic;
        let alpha_ct = self.config.temperature_coefficient_ct;
        let alpha_dlc = self.config.temperature_coefficient_dlc;
        let alpha_w = self.config.temperature_coefficient_warburg;

        let compensation_factor_r = 1.0 / (1.0 + alpha_r * delta_t);
        let compensation_factor_ct = 1.0 / (1.0 + alpha_ct * delta_t);
        let compensation_factor_dlc = 1.0 / (1.0 + alpha_dlc * delta_t);
        let compensation_factor_w = 1.0 / (1.0 + alpha_w * delta_t);

        debug!(
            "Temperature compensation: T={:.1}°C, ΔT={:.1}°C, factors: R_ohm={:.4}, R_ct={:.4}, C_dl={:.4}, W={:.4}",
            t, delta_t,
            compensation_factor_r,
            compensation_factor_ct,
            compensation_factor_dlc,
            compensation_factor_w
        );

        params.ohmic_resistance *= compensation_factor_r;
        params.charge_transfer_resistance *= compensation_factor_ct;
        params.double_layer_capacitance *= compensation_factor_dlc;
        params.warburg_coefficient *= compensation_factor_w;

        params
    }

    fn calculate_temperature_correction_coefficient(
        &self,
        temperature: f64,
    ) -> f64 {
        let t_ref = self.config.reference_temperature;
        let t = temperature.clamp(
            self.config.min_temperature_compensation,
            self.config.max_temperature_compensation,
        );
        let delta_t = t - t_ref;
        let activation_energy = 20000.0;
        let r = 8.314;
        let t_ref_k = t_ref + 273.15;
        let t_k = t + 273.15;

        (-activation_energy / r * (1.0 / t_k - 1.0 / t_ref_k)).exp()
    }

    fn calculate_fit_error(&self, eis_data: &[EISDataPoint], params: &EquivalentCircuitParams) -> f64 {
        let mut total_error = 0.0;

        for point in eis_data {
            let omega = 2.0 * std::f64::consts::PI * point.frequency;
            let z = self.calculate_impedance(params, omega);

            let error_real = z.0 - point.real_impedance;
            let error_imag = z.1 - point.imag_impedance;

            total_error += error_real * error_real + error_imag * error_imag;
        }

        (total_error / eis_data.len() as f64).sqrt()
    }

    fn calculate_impedance(&self, params: &EquivalentCircuitParams, omega: f64) -> (f64, f64) {
        let r_ct = params.charge_transfer_resistance;
        let c_dl = params.double_layer_capacitance;
        let w = params.warburg_coefficient;

        let denominator = 1.0 + (omega * c_dl * r_ct).powi(2);
        let z_parallel_real = r_ct / denominator;
        let z_parallel_imag = -omega * c_dl * r_ct * r_ct / denominator;

        let warburg_real = w / (2.0 * omega).sqrt();
        let warburg_imag = -w / (2.0 * omega).sqrt();

        (
            params.ohmic_resistance + z_parallel_real + warburg_real,
            z_parallel_imag + warburg_imag,
        )
    }

    fn calculate_gradient(
        &self,
        eis_data: &[EISDataPoint],
        params: &EquivalentCircuitParams,
    ) -> [f64; 4] {
        let epsilon = 1e-6;
        let mut gradient = [0.0; 4];

        let base_error = self.calculate_fit_error(eis_data, params);

        let mut params_plus = params.clone();
        params_plus.ohmic_resistance += epsilon;
        gradient[0] = (self.calculate_fit_error(eis_data, &params_plus) - base_error) / epsilon;

        let mut params_plus = params.clone();
        params_plus.charge_transfer_resistance += epsilon;
        gradient[1] = (self.calculate_fit_error(eis_data, &params_plus) - base_error) / epsilon;

        let mut params_plus = params.clone();
        params_plus.double_layer_capacitance += epsilon;
        gradient[2] = (self.calculate_fit_error(eis_data, &params_plus) - base_error) / epsilon;

        let mut params_plus = params.clone();
        params_plus.warburg_coefficient += epsilon;
        gradient[3] = (self.calculate_fit_error(eis_data, &params_plus) - base_error) / epsilon;

        gradient
    }

    fn analyze_step_response(&self, step_response: &StepResponseData) -> StepResponseFeatures {
        if step_response.time_points.len() < 2
            || step_response.voltage_points.len() != step_response.time_points.len()
        {
            return StepResponseFeatures {
                overshoot: 0.0,
                settling_time: 0.0,
                rise_time: 0.0,
                steady_state_value: 0.0,
            };
        }

        let initial_value = step_response.voltage_points[0];
        let final_value = *step_response.voltage_points.last().unwrap_or(&0.0);
        let step_change = final_value - initial_value;

        let steady_state_value = final_value;

        let mut peak_value = initial_value;
        for &v in &step_response.voltage_points {
            if step_change > 0.0 {
                if v > peak_value {
                    peak_value = v;
                }
            } else {
                if v < peak_value {
                    peak_value = v;
                }
            }
        }

        let overshoot = if step_change.abs() > 1e-6 {
            ((peak_value - final_value) / step_change.abs()) * 100.0
        } else {
            0.0
        };

        let settling_threshold = 0.05 * step_change.abs();
        let mut settling_time = 0.0;
        let mut settled = false;
        for i in (0..step_response.time_points.len()).rev() {
            let deviation = (step_response.voltage_points[i] - final_value).abs();
            if deviation <= settling_threshold && !settled {
                settling_time = step_response.time_points[i];
                settled = true;
            } else if deviation > settling_threshold && settled {
                break;
            }
        }

        let rise_start = if step_change > 0.0 {
            initial_value + 0.1 * step_change
        } else {
            initial_value - 0.1 * step_change.abs()
        };
        let rise_end = if step_change > 0.0 {
            initial_value + 0.9 * step_change
        } else {
            initial_value - 0.9 * step_change.abs()
        };

        let mut t_start = 0.0;
        let mut t_end = 0.0;
        let mut found_start = false;
        let mut found_end = false;

        for i in 0..step_response.time_points.len() {
            let v = step_response.voltage_points[i];
            if !found_start {
                if (step_change > 0.0 && v >= rise_start)
                    || (step_change < 0.0 && v <= rise_start)
                {
                    t_start = step_response.time_points[i];
                    found_start = true;
                }
            }
            if !found_end {
                if (step_change > 0.0 && v >= rise_end)
                    || (step_change < 0.0 && v <= rise_end)
                {
                    t_end = step_response.time_points[i];
                    found_end = true;
                }
            }
            if found_start && found_end {
                break;
            }
        }

        let rise_time = if found_start && found_end {
            t_end - t_start
        } else {
            0.0
        };

        StepResponseFeatures {
            overshoot,
            settling_time,
            rise_time,
            steady_state_value,
        }
    }

    fn calculate_conductivity_trend(&self, history: &[f64]) -> f64 {
        if history.len() < 2 {
            return 0.0;
        }

        let n = history.len() as f64;
        let sum_x: f64 = (0..history.len()).map(|i| i as f64).sum();
        let sum_y: f64 = history.iter().sum();
        let sum_xy: f64 = history
            .iter()
            .enumerate()
            .map(|(i, &y)| i as f64 * y)
            .sum();
        let sum_x2: f64 = (0..history.len()).map(|i| (i as f64).powi(2)).sum();

        let denominator = n * sum_x2 - sum_x * sum_x;
        if denominator.abs() < 1e-10 {
            return 0.0;
        }

        let slope = (n * sum_xy - sum_x * sum_y) / denominator;

        let avg_y = sum_y / n;
        if avg_y.abs() > 1e-10 {
            slope / avg_y * 100.0
        } else {
            0.0
        }
    }

    pub fn classify_degradation(
        &self,
        circuit: &EquivalentCircuitParams,
        step_features: &StepResponseFeatures,
        conductivity_trend: f64,
    ) -> (DegradationMode, f64) {
        let base_r_ohm = 0.08;
        let base_r_ct = 0.15;
        let base_w = 0.05;

        let r_ohm_ratio = circuit.ohmic_resistance / base_r_ohm;
        let r_ct_ratio = circuit.charge_transfer_resistance / base_r_ct;
        let w_ratio = circuit.warburg_coefficient / base_w;

        let mut scores = std::collections::HashMap::new();

        let dryout_score = (w_ratio - 1.0).max(0.0) * 0.4
            + (-conductivity_trend).max(0.0) * 0.4
            + step_features.overshoot.max(0.0) * 0.2;
        scores.insert(DegradationMode::MembraneDryout, dryout_score);

        let poisoning_score = (r_ct_ratio - 1.0).max(0.0) * 0.5
            + step_features.settling_time.max(0.0) * 0.3
            + (step_features.overshoot / 100.0).max(0.0) * 0.2;
        scores.insert(DegradationMode::CatalystPoisoning, poisoning_score);

        let contact_score = (r_ohm_ratio - 1.0).max(0.0) * 0.6
            + (-conductivity_trend).max(0.0) * 0.2
            + step_features.rise_time.max(0.0) * 0.2;
        scores.insert(DegradationMode::ContactResistanceIncrease, contact_score);

        let normal_score = 1.0
            - (r_ohm_ratio - 1.0).abs().min(1.0) * 0.3
            - (r_ct_ratio - 1.0).abs().min(1.0) * 0.3
            - (w_ratio - 1.0).abs().min(1.0) * 0.2
            - conductivity_trend.abs().min(1.0) * 0.2;
        scores.insert(DegradationMode::Normal, normal_score.max(0.0));

        let mut best_mode = DegradationMode::Normal;
        let mut best_score = 0.0;
        let mut total_score = 0.0;

        for (mode, score) in &scores {
            total_score += score;
            if score > best_score {
                best_score = *score;
                best_mode = mode.clone();
            }
        }

        let confidence = if total_score > 0.0 {
            best_score / total_score
        } else {
            0.5
        };

        if confidence < self.config.min_confidence {
            (DegradationMode::Normal, confidence)
        } else {
            (best_mode, confidence)
        }
    }

    fn determine_severity(
        &self,
        mode: &DegradationMode,
        circuit: &EquivalentCircuitParams,
        conductivity_trend: f64,
    ) -> DegradationSeverity {
        match mode {
            DegradationMode::Normal => DegradationSeverity::Low,
            DegradationMode::MembraneDryout => {
                let w_norm = (circuit.warburg_coefficient / 0.05 - 1.0).max(0.0);
                let cond_norm = (-conductivity_trend / 100.0).max(0.0);
                let severity = (w_norm + cond_norm) / 2.0;

                if severity < 0.1 {
                    DegradationSeverity::Low
                } else if severity < 0.3 {
                    DegradationSeverity::Medium
                } else if severity < 0.5 {
                    DegradationSeverity::High
                } else {
                    DegradationSeverity::Critical
                }
            }
            DegradationMode::CatalystPoisoning => {
                let rct_norm = (circuit.charge_transfer_resistance / 0.15 - 1.0).max(0.0);
                let severity = rct_norm;

                if severity < 0.15 {
                    DegradationSeverity::Low
                } else if severity < 0.4 {
                    DegradationSeverity::Medium
                } else if severity < 0.7 {
                    DegradationSeverity::High
                } else {
                    DegradationSeverity::Critical
                }
            }
            DegradationMode::ContactResistanceIncrease => {
                let roh_norm = (circuit.ohmic_resistance / 0.08 - 1.0).max(0.0);
                let severity = roh_norm;

                if severity < 0.1 {
                    DegradationSeverity::Low
                } else if severity < 0.25 {
                    DegradationSeverity::Medium
                } else if severity < 0.5 {
                    DegradationSeverity::High
                } else {
                    DegradationSeverity::Critical
                }
            }
        }
    }

    pub fn generate_recommendations(
        &self,
        mode: &DegradationMode,
        severity: &DegradationSeverity,
        circuit: &EquivalentCircuitParams,
    ) -> Vec<String> {
        let mut recommendations = Vec::new();

        match mode {
            DegradationMode::Normal => {
                recommendations.push("膜电极运行状态正常，继续定期监测".to_string());
            }
            DegradationMode::MembraneDryout => {
                recommendations.push("检测到膜干涸迹象，建议检查增湿系统".to_string());
                match severity {
                    DegradationSeverity::Low => {
                        recommendations.push("适当提高阳极侧湿度设定值".to_string());
                        recommendations.push("检查进水水质和流量".to_string());
                    }
                    DegradationSeverity::Medium => {
                        recommendations.push("降低运行电流密度，减轻膜干化程度".to_string());
                        recommendations.push("检查增湿器性能，必要时进行维护".to_string());
                    }
                    DegradationSeverity::High | DegradationSeverity::Critical => {
                        recommendations.push("紧急建议：降低负荷或停机检查，避免膜永久性损坏".to_string());
                        recommendations.push("检查膜电极组件是否需要更换".to_string());
                        recommendations.push(format!("当前Warburg阻抗: {:.4} Ω·cm²，超出基准{:.1}%",
                            circuit.warburg_coefficient,
                            (circuit.warburg_coefficient / 0.05 - 1.0) * 100.0));
                    }
                }
            }
            DegradationMode::CatalystPoisoning => {
                recommendations.push("检测到催化剂中毒迹象，建议检查气体纯度".to_string());
                match severity {
                    DegradationSeverity::Low => {
                        recommendations.push("检查原料气净化装置运行状态".to_string());
                        recommendations.push("适当提高运行温度，促进表面解吸".to_string());
                    }
                    DegradationSeverity::Medium => {
                        recommendations.push("进行原位活化：周期性低电流运行".to_string());
                        recommendations.push("检查是否存在CO或S等杂质来源".to_string());
                    }
                    DegradationSeverity::High | DegradationSeverity::Critical => {
                        recommendations.push("紧急建议：停机进行催化剂活化或更换".to_string());
                        recommendations.push(format!("当前电荷转移阻抗: {:.4} Ω·cm²，超出基准{:.1}%",
                            circuit.charge_transfer_resistance,
                            (circuit.charge_transfer_resistance / 0.15 - 1.0) * 100.0));
                    }
                }
            }
            DegradationMode::ContactResistanceIncrease => {
                recommendations.push("检测到接触电阻增大，建议检查电连接".to_string());
                match severity {
                    DegradationSeverity::Low => {
                        recommendations.push("检查端板螺栓扭矩是否均匀".to_string());
                        recommendations.push("监测接触电阻变化趋势".to_string());
                    }
                    DegradationSeverity::Medium => {
                        recommendations.push("停机检查密封垫片和集电器状态".to_string());
                        recommendations.push("按规范重新紧固端板螺栓".to_string());
                    }
                    DegradationSeverity::High | DegradationSeverity::Critical => {
                        recommendations.push("紧急建议：停机检查，避免过热导致密封失效".to_string());
                        recommendations.push("检查集电器表面是否氧化或腐蚀".to_string());
                        recommendations.push(format!("当前欧姆阻抗: {:.4} Ω·cm²，超出基准{:.1}%",
                            circuit.ohmic_resistance,
                            (circuit.ohmic_resistance / 0.08 - 1.0) * 100.0));
                    }
                }
            }
        }

        recommendations
    }

    fn generate_diagnostic_icons(
        &self,
        electrolyzer_id: u8,
        mode: &DegradationMode,
        severity: &DegradationSeverity,
        confidence: f64,
    ) -> Vec<DiagnosticIcon> {
        if matches!(mode, DegradationMode::Normal) {
            return Vec::new();
        }

        let positions = match mode {
            DegradationMode::MembraneDryout => vec![
                (0.3, 0.5),
                (0.5, 0.5),
                (0.7, 0.5),
            ],
            DegradationMode::CatalystPoisoning => vec![
                (0.25, 0.3),
                (0.5, 0.3),
                (0.75, 0.3),
                (0.25, 0.7),
                (0.5, 0.7),
                (0.75, 0.7),
            ],
            DegradationMode::ContactResistanceIncrease => vec![
                (0.1, 0.5),
                (0.9, 0.5),
            ],
            DegradationMode::Normal => vec![],
        };

        positions
            .into_iter()
            .map(|(x, y)| DiagnosticIcon {
                x,
                y,
                degradation_mode: mode.clone(),
                severity: severity.clone(),
                confidence,
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
struct StepResponseFeatures {
    overshoot: f64,
    settling_time: f64,
    rise_time: f64,
    steady_state_value: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn create_test_config() -> MeaDiagnosticsConfig {
        MeaDiagnosticsConfig {
            eis_fit_max_iterations: 100,
            eis_fit_tolerance: 1e-6,
            membrane_dryout_threshold: 0.15,
            catalyst_poisoning_threshold: 0.2,
            contact_resistance_threshold: 0.1,
            conductivity_trend_window: 100,
            min_confidence: 0.7,
            diagnosis_interval_secs: 300,
            max_concurrent_diagnoses: 3,
        }
    }

    fn generate_test_eis_data(params: &EquivalentCircuitParams, noise: f64) -> Vec<EISDataPoint> {
        let frequencies = vec![
            10000.0, 5000.0, 2000.0, 1000.0, 500.0, 200.0, 100.0, 50.0, 20.0, 10.0, 5.0, 2.0, 1.0, 0.5, 0.1,
        ];

        let engine = MEADiagnosticEngine::new(create_test_config()).0;

        frequencies
            .into_iter()
            .map(|f| {
                let omega = 2.0 * std::f64::consts::PI * f;
                let (real, imag) = engine.calculate_impedance(params, omega);

                let real_noise = if noise > 0.0 {
                    real + (rand::random::<f64>() - 0.5) * 2.0 * noise
                } else {
                    real
                };
                let imag_noise = if noise > 0.0 {
                    imag + (rand::random::<f64>() - 0.5) * 2.0 * noise
                } else {
                    imag
                };

                EISDataPoint {
                    frequency: f,
                    real_impedance: real_noise,
                    imag_impedance: imag_noise,
                }
            })
            .collect()
    }

    #[test]
    fn test_equivalent_circuit_fit_normal_case() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let expected_params = EquivalentCircuitParams {
            ohmic_resistance: 0.08,
            charge_transfer_resistance: 0.15,
            double_layer_capacitance: 0.02,
            warburg_coefficient: 0.05,
            fit_error: 0.0,
        };

        let eis_data = generate_test_eis_data(&expected_params, 0.0);
        let fitted = engine.fit_equivalent_circuit(&eis_data).unwrap();

        assert_relative_eq!(fitted.ohmic_resistance, expected_params.ohmic_resistance, epsilon = 0.01);
        assert_relative_eq!(fitted.charge_transfer_resistance, expected_params.charge_transfer_resistance, epsilon = 0.01);
        assert_relative_eq!(fitted.double_layer_capacitance, expected_params.double_layer_capacitance, epsilon = 0.01);
        assert_relative_eq!(fitted.warburg_coefficient, expected_params.warburg_coefficient, epsilon = 0.01);
        assert!(fitted.fit_error < 0.001);
    }

    #[test]
    fn test_equivalent_circuit_fit_with_noise() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let expected_params = EquivalentCircuitParams {
            ohmic_resistance: 0.09,
            charge_transfer_resistance: 0.18,
            double_layer_capacitance: 0.025,
            warburg_coefficient: 0.06,
            fit_error: 0.0,
        };

        let eis_data = generate_test_eis_data(&expected_params, 0.001);
        let fitted = engine.fit_equivalent_circuit(&eis_data).unwrap();

        assert_relative_eq!(fitted.ohmic_resistance, expected_params.ohmic_resistance, epsilon = 0.02);
        assert_relative_eq!(fitted.charge_transfer_resistance, expected_params.charge_transfer_resistance, epsilon = 0.03);
        assert!(fitted.fit_error > 0.0);
        assert!(fitted.fit_error < 0.01);
    }

    #[test]
    fn test_equivalent_circuit_fit_insufficient_data() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let eis_data = vec![EISDataPoint {
            frequency: 1000.0,
            real_impedance: 0.1,
            imag_impedance: -0.05,
        }];

        let result = engine.fit_equivalent_circuit(&eis_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_classify_degradation_normal() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let circuit = EquivalentCircuitParams {
            ohmic_resistance: 0.08,
            charge_transfer_resistance: 0.15,
            double_layer_capacitance: 0.02,
            warburg_coefficient: 0.05,
            fit_error: 0.001,
        };

        let step_features = StepResponseFeatures {
            overshoot: 5.0,
            settling_time: 0.5,
            rise_time: 0.2,
            steady_state_value: 1.8,
        };

        let (mode, confidence) = engine.classify_degradation(&circuit, &step_features, 0.5);

        assert_eq!(mode, DegradationMode::Normal);
        assert!(confidence >= 0.5);
    }

    #[test]
    fn test_classify_degradation_membrane_dryout() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let circuit = EquivalentCircuitParams {
            ohmic_resistance: 0.085,
            charge_transfer_resistance: 0.155,
            double_layer_capacitance: 0.02,
            warburg_coefficient: 0.12,
            fit_error: 0.001,
        };

        let step_features = StepResponseFeatures {
            overshoot: 20.0,
            settling_time: 2.0,
            rise_time: 0.3,
            steady_state_value: 1.95,
        };

        let (mode, confidence) = engine.classify_degradation(&circuit, &step_features, -15.0);

        assert_eq!(mode, DegradationMode::MembraneDryout);
        assert!(confidence > 0.6);
    }

    #[test]
    fn test_classify_degradation_catalyst_poisoning() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let circuit = EquivalentCircuitParams {
            ohmic_resistance: 0.082,
            charge_transfer_resistance: 0.28,
            double_layer_capacitance: 0.018,
            warburg_coefficient: 0.055,
            fit_error: 0.001,
        };

        let step_features = StepResponseFeatures {
            overshoot: 8.0,
            settling_time: 3.5,
            rise_time: 0.8,
            steady_state_value: 2.0,
        };

        let (mode, confidence) = engine.classify_degradation(&circuit, &step_features, -2.0);

        assert_eq!(mode, DegradationMode::CatalystPoisoning);
        assert!(confidence > 0.6);
    }

    #[test]
    fn test_classify_degradation_contact_resistance() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let circuit = EquivalentCircuitParams {
            ohmic_resistance: 0.13,
            charge_transfer_resistance: 0.155,
            double_layer_capacitance: 0.02,
            warburg_coefficient: 0.052,
            fit_error: 0.001,
        };

        let step_features = StepResponseFeatures {
            overshoot: 3.0,
            settling_time: 0.8,
            rise_time: 1.2,
            steady_state_value: 1.85,
        };

        let (mode, confidence) = engine.classify_degradation(&circuit, &step_features, -8.0);

        assert_eq!(mode, DegradationMode::ContactResistanceIncrease);
        assert!(confidence > 0.6);
    }

    #[test]
    fn test_severity_classification_critical() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let circuit = EquivalentCircuitParams {
            ohmic_resistance: 0.08,
            charge_transfer_resistance: 0.15,
            double_layer_capacitance: 0.02,
            warburg_coefficient: 0.15,
            fit_error: 0.001,
        };

        let severity = engine.determine_severity(
            &DegradationMode::MembraneDryout,
            &circuit,
            -25.0,
        );

        assert_eq!(severity, DegradationSeverity::Critical);
    }

    #[test]
    fn test_severity_classification_low() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let circuit = EquivalentCircuitParams {
            ohmic_resistance: 0.082,
            charge_transfer_resistance: 0.15,
            double_layer_capacitance: 0.02,
            warburg_coefficient: 0.05,
            fit_error: 0.001,
        };

        let severity = engine.determine_severity(
            &DegradationMode::ContactResistanceIncrease,
            &circuit,
            -1.0,
        );

        assert_eq!(severity, DegradationSeverity::Low);
    }

    #[test]
    fn test_generate_recommendations_normal() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let circuit = EquivalentCircuitParams {
            ohmic_resistance: 0.08,
            charge_transfer_resistance: 0.15,
            double_layer_capacitance: 0.02,
            warburg_coefficient: 0.05,
            fit_error: 0.001,
        };

        let recommendations = engine.generate_recommendations(
            &DegradationMode::Normal,
            &DegradationSeverity::Low,
            &circuit,
        );

        assert!(!recommendations.is_empty());
        assert!(recommendations[0].contains("正常"));
    }

    #[test]
    fn test_generate_recommendations_critical_dryout() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let circuit = EquivalentCircuitParams {
            ohmic_resistance: 0.08,
            charge_transfer_resistance: 0.15,
            double_layer_capacitance: 0.02,
            warburg_coefficient: 0.15,
            fit_error: 0.001,
        };

        let recommendations = engine.generate_recommendations(
            &DegradationMode::MembraneDryout,
            &DegradationSeverity::Critical,
            &circuit,
        );

        assert!(recommendations.len() >= 3);
        assert!(recommendations.iter().any(|r| r.contains("紧急")));
        assert!(recommendations.iter().any(|r| r.contains("Warburg")));
    }

    #[test]
    fn test_conductivity_trend_decreasing() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let history: Vec<f64> = (0..50).map(|i| 100.0 - i as f64 * 0.5).collect();
        let trend = engine.calculate_conductivity_trend(&history);

        assert!(trend < 0.0);
        assert!((trend + 0.5).abs() < 0.1);
    }

    #[test]
    fn test_conductivity_trend_increasing() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let history: Vec<f64> = (0..50).map(|i| 50.0 + i as f64 * 0.3).collect();
        let trend = engine.calculate_conductivity_trend(&history);

        assert!(trend > 0.0);
    }

    #[test]
    fn test_conductivity_trend_insufficient_data() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let history = vec![95.0];
        let trend = engine.calculate_conductivity_trend(&history);

        assert_eq!(trend, 0.0);
    }

    #[test]
    fn test_step_response_analysis() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let mut time_points = Vec::new();
        let mut voltage_points = Vec::new();
        let dt = 0.01;
        for i in 0..200 {
            let t = i as f64 * dt;
            let v = if t < 0.5 {
                1.6
            } else {
                1.6 + 0.25 * (1.0 - (- (t - 0.5) / 0.05).exp())
            };
            time_points.push(t);
            voltage_points.push(v);
        }

        let step_response = StepResponseData {
            time_points,
            voltage_points,
            current_step: 0.5,
        };

        let features = engine.analyze_step_response(&step_response);

        assert!(features.steady_state_value > 1.8);
        assert!(features.overshoot.abs() < 5.0);
        assert!(features.settling_time > 0.0);
    }

    #[test]
    fn test_step_response_analysis_with_overshoot() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let mut time_points = Vec::new();
        let mut voltage_points = Vec::new();
        let dt = 0.01;
        for i in 0..300 {
            let t = i as f64 * dt;
            let v = if t < 0.5 {
                1.6
            } else {
                let tau = 0.1;
                let zeta = 0.5;
                let omega_n = 10.0;
                let step = 0.25;
                let rel_t = t - 0.5;
                let overshoot = (-zeta * omega_n * rel_t).exp()
                    * (omega_n * rel_t * (1.0 - zeta * zeta).sqrt()).sin();
                1.6 + step * (1.0 + overshoot * 0.3)
            };
            time_points.push(t);
            voltage_points.push(v);
        }

        let step_response = StepResponseData {
            time_points,
            voltage_points,
            current_step: 0.5,
        };

        let features = engine.analyze_step_response(&step_response);

        assert!(features.overshoot > 0.0);
        assert!(features.settling_time > 0.5);
    }

    #[test]
    fn test_impedance_calculation() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let params = EquivalentCircuitParams {
            ohmic_resistance: 0.08,
            charge_transfer_resistance: 0.15,
            double_layer_capacitance: 0.02,
            warburg_coefficient: 0.05,
            fit_error: 0.0,
        };

        let omega_high = 2.0 * std::f64::consts::PI * 10000.0;
        let (zr_hi, zi_hi) = engine.calculate_impedance(&params, omega_high);

        assert_relative_eq!(zr_hi, 0.08, epsilon = 0.001);
        assert!(zi_hi.abs() < 0.01);

        let omega_low = 2.0 * std::f64::consts::PI * 0.1;
        let (zr_lo, zi_lo) = engine.calculate_impedance(&params, omega_low);

        assert!(zr_lo > 0.2);
        assert!(zi_lo < -0.05);
    }

    #[tokio::test]
    async fn test_full_diagnosis_pipeline() {
        let config = create_test_config();
        let (engine, _rx) = MEADiagnosticEngine::new(config);

        let expected_params = EquivalentCircuitParams {
            ohmic_resistance: 0.095,
            charge_transfer_resistance: 0.15,
            double_layer_capacitance: 0.02,
            warburg_coefficient: 0.05,
            fit_error: 0.0,
        };

        let eis_data = generate_test_eis_data(&expected_params, 0.0005);

        let mut time_points = Vec::new();
        let mut voltage_points = Vec::new();
        for i in 0..150 {
            let t = i as f64 * 0.01;
            let v = if t < 0.5 {
                1.6
            } else {
                1.6 + 0.2 * (1.0 - (- (t - 0.5) / 0.15).exp())
            };
            time_points.push(t);
            voltage_points.push(v);
        }

        let step_response = StepResponseData {
            time_points,
            voltage_points,
            current_step: 0.5,
        };

        let conductivity_history: Vec<f64> = (0..30).map(|i| 90.0 - i as f64 * 0.3).collect();

        let request = MEADiagnosticRequest {
            electrolyzer_id: 1,
            eis_data,
            step_response,
            conductivity_history,
        };

        let result = engine.run_diagnosis(request).await.unwrap();

        assert_eq!(result.electrolyzer_id, 1);
        assert!(result.confidence > 0.0);
        assert!(!result.recommendations.is_empty());
        assert!(result.equivalent_circuit.fit_error < 0.01);
        assert_relative_eq!(result.equivalent_circuit.ohmic_resistance, 0.095, epsilon = 0.01);
    }

    #[test]
    fn test_diagnostic_icons_generation() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let icons = engine.generate_diagnostic_icons(
            1,
            &DegradationMode::MembraneDryout,
            &DegradationSeverity::High,
            0.85,
        );

        assert_eq!(icons.len(), 3);
        for icon in &icons {
            assert_eq!(icon.degradation_mode, DegradationMode::MembraneDryout);
            assert_eq!(icon.severity, DegradationSeverity::High);
            assert_eq!(icon.confidence, 0.85);
            assert!(icon.x > 0.0 && icon.x < 1.0);
            assert!(icon.y > 0.0 && icon.y < 1.0);
        }

        let icons_normal = engine.generate_diagnostic_icons(
            1,
            &DegradationMode::Normal,
            &DegradationSeverity::Low,
            0.9,
        );
        assert!(icons_normal.is_empty());
    }

    #[test]
    fn test_gradient_calculation() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let params = EquivalentCircuitParams {
            ohmic_resistance: 0.08,
            charge_transfer_resistance: 0.15,
            double_layer_capacitance: 0.02,
            warburg_coefficient: 0.05,
            fit_error: 0.0,
        };

        let eis_data = generate_test_eis_data(&params, 0.0);
        let gradient = engine.calculate_gradient(&eis_data, &params);

        assert_eq!(gradient.len(), 4);
        for &g in &gradient {
            assert!(!g.is_nan());
            assert!(!g.is_infinite());
        }
    }

    #[test]
    fn test_fit_error_calculation() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let params = EquivalentCircuitParams {
            ohmic_resistance: 0.08,
            charge_transfer_resistance: 0.15,
            double_layer_capacitance: 0.02,
            warburg_coefficient: 0.05,
            fit_error: 0.0,
        };

        let eis_data = generate_test_eis_data(&params, 0.0);
        let error = engine.calculate_fit_error(&eis_data, &params);

        assert_relative_eq!(error, 0.0, epsilon = 1e-6);
    }

    #[test]
    fn test_classification_confidence_threshold() {
        let mut config = create_test_config();
        config.min_confidence = 0.9;
        let (engine, _) = MEADiagnosticEngine::new(config);

        let circuit = EquivalentCircuitParams {
            ohmic_resistance: 0.082,
            charge_transfer_resistance: 0.152,
            double_layer_capacitance: 0.02,
            warburg_coefficient: 0.052,
            fit_error: 0.001,
        };

        let step_features = StepResponseFeatures {
            overshoot: 6.0,
            settling_time: 0.8,
            rise_time: 0.25,
            steady_state_value: 1.82,
        };

        let (mode, confidence) = engine.classify_degradation(&circuit, &step_features, -1.0);

        assert_eq!(mode, DegradationMode::Normal);
        assert!(confidence < 0.9);
    }

    #[test]
    fn test_temperature_compensation_low_temperature() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let params = EquivalentCircuitParams {
            ohmic_resistance: 0.088,
            charge_transfer_resistance: 0.168,
            double_layer_capacitance: 0.019,
            warburg_coefficient: 0.055,
            fit_error: 0.001,
        };

        let compensated = engine.apply_temperature_compensation(params.clone(), 20.0);

        assert!(compensated.ohmic_resistance < params.ohmic_resistance);
        assert!(compensated.charge_transfer_resistance < params.charge_transfer_resistance);
        assert!(compensated.double_layer_capacitance > params.double_layer_capacitance);
        assert!(compensated.warburg_coefficient < params.warburg_coefficient);

        assert_relative_eq!(compensated.ohmic_resistance, 0.08, epsilon = 0.01);
        assert_relative_eq!(compensated.charge_transfer_resistance, 0.15, epsilon = 0.01);
    }

    #[test]
    fn test_temperature_compensation_high_temperature() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let params = EquivalentCircuitParams {
            ohmic_resistance: 0.076,
            charge_transfer_resistance: 0.14,
            double_layer_capacitance: 0.021,
            warburg_coefficient: 0.047,
            fit_error: 0.001,
        };

        let compensated = engine.apply_temperature_compensation(params.clone(), 80.0);

        assert!(compensated.ohmic_resistance > params.ohmic_resistance);
        assert!(compensated.charge_transfer_resistance > params.charge_transfer_resistance);
        assert!(compensated.double_layer_capacitance < params.double_layer_capacitance);
        assert!(compensated.warburg_coefficient > params.warburg_coefficient);

        assert_relative_eq!(compensated.ohmic_resistance, 0.08, epsilon = 0.01);
        assert_relative_eq!(compensated.charge_transfer_resistance, 0.15, epsilon = 0.01);
    }

    #[test]
    fn test_temperature_compensation_reference_temperature() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config.clone());

        let params = EquivalentCircuitParams {
            ohmic_resistance: 0.08,
            charge_transfer_resistance: 0.15,
            double_layer_capacitance: 0.02,
            warburg_coefficient: 0.05,
            fit_error: 0.001,
        };

        let compensated = engine.apply_temperature_compensation(params.clone(), config.reference_temperature);

        assert_relative_eq!(compensated.ohmic_resistance, params.ohmic_resistance, epsilon = 1e-6);
        assert_relative_eq!(compensated.charge_transfer_resistance, params.charge_transfer_resistance, epsilon = 1e-6);
        assert_relative_eq!(compensated.double_layer_capacitance, params.double_layer_capacitance, epsilon = 1e-6);
        assert_relative_eq!(compensated.warburg_coefficient, params.warburg_coefficient, epsilon = 1e-6);
    }

    #[test]
    fn test_temperature_compensation_clamping() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config.clone());

        let params = EquivalentCircuitParams {
            ohmic_resistance: 0.1,
            charge_transfer_resistance: 0.2,
            double_layer_capacitance: 0.018,
            warburg_coefficient: 0.06,
            fit_error: 0.001,
        };

        let compensated_low = engine.apply_temperature_compensation(params.clone(), 0.0);
        let compensated_high = engine.apply_temperature_compensation(params.clone(), 100.0);

        let expected_low = engine.apply_temperature_compensation(params.clone(), config.min_temperature_compensation);
        let expected_high = engine.apply_temperature_compensation(params.clone(), config.max_temperature_compensation);

        assert_relative_eq!(compensated_low.ohmic_resistance, expected_low.ohmic_resistance, epsilon = 1e-6);
        assert_relative_eq!(compensated_high.ohmic_resistance, expected_high.ohmic_resistance, epsilon = 1e-6);
    }

    #[test]
    fn test_temperature_compensation_with_eis_fit() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config.clone());

        let base_params = EquivalentCircuitParams {
            ohmic_resistance: 0.08,
            charge_transfer_resistance: 0.15,
            double_layer_capacitance: 0.02,
            warburg_coefficient: 0.05,
            fit_error: 0.0,
        };

        let eis_data = generate_test_eis_data(&base_params, 0.0001);

        let mut time_points = Vec::new();
        let mut voltage_points = Vec::new();
        for i in 0..150 {
            let t = i as f64 * 0.01;
            let v = if t < 0.5 {
                1.6
            } else {
                1.6 + 0.2 * (1.0 - (- (t - 0.5) / 0.15).exp())
            };
            time_points.push(t);
            voltage_points.push(v);
        }

        let step_response = StepResponseData {
            time_points,
            voltage_points,
            current_step: 0.5,
        };

        let conductivity_history: Vec<f64> = (0..30).map(|i| 90.0 - i as f64 * 0.1).collect();

        let request = MEADiagnosticRequest {
            electrolyzer_id: 1,
            eis_data: eis_data.clone(),
            step_response: step_response.clone(),
            conductivity_history: conductivity_history.clone(),
            temperature: 20.0,
        };

        let result = engine.run_diagnosis(request).await.unwrap();

        assert!(result.equivalent_circuit.ohmic_resistance < 0.088);
        assert!(result.equivalent_circuit.charge_transfer_resistance < 0.168);
    }

    #[test]
    fn test_temperature_correction_coefficient() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let coeff_20 = engine.calculate_temperature_correction_coefficient(20.0);
        let coeff_60 = engine.calculate_temperature_correction_coefficient(60.0);
        let coeff_80 = engine.calculate_temperature_correction_coefficient(80.0);

        assert!(coeff_20 > 1.0);
        assert_relative_eq!(coeff_60, 1.0, epsilon = 0.01);
        assert!(coeff_80 < 1.0);
        assert!(coeff_20 > coeff_80);
    }

    #[test]
    fn test_low_temperature_diagnosis_accuracy() {
        let config = create_test_config();
        let (engine, _) = MEADiagnosticEngine::new(config);

        let dryout_params_60c = EquivalentCircuitParams {
            ohmic_resistance: 0.08,
            charge_transfer_resistance: 0.15,
            double_layer_capacitance: 0.02,
            warburg_coefficient: 0.15,
            fit_error: 0.0,
        };

        let eis_data = generate_test_eis_data(&dryout_params_60c, 0.0001);

        let mut time_points = Vec::new();
        let mut voltage_points = Vec::new();
        for i in 0..150 {
            let t = i as f64 * 0.01;
            let v = if t < 0.5 {
                1.6
            } else {
                1.6 + 0.2 * (1.0 - (- (t - 0.5) / 0.15).exp())
            };
            time_points.push(t);
            voltage_points.push(v);
        }

        let step_response = StepResponseData {
            time_points,
            voltage_points,
            current_step: 0.5,
        };

        let conductivity_history: Vec<f64> = (0..30).map(|i| 90.0 - i as f64 * 0.5).collect();

        let request_20c = MEADiagnosticRequest {
            electrolyzer_id: 1,
            eis_data: eis_data.clone(),
            step_response: step_response.clone(),
            conductivity_history: conductivity_history.clone(),
            temperature: 20.0,
        };

        let result_20c = engine.run_diagnosis(request_20c).await.unwrap();

        assert_eq!(result_20c.degradation_mode, DegradationMode::MembraneDryout);
        assert!(result_20c.confidence > 0.7);
    }
}
