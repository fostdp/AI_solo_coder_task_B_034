use chrono::Utc;
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::config::LeakDetectionConfig;
use crate::models::*;

#[derive(Debug, Clone)]
pub struct SensorPosition {
    pub sensor_id: u16,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone)]
pub struct LeakDetectionRequest {
    pub electrolyzer_id: u8,
    pub acoustic_data: Vec<AcousticEmissionData>,
    pub sensor_positions: Vec<SensorPosition>,
    pub flow_rate_reference: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct HydrogenLeakDetector {
    config: LeakDetectionConfig,
    semaphore: Arc<Semaphore>,
}

impl HydrogenLeakDetector {
    pub fn new(config: LeakDetectionConfig) -> (Self, mpsc::Receiver<HydrogenLeak>) {
        let (tx, rx) = mpsc::channel(100);
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_detections));

        let engine = Self {
            config,
            semaphore,
        };

        (engine, rx)
    }

    pub async fn detect_leaks(
        &self,
        request: LeakDetectionRequest,
    ) -> Result<Vec<HydrogenLeak>, Box<dyn std::error::Error + Send + Sync>> {
        let _permit = self.semaphore.acquire().await?;

        debug!(
            "Starting leak detection for electrolyzer {} with {} sensors",
            request.electrolyzer_id,
            request.acoustic_data.len()
        );

        let mut leaks = Vec::new();

        let mut sensor_features = Vec::new();
        for data in &request.acoustic_data {
            let features = self.calculate_spectral_features(data)?;
            sensor_features.push((data.sensor_id, features));
        }

        let active_sensors: Vec<(u16, SpectralFeatures)> = sensor_features
            .into_iter()
            .filter(|(_, f)| {
                f.rms > self.config.leak_threshold_rms
                    || f.peak_amplitude > self.config.leak_threshold_peak
            })
            .collect();

        if active_sensors.len() >= self.config.trilateration_sensor_count {
            let toa_data: Vec<(u16, f64, f64)> = active_sensors
                .iter()
                .map(|(id, f)| {
                    let toa = self.estimate_time_of_arrival(
                        &request
                            .acoustic_data
                            .iter()
                            .find(|d| d.sensor_id == *id)
                            .unwrap(),
                    );
                    (*id, toa, f.peak_amplitude)
                })
                .collect();

            let leak_location = self.trilaterate_leak_position(&toa_data, &request.sensor_positions)?;

            let max_amplitude = active_sensors
                .iter()
                .map(|(_, f)| f.peak_amplitude)
                .fold(f64::NEG_INFINITY, f64::max);

            let leak_rate = self.calculate_leak_rate(max_amplitude, leak_location.uncertainty);

            if leak_rate >= self.config.min_leak_rate {
                let diffusion_radius =
                    self.calculate_diffusion_radius(leak_rate, Utc::now().timestamp() as f64);

                let severity = self.determine_leak_severity(leak_rate, diffusion_radius);

                let primary_features = active_sensors
                    .iter()
                    .max_by(|a, b| a.1.peak_amplitude.partial_cmp(&b.1.peak_amplitude).unwrap())
                    .map(|(_, f)| f.clone())
                    .unwrap_or(SpectralFeatures {
                        rms: 0.0,
                        peak_frequency: 0.0,
                        spectral_centroid: 0.0,
                        kurtosis: 0.0,
                        peak_amplitude: 0.0,
                    });

                let leak = HydrogenLeak {
                    id: Uuid::new_v4(),
                    electrolyzer_id: request.electrolyzer_id,
                    timestamp: Utc::now(),
                    location: leak_location,
                    leak_rate,
                    diffusion_radius,
                    severity,
                    spectral_features: primary_features,
                    acknowledged: false,
                    resolved: false,
                };

                info!(
                    "Leak detected for electrolyzer {}: rate={:.4} L/min, radius={:.2} m, severity={:?}",
                    request.electrolyzer_id, leak.leak_rate, leak.diffusion_radius, leak.severity
                );

                leaks.push(leak);
            }
        } else if !active_sensors.is_empty() {
            warn!(
                "Insufficient active sensors for trilateration: {}/{} required",
                active_sensors.len(),
                self.config.trilateration_sensor_count
            );
        }

        if let Some(flow_rate) = request.flow_rate_reference {
            for leak in &mut leaks {
                let corrected_rate = self.calibrate_leak_rate(leak.leak_rate, flow_rate);
                debug!(
                    "Leak rate calibrated: {:.4} -> {:.4} L/min (reference: {:.4})",
                    leak.leak_rate, corrected_rate, flow_rate
                );
                leak.leak_rate = corrected_rate;
            }
        }

        Ok(leaks)
    }

    pub fn fft(&self, signal: &[f64]) -> Vec<(f64, f64)> {
        let n = signal.len();
        if n == 0 {
            return Vec::new();
        }

        let n_power2 = n.next_power_of_two();
        let mut padded = signal.to_vec();
        padded.resize(n_power2, 0.0);

        let result = self.cooley_tukey_fft(&padded, false);

        let sampling_rate = 1.0;
        result
            .iter()
            .take(n_power2 / 2)
            .enumerate()
            .map(|(i, &(re, im))| {
                let freq = i as f64 * sampling_rate / n_power2 as f64;
                let magnitude = (re * re + im * im).sqrt() / n_power2 as f64;
                (freq, magnitude)
            })
            .collect()
    }

    fn cooley_tukey_fft(&self, signal: &[f64], inverse: bool) -> Vec<(f64, f64)> {
        let n = signal.len();
        if n == 1 {
            return vec![(signal[0], 0.0)];
        }

        let mut even = Vec::with_capacity(n / 2);
        let mut odd = Vec::with_capacity(n / 2);
        for i in (0..n).step_by(2) {
            even.push(signal[i]);
            odd.push(signal[i + 1]);
        }

        let fft_even = self.cooley_tukey_fft(&even, inverse);
        let fft_odd = self.cooley_tukey_fft(&odd, inverse);

        let mut result = vec![(0.0, 0.0); n];
        let sign = if inverse { 1.0 } else { -1.0 };

        for k in 0..n / 2 {
            let angle = sign * 2.0 * std::f64::consts::PI * k as f64 / n as f64;
            let w_re = angle.cos();
            let w_im = angle.sin();

            let (odd_re, odd_im) = fft_odd[k];
            let t_re = w_re * odd_re - w_im * odd_im;
            let t_im = w_re * odd_im + w_im * odd_re;

            let (even_re, even_im) = fft_even[k];
            result[k] = (even_re + t_re, even_im + t_im);
            result[k + n / 2] = (even_re - t_re, even_im - t_im);
        }

        if inverse {
            for (re, im) in result.iter_mut() {
                *re /= n as f64;
                *im /= n as f64;
            }
        }

        result
    }

    pub fn calculate_spectral_features(
        &self,
        data: &AcousticEmissionData,
    ) -> Result<SpectralFeatures, Box<dyn std::error::Error + Send + Sync>> {
        if data.signal.is_empty() {
            return Err("Empty signal data".into());
        }

        let signal = &data.signal;
        let n = signal.len();

        let rms = (signal.iter().map(|&x| x * x).sum::<f64>() / n as f64).sqrt();

        let peak_amplitude = signal.iter().map(|&x| x.abs()).fold(f64::NEG_INFINITY, f64::max);

        let fft_result = self.fft(signal);

        let mut peak_freq = 0.0;
        let mut max_magnitude = 0.0;
        let mut weighted_sum = 0.0;
        let mut total_magnitude = 0.0;

        for (freq, mag) in &fft_result {
            if *freq >= self.config.ultrasound_frequency_min
                && *freq <= self.config.ultrasound_frequency_max
            {
                if *mag > max_magnitude {
                    max_magnitude = *mag;
                    peak_freq = *freq;
                }
                weighted_sum += freq * mag;
                total_magnitude += mag;
            }
        }

        let spectral_centroid = if total_magnitude > 0.0 {
            weighted_sum / total_magnitude
        } else {
            0.0
        };

        let mean = signal.iter().sum::<f64>() / n as f64;
        let mut variance = 0.0;
        let mut fourth_moment = 0.0;
        for &x in signal {
            let diff = x - mean;
            variance += diff * diff;
            fourth_moment += diff * diff * diff * diff;
        }
        variance /= n as f64;
        fourth_moment /= n as f64;

        let kurtosis = if variance > 1e-10 {
            fourth_moment / (variance * variance) - 3.0
        } else {
            0.0
        };

        Ok(SpectralFeatures {
            rms,
            peak_frequency: peak_freq * data.sampling_rate,
            spectral_centroid: spectral_centroid * data.sampling_rate,
            kurtosis,
            peak_amplitude,
        })
    }

    fn estimate_time_of_arrival(&self, data: &AcousticEmissionData) -> f64 {
        let threshold = self.config.leak_threshold_peak * 0.5;
        let mut toa = 0.0;

        for (i, &sample) in data.signal.iter().enumerate() {
            if sample.abs() > threshold {
                toa = i as f64 / data.sampling_rate;
                break;
            }
        }

        toa
    }

    pub fn trilaterate_leak_position(
        &self,
        toa_data: &[(u16, f64, f64)],
        sensor_positions: &[SensorPosition],
    ) -> Result<LeakLocation, Box<dyn std::error::Error + Send + Sync>> {
        if toa_data.len() < 3 {
            return Err("At least 3 sensors required for trilateration".into());
        }

        let reference_toa = toa_data
            .iter()
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|&(_, t, _)| t)
            .unwrap_or(0.0);

        let mut time_differences = Vec::new();
        for (sensor_id, toa, amplitude) in toa_data {
            let td = toa - reference_toa;
            if let Some(pos) = sensor_positions.iter().find(|p| p.sensor_id == *sensor_id) {
                time_differences.push((pos, td, amplitude));
            }
        }

        if time_differences.len() < 3 {
            return Err("Could not find positions for all sensors".into());
        }

        let mut x = 0.5;
        let mut y = 0.5;
        let mut z = 0.0;
        let sound_speed = self.config.sound_speed_hydrogen;

        let mut total_weight = 0.0;
        for (pos, td, amplitude) in &time_differences {
            let distance = td * sound_speed;
            let weight = amplitude / (1.0 + distance);
            x += pos.x * weight;
            y += pos.y * weight;
            z += pos.z * weight;
            total_weight += weight;
        }

        if total_weight > 0.0 {
            x /= total_weight + 1.0;
            y /= total_weight + 1.0;
            z /= total_weight + 1.0;
        }

        let mut error_sum = 0.0;
        let mut count = 0.0;
        for (pos, td, _) in &time_differences {
            let dx = x - pos.x;
            let dy = y - pos.y;
            let dz = z - pos.z;
            let distance = (dx * dx + dy * dy + dz * dz).sqrt();
            let expected_td = distance / sound_speed;
            error_sum += (td - expected_td).abs();
            count += 1.0;
        }

        let uncertainty = if count > 0.0 {
            (error_sum / count) * sound_speed
        } else {
            0.1
        };

        Ok(LeakLocation {
            x: x.clamp(0.0, 1.0),
            y: y.clamp(0.0, 1.0),
            z: z.clamp(-0.1, 0.5),
            uncertainty: uncertainty.max(0.01),
        })
    }

    pub fn calculate_leak_rate(
        &self,
        amplitude: f64,
        uncertainty: f64,
    ) -> f64 {
        let calibration_factor = 10.0;
        let base_rate = amplitude * calibration_factor;
        let uncertainty_factor = 1.0 - uncertainty.min(0.5);
        base_rate * uncertainty_factor
    }

    pub fn calibrate_leak_rate(&self, estimated_rate: f64, flow_reference: f64) -> f64 {
        if flow_reference <= 0.0 {
            return estimated_rate;
        }

        let max_expected_rate = flow_reference * 0.1;
        estimated_rate.min(max_expected_rate)
    }

    pub fn calculate_diffusion_radius(&self, leak_rate: f64, time_seconds: f64) -> f64 {
        if leak_rate <= 0.0 || time_seconds <= 0.0 {
            return 0.0;
        }

        let d = self.config.diffusion_coefficient;
        let volume = leak_rate * time_seconds / 60.0;
        let concentration_threshold = 0.04;

        let radius = ((3.0 * volume) / (4.0 * std::f64::consts::PI * concentration_threshold))
            .cbrt();

        let diffusion_radius = 2.0 * (d * time_seconds).sqrt();

        radius.max(diffusion_radius).min(5.0)
    }

    fn determine_leak_severity(&self, leak_rate: f64, diffusion_radius: f64) -> DegradationSeverity {
        let rate_score = if leak_rate < 0.01 {
            0.0
        } else if leak_rate < 0.1 {
            0.25
        } else if leak_rate < 0.5 {
            0.5
        } else if leak_rate < 1.0 {
            0.75
        } else {
            1.0
        };

        let radius_score = if diffusion_radius < 0.1 {
            0.0
        } else if diffusion_radius < 0.5 {
            0.25
        } else if diffusion_radius < 1.0 {
            0.5
        } else if diffusion_radius < 2.0 {
            0.75
        } else {
            1.0
        };

        let total_score = (rate_score + radius_score) / 2.0;

        if total_score < 0.15 {
            DegradationSeverity::Low
        } else if total_score < 0.4 {
            DegradationSeverity::Medium
        } else if total_score < 0.7 {
            DegradationSeverity::High
        } else {
            DegradationSeverity::Critical
        }
    }

    pub fn generate_leak_animations(leaks: &[HydrogenLeak]) -> Vec<LeakAnimation> {
        leaks
            .iter()
            .map(|leak| LeakAnimation {
                leak_id: leak.id,
                x: leak.location.x,
                y: leak.location.y,
                max_radius: leak.diffusion_radius.min(0.3),
                leak_rate: leak.leak_rate,
                severity: leak.severity.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn create_test_config() -> LeakDetectionConfig {
        LeakDetectionConfig {
            ultrasound_frequency_min: 30000.0,
            ultrasound_frequency_max: 80000.0,
            leak_threshold_rms: 0.01,
            leak_threshold_peak: 0.05,
            sound_speed_hydrogen: 1310.0,
            diffusion_coefficient: 0.61e-4,
            trilateration_sensor_count: 3,
            min_leak_rate: 0.001,
            detection_interval_secs: 10,
            max_concurrent_detections: 5,
        }
    }

    fn generate_test_signal(
        duration: f64,
        sampling_rate: f64,
        frequencies: &[f64],
        amplitudes: &[f64],
        noise_level: f64,
    ) -> Vec<f64> {
        let n = (duration * sampling_rate) as usize;
        (0..n)
            .map(|i| {
                let t = i as f64 / sampling_rate;
                let mut value = 0.0;
                for (freq, amp) in frequencies.iter().zip(amplitudes.iter()) {
                    value += amp * (2.0 * std::f64::consts::PI * freq * t).sin();
                }
                if noise_level > 0.0 {
                    value += (rand::random::<f64>() - 0.5) * 2.0 * noise_level;
                }
                value
            })
            .collect()
    }

    fn generate_test_leak_signal(
        duration: f64,
        sampling_rate: f64,
        amplitude: f64,
        delay: f64,
    ) -> Vec<f64> {
        let n = (duration * sampling_rate) as usize;
        let leak_freq = 50000.0;
        (0..n)
            .map(|i| {
                let t = i as f64 / sampling_rate;
                if t < delay {
                    (rand::random::<f64>() - 0.5) * 0.001
                } else {
                    let decay = (- (t - delay) * 100.0).exp();
                    amplitude
                        * decay
                        * (2.0 * std::f64::consts::PI * leak_freq * t).sin()
                        + (rand::random::<f64>() - 0.5) * 0.005
                }
            })
            .collect()
    }

    #[test]
    fn test_fft_basic_sine_wave() {
        let config = create_test_config();
        let (detector, _) = HydrogenLeakDetector::new(config);

        let frequency = 5.0;
        let amplitude = 1.0;
        let sampling_rate = 100.0;
        let duration = 1.0;
        let signal = generate_test_signal(duration, 1.0, &[frequency * 0.01], &[amplitude], 0.0);

        let fft_result = detector.fft(&signal);

        assert!(!fft_result.is_empty());

        let mut max_mag = 0.0;
        let mut max_freq = 0.0;
        for (freq, mag) in &fft_result {
            if *mag > max_mag {
                max_mag = *mag;
                max_freq = *freq;
            }
        }

        assert_relative_eq!(max_freq, 0.05, epsilon = 0.02);
        assert!(max_mag > 0.1);
    }

    #[test]
    fn test_fft_multiple_frequencies() {
        let config = create_test_config();
        let (detector, _) = HydrogenLeakDetector::new(config);

        let signal = generate_test_signal(
            1.0,
            1.0,
            &[0.05, 0.2],
            &[1.0, 0.5],
            0.0,
        );

        let fft_result = detector.fft(&signal);

        let frequencies: Vec<f64> = fft_result.iter().map(|(f, _)| *f).collect();
        let magnitudes: Vec<f64> = fft_result.iter().map(|(_, m)| *m).collect();

        assert_relative_eq!(magnitudes[5] / magnitudes[20], 2.0, epsilon = 0.5);
    }

    #[test]
    fn test_fft_empty_signal() {
        let config = create_test_config();
        let (detector, _) = HydrogenLeakDetector::new(config);

        let signal: Vec<f64> = Vec::new();
        let fft_result = detector.fft(&signal);

        assert!(fft_result.is_empty());
    }

    #[test]
    fn test_calculate_spectral_features_normal() {
        let config = create_test_config();
        let (detector, _) = HydrogenLeakDetector::new(config);

        let signal = generate_test_signal(0.1, 100000.0, &[50000.0], &[0.1], 0.001);

        let data = AcousticEmissionData {
            sensor_id: 1,
            timestamp: Utc::now(),
            signal,
            sampling_rate: 100000.0,
        };

        let features = detector.calculate_spectral_features(&data).unwrap();

        assert!(features.rms > 0.0);
        assert!(features.rms < 0.2);
        assert!(features.peak_amplitude > 0.05);
        assert!(features.peak_frequency > 40000.0);
        assert!(features.peak_frequency < 60000.0);
        assert!(features.kurtosis > -2.0);
    }

    #[test]
    fn test_calculate_spectral_features_empty_signal() {
        let config = create_test_config();
        let (detector, _) = HydrogenLeakDetector::new(config);

        let data = AcousticEmissionData {
            sensor_id: 1,
            timestamp: Utc::now(),
            signal: Vec::new(),
            sampling_rate: 100000.0,
        };

        let result = detector.calculate_spectral_features(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_spectral_features_noise_only() {
        let config = create_test_config();
        let (detector, _) = HydrogenLeakDetector::new(config);

        let signal: Vec<f64> = (0..1000)
            .map(|_| (rand::random::<f64>() - 0.5) * 0.001)
            .collect();

        let data = AcousticEmissionData {
            sensor_id: 1,
            timestamp: Utc::now(),
            signal,
            sampling_rate: 100000.0,
        };

        let features = detector.calculate_spectral_features(&data).unwrap();

        assert!(features.rms < 0.001);
        assert!(features.peak_amplitude < 0.001);
        assert_relative_eq!(features.kurtosis, 0.0, epsilon = 1.0);
    }

    #[test]
    fn test_trilateration_known_position() {
        let config = create_test_config();
        let (detector, _) = HydrogenLeakDetector::new(config);

        let sensor_positions = vec![
            SensorPosition { sensor_id: 1, x: 0.0, y: 0.0, z: 0.0 },
            SensorPosition { sensor_id: 2, x: 1.0, y: 0.0, z: 0.0 },
            SensorPosition { sensor_id: 3, x: 0.5, y: 1.0, z: 0.0 },
            SensorPosition { sensor_id: 4, x: 0.0, y: 1.0, z: 0.0 },
        ];

        let leak_x = 0.3;
        let leak_y = 0.4;
        let leak_z = 0.0;
        let sound_speed = config.sound_speed_hydrogen;

        let mut toa_data = Vec::new();
        for (i, pos) in sensor_positions.iter().enumerate() {
            let dx = leak_x - pos.x;
            let dy = leak_y - pos.y;
            let dz = leak_z - pos.z;
            let distance = (dx * dx + dy * dy + dz * dz).sqrt();
            let toa = distance / sound_speed;
            toa_data.push((pos.sensor_id, toa, 0.5 + i as f64 * 0.1));
        }

        let location = detector
            .trilaterate_leak_position(&toa_data, &sensor_positions)
            .unwrap();

        assert!(location.x > 0.1 && location.x < 0.6);
        assert!(location.y > 0.2 && location.y < 0.6);
        assert!(location.uncertainty > 0.0);
    }

    #[test]
    fn test_trilateration_insufficient_sensors() {
        let config = create_test_config();
        let (detector, _) = HydrogenLeakDetector::new(config);

        let sensor_positions = vec![
            SensorPosition { sensor_id: 1, x: 0.0, y: 0.0, z: 0.0 },
            SensorPosition { sensor_id: 2, x: 1.0, y: 0.0, z: 0.0 },
        ];

        let toa_data = vec![(1, 0.0, 0.5), (2, 0.001, 0.4)];

        let result = detector.trilaterate_leak_position(&toa_data, &sensor_positions);
        assert!(result.is_err());
    }

    #[test]
    fn test_trilateration_missing_positions() {
        let config = create_test_config();
        let (detector, _) = HydrogenLeakDetector::new(config);

        let sensor_positions = vec![
            SensorPosition { sensor_id: 1, x: 0.0, y: 0.0, z: 0.0 },
        ];

        let toa_data = vec![(1, 0.0, 0.5), (99, 0.001, 0.4), (100, 0.002, 0.3)];

        let result = detector.trilaterate_leak_position(&toa_data, &sensor_positions);
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_leak_rate_proportional() {
        let config = create_test_config();
        let (detector, _) = HydrogenLeakDetector::new(config);

        let rate1 = detector.calculate_leak_rate(0.1, 0.05);
        let rate2 = detector.calculate_leak_rate(0.2, 0.05);

        assert_relative_eq!(rate2 / rate1, 2.0, epsilon = 0.1);
        assert!(rate1 > 0.0);
    }

    #[test]
    fn test_calculate_leak_rate_with_uncertainty() {
        let config = create_test_config();
        let (detector, _) = HydrogenLeakDetector::new(config);

        let rate_low_uncertainty = detector.calculate_leak_rate(0.1, 0.01);
        let rate_high_uncertainty = detector.calculate_leak_rate(0.1, 0.2);

        assert!(rate_low_uncertainty > rate_high_uncertainty);
    }

    #[test]
    fn test_calculate_diffusion_radius_basic() {
        let config = create_test_config();
        let (detector, _) = HydrogenLeakDetector::new(config);

        let radius = detector.calculate_diffusion_radius(1.0, 60.0);

        assert!(radius > 0.0);
        assert!(radius <= 5.0);
    }

    #[test]
    fn test_calculate_diffusion_radius_time_dependence() {
        let config = create_test_config();
        let (detector, _) = HydrogenLeakDetector::new(config);

        let radius_short = detector.calculate_diffusion_radius(0.5, 10.0);
        let radius_long = detector.calculate_diffusion_radius(0.5, 300.0);

        assert!(radius_long > radius_short);
    }

    #[test]
    fn test_calculate_diffusion_radius_edge_cases() {
        let config = create_test_config();
        let (detector, _) = HydrogenLeakDetector::new(config);

        assert_eq!(detector.calculate_diffusion_radius(0.0, 60.0), 0.0);
        assert_eq!(detector.calculate_diffusion_radius(1.0, 0.0), 0.0);
        assert_eq!(detector.calculate_diffusion_radius(-1.0, 60.0), 0.0);
    }

    #[test]
    fn test_determine_leak_severity_critical() {
        let config = create_test_config();
        let (detector, _) = HydrogenLeakDetector::new(config);

        let severity = detector.determine_leak_severity(2.0, 3.0);
        assert_eq!(severity, DegradationSeverity::Critical);
    }

    #[test]
    fn test_determine_leak_severity_low() {
        let config = create_test_config();
        let (detector, _) = HydrogenLeakDetector::new(config);

        let severity = detector.determine_leak_severity(0.005, 0.05);
        assert_eq!(severity, DegradationSeverity::Low);
    }

    #[test]
    fn test_determine_leak_severity_medium() {
        let config = create_test_config();
        let (detector, _) = HydrogenLeakDetector::new(config);

        let severity = detector.determine_leak_severity(0.05, 0.3);
        assert_eq!(severity, DegradationSeverity::Medium);
    }

    #[test]
    fn test_calibrate_leak_rate_normal() {
        let config = create_test_config();
        let (detector, _) = HydrogenLeakDetector::new(config);

        let calibrated = detector.calibrate_leak_rate(0.05, 10.0);
        assert_relative_eq!(calibrated, 0.05, epsilon = 0.01);
    }

    #[test]
    fn test_calibrate_leak_rate_cap_at_reference() {
        let config = create_test_config();
        let (detector, _) = HydrogenLeakDetector::new(config);

        let calibrated = detector.calibrate_leak_rate(5.0, 10.0);
        assert!(calibrated <= 1.0);
    }

    #[test]
    fn test_calibrate_leak_rate_invalid_reference() {
        let config = create_test_config();
        let (detector, _) = HydrogenLeakDetector::new(config);

        let calibrated = detector.calibrate_leak_rate(0.05, 0.0);
        assert_relative_eq!(calibrated, 0.05, epsilon = 0.01);
    }

    #[tokio::test]
    async fn test_detect_leaks_with_leak() {
        let config = create_test_config();
        let (detector, _rx) = HydrogenLeakDetector::new(config.clone());

        let sensor_positions = vec![
            SensorPosition { sensor_id: 1, x: 0.0, y: 0.0, z: 0.0 },
            SensorPosition { sensor_id: 2, x: 1.0, y: 0.0, z: 0.0 },
            SensorPosition { sensor_id: 3, x: 0.5, y: 1.0, z: 0.0 },
            SensorPosition { sensor_id: 4, x: 0.0, y: 1.0, z: 0.0 },
        ];

        let leak_x = 0.4;
        let leak_y = 0.5;
        let leak_z = 0.0;
        let sound_speed = config.sound_speed_hydrogen;

        let mut acoustic_data = Vec::new();
        for (i, pos) in sensor_positions.iter().enumerate() {
            let dx = leak_x - pos.x;
            let dy = leak_y - pos.y;
            let dz = leak_z - pos.z;
            let distance = (dx * dx + dy * dy + dz * dz).sqrt();
            let delay = distance / sound_speed;
            let amplitude = 0.3 / (1.0 + distance * 2.0);

            let signal = generate_test_leak_signal(0.1, 100000.0, amplitude, delay);
            acoustic_data.push(AcousticEmissionData {
                sensor_id: pos.sensor_id,
                timestamp: Utc::now(),
                signal,
                sampling_rate: 100000.0,
            });
        }

        let request = LeakDetectionRequest {
            electrolyzer_id: 1,
            acoustic_data,
            sensor_positions: sensor_positions.clone(),
            flow_rate_reference: Some(100.0),
        };

        let leaks = detector.detect_leaks(request).await.unwrap();

        assert!(!leaks.is_empty());
        assert_eq!(leaks[0].electrolyzer_id, 1);
        assert!(leaks[0].leak_rate > 0.0);
        assert!(leaks[0].diffusion_radius > 0.0);
        assert!(!leaks[0].acknowledged);
        assert!(!leaks[0].resolved);
        assert!(leaks[0].location.x > 0.0 && leaks[0].location.x < 1.0);
        assert!(leaks[0].location.y > 0.0 && leaks[0].location.y < 1.0);
    }

    #[tokio::test]
    async fn test_detect_leaks_no_leak() {
        let config = create_test_config();
        let (detector, _rx) = HydrogenLeakDetector::new(config);

        let sensor_positions = vec![
            SensorPosition { sensor_id: 1, x: 0.0, y: 0.0, z: 0.0 },
            SensorPosition { sensor_id: 2, x: 1.0, y: 0.0, z: 0.0 },
            SensorPosition { sensor_id: 3, x: 0.5, y: 1.0, z: 0.0 },
        ];

        let mut acoustic_data = Vec::new();
        for i in 0..3 {
            let signal: Vec<f64> = (0..1000)
                .map(|_| (rand::random::<f64>() - 0.5) * 0.0005)
                .collect();
            acoustic_data.push(AcousticEmissionData {
                sensor_id: i + 1,
                timestamp: Utc::now(),
                signal,
                sampling_rate: 100000.0,
            });
        }

        let request = LeakDetectionRequest {
            electrolyzer_id: 1,
            acoustic_data,
            sensor_positions,
            flow_rate_reference: None,
        };

        let leaks = detector.detect_leaks(request).await.unwrap();
        assert!(leaks.is_empty());
    }

    #[tokio::test]
    async fn test_detect_leaks_insufficient_active_sensors() {
        let config = create_test_config();
        let (detector, _rx) = HydrogenLeakDetector::new(config.clone());

        let sensor_positions = vec![
            SensorPosition { sensor_id: 1, x: 0.0, y: 0.0, z: 0.0 },
            SensorPosition { sensor_id: 2, x: 1.0, y: 0.0, z: 0.0 },
        ];

        let mut acoustic_data = Vec::new();
        for i in 0..2 {
            let signal = generate_test_leak_signal(0.1, 100000.0, 0.2, 0.001);
            acoustic_data.push(AcousticEmissionData {
                sensor_id: i + 1,
                timestamp: Utc::now(),
                signal,
                sampling_rate: 100000.0,
            });
        }

        let request = LeakDetectionRequest {
            electrolyzer_id: 1,
            acoustic_data,
            sensor_positions,
            flow_rate_reference: None,
        };

        let leaks = detector.detect_leaks(request).await.unwrap();
        assert!(leaks.is_empty());
    }

    #[test]
    fn test_estimate_time_of_arrival() {
        let config = create_test_config();
        let (detector, _) = HydrogenLeakDetector::new(config);

        let mut signal = vec![0.0; 500];
        for i in 100..500 {
            signal[i] = 0.1 * (i as f64 * 0.01).sin();
        }

        let data = AcousticEmissionData {
            sensor_id: 1,
            timestamp: Utc::now(),
            signal,
            sampling_rate: 1000.0,
        };

        let toa = detector.estimate_time_of_arrival(&data);
        assert!(toa > 0.09);
        assert!(toa < 0.11);
    }

    #[test]
    fn test_generate_leak_animations() {
        let leak = HydrogenLeak {
            id: Uuid::new_v4(),
            electrolyzer_id: 1,
            timestamp: Utc::now(),
            location: LeakLocation {
                x: 0.3,
                y: 0.7,
                z: 0.0,
                uncertainty: 0.05,
            },
            leak_rate: 0.5,
            diffusion_radius: 0.2,
            severity: DegradationSeverity::High,
            spectral_features: SpectralFeatures {
                rms: 0.05,
                peak_frequency: 50000.0,
                spectral_centroid: 45000.0,
                kurtosis: 2.0,
                peak_amplitude: 0.15,
            },
            acknowledged: false,
            resolved: false,
        };

        let animations = HydrogenLeakDetector::generate_leak_animations(&[leak.clone()]);

        assert_eq!(animations.len(), 1);
        assert_eq!(animations[0].leak_id, leak.id);
        assert_eq!(animations[0].x, 0.3);
        assert_eq!(animations[0].y, 0.7);
        assert_eq!(animations[0].severity, DegradationSeverity::High);
        assert!(animations[0].max_radius > 0.0);
    }

    #[test]
    fn test_generate_leak_animations_empty() {
        let animations = HydrogenLeakDetector::generate_leak_animations(&[]);
        assert!(animations.is_empty());
    }

    #[test]
    fn test_cooley_tukey_fft_power_of_two() {
        let config = create_test_config();
        let (detector, _) = HydrogenLeakDetector::new(config);

        let n = 8;
        let signal: Vec<f64> = (0..n).map(|i| (i as f64 * 0.5).sin()).collect();

        let result = detector.cooley_tukey_fft(&signal, false);
        assert_eq!(result.len(), n);

        for &(re, im) in &result {
            assert!(!re.is_nan());
            assert!(!im.is_nan());
        }
    }

    #[test]
    fn test_cooley_tukey_fft_single_point() {
        let config = create_test_config();
        let (detector, _) = HydrogenLeakDetector::new(config);

        let signal = vec![42.0];
        let result = detector.cooley_tukey_fft(&signal, false);

        assert_eq!(result.len(), 1);
        assert_relative_eq!(result[0].0, 42.0, epsilon = 1e-10);
        assert_relative_eq!(result[0].1, 0.0, epsilon = 1e-10);
    }
}
