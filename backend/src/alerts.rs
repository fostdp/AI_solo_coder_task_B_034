use crate::db::Database;
use crate::models::*;
use chrono::{DateTime, Duration, Utc};
use log::{error, info, warn};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

const VOLTAGE_THRESHOLD: f64 = 2.0;
const VOLTAGE_DURATION_SECONDS: i64 = 300;

const PURITY_THRESHOLD: f64 = 99.9;
const PURITY_DURATION_SECONDS: i64 = 180;

const CONDUCTIVITY_DEGRADATION_THRESHOLD: f64 = 20.0;

#[derive(Debug, Clone)]
struct AlertConditionState {
    start_time: Option<DateTime<Utc>>,
    current_value: f64,
    triggered: bool,
    baseline_conductivity: Option<f64>,
    last_alert_id: Option<Uuid>,
}

impl Default for AlertConditionState {
    fn default() -> Self {
        Self {
            start_time: None,
            current_value: 0.0,
            triggered: false,
            baseline_conductivity: None,
            last_alert_id: None,
        }
    }
}

pub struct AlertManager {
    db: Database,
    states: Arc<RwLock<HashMap<u8, AlertState>>>,
    opcua_client: Option<OpcUaClient>,
}

#[derive(Debug, Clone, Default)]
struct AlertState {
    voltage: AlertConditionState,
    purity: AlertConditionState,
    conductivity: AlertConditionState,
}

struct OpcUaClient {
    server_url: String,
}

impl OpcUaClient {
    fn new(server_url: &str) -> Self {
        Self {
            server_url: server_url.to_string(),
        }
    }

    async fn connect(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Connecting to OPC UA server at {}", self.server_url);
        Ok(())
    }

    async fn send_alert(
        &self,
        alert: &Alert,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let level_str = match alert.alert_level {
            AlertLevel::Level1 => "LEVEL_1",
            AlertLevel::Level2 => "LEVEL_2",
            AlertLevel::Level3 => "LEVEL_3",
        };

        info!(
            "OPC UA Alert [{}] {}: Electrolyzer {} - {} (value: {:.3}, threshold: {:.3})",
            level_str,
            alert.id,
            alert.electrolyzer_id,
            alert.message,
            alert.value,
            alert.threshold
        );

        Ok(())
    }
}

impl AlertManager {
    pub fn new(db: Database, opcua_server_url: Option<&str>) -> Self {
        let opcua_client = opcua_server_url.map(OpcUaClient::new);

        Self {
            db,
            states: Arc::new(RwLock::new(HashMap::new())),
            opcua_client,
        }
    }

    pub async fn process_data(
        &self,
        electrolyzer_id: u8,
        cell_voltage: f64,
        avg_cell_voltage: f64,
        hydrogen_purity: f64,
        membrane_conductivity: f64,
        timestamp: DateTime<Utc>,
    ) -> Vec<Alert> {
        let mut alerts = Vec::new();

        if let Some(alert) = self
            .check_voltage_alert(electrolyzer_id, cell_voltage, avg_cell_voltage, timestamp)
            .await
        {
            alerts.push(alert);
        }

        if let Some(alert) = self
            .check_purity_alert(electrolyzer_id, hydrogen_purity, timestamp)
            .await
        {
            alerts.push(alert);
        }

        if let Some(alert) = self
            .check_conductivity_alert(electrolyzer_id, membrane_conductivity, timestamp)
            .await
        {
            alerts.push(alert);
        }

        alerts
    }

    async fn check_voltage_alert(
        &self,
        electrolyzer_id: u8,
        cell_voltage: f64,
        avg_cell_voltage: f64,
        timestamp: DateTime<Utc>,
    ) -> Option<Alert> {
        let max_voltage = cell_voltage.max(avg_cell_voltage);

        let mut states = self.states.write();
        let state = states
            .entry(electrolyzer_id)
            .or_insert_with(AlertState::default);

        state.voltage.current_value = max_voltage;

        if max_voltage > VOLTAGE_THRESHOLD {
            if state.voltage.start_time.is_none() {
                state.voltage.start_time = Some(timestamp);
                info!(
                    "Electrolyzer {} voltage {:.3}V exceeds threshold {:.3}V, starting timer",
                    electrolyzer_id, max_voltage, VOLTAGE_THRESHOLD
                );
            }

            if !state.voltage.triggered {
                if let Some(start_time) = state.voltage.start_time {
                    let duration = timestamp - start_time;
                    if duration.num_seconds() >= VOLTAGE_DURATION_SECONDS {
                        let alert = Alert {
                            id: Uuid::new_v4(),
                            timestamp,
                            electrolyzer_id,
                            alert_level: AlertLevel::Level1,
                            alert_type: "high_voltage".to_string(),
                            message: format!(
                                "Electrolyzer {} voltage exceeded {:.3}V for more than 5 minutes",
                                electrolyzer_id, VOLTAGE_THRESHOLD
                            ),
                            value: max_voltage,
                            threshold: VOLTAGE_THRESHOLD,
                            acknowledged: false,
                            resolved: false,
                        };

                        state.voltage.triggered = true;
                        state.voltage.last_alert_id = Some(alert.id);

                        self.handle_alert(&alert).await;
                        return Some(alert);
                    }
                }
            }
        } else {
            if state.voltage.start_time.is_some() {
                info!(
                    "Electrolyzer {} voltage {:.3}V normalized, resetting alert timer",
                    electrolyzer_id, max_voltage
                );
            }
            state.voltage.start_time = None;
            state.voltage.triggered = false;
        }

        None
    }

    async fn check_purity_alert(
        &self,
        electrolyzer_id: u8,
        hydrogen_purity: f64,
        timestamp: DateTime<Utc>,
    ) -> Option<Alert> {
        let mut states = self.states.write();
        let state = states
            .entry(electrolyzer_id)
            .or_insert_with(AlertState::default);

        state.purity.current_value = hydrogen_purity;

        if hydrogen_purity < PURITY_THRESHOLD {
            if state.purity.start_time.is_none() {
                state.purity.start_time = Some(timestamp);
                info!(
                    "Electrolyzer {} hydrogen purity {:.3}% below threshold {:.3}%, starting timer",
                    electrolyzer_id, hydrogen_purity, PURITY_THRESHOLD
                );
            }

            if !state.purity.triggered {
                if let Some(start_time) = state.purity.start_time {
                    let duration = timestamp - start_time;
                    if duration.num_seconds() >= PURITY_DURATION_SECONDS {
                        let alert = Alert {
                            id: Uuid::new_v4(),
                            timestamp,
                            electrolyzer_id,
                            alert_level: AlertLevel::Level2,
                            alert_type: "low_hydrogen_purity".to_string(),
                            message: format!(
                                "Electrolyzer {} hydrogen purity dropped below {:.3}% for more than 3 minutes",
                                electrolyzer_id, PURITY_THRESHOLD
                            ),
                            value: hydrogen_purity,
                            threshold: PURITY_THRESHOLD,
                            acknowledged: false,
                            resolved: false,
                        };

                        state.purity.triggered = true;
                        state.purity.last_alert_id = Some(alert.id);

                        self.handle_alert(&alert).await;
                        return Some(alert);
                    }
                }
            }
        } else {
            if state.purity.start_time.is_some() {
                info!(
                    "Electrolyzer {} hydrogen purity {:.3}% normalized, resetting alert timer",
                    electrolyzer_id, hydrogen_purity
                );
            }
            state.purity.start_time = None;
            state.purity.triggered = false;
        }

        None
    }

    async fn check_conductivity_alert(
        &self,
        electrolyzer_id: u8,
        membrane_conductivity: f64,
        timestamp: DateTime<Utc>,
    ) -> Option<Alert> {
        let mut states = self.states.write();
        let state = states
            .entry(electrolyzer_id)
            .or_insert_with(AlertState::default);

        state.conductivity.current_value = membrane_conductivity;

        if state.conductivity.baseline_conductivity.is_none() {
            state.conductivity.baseline_conductivity = Some(membrane_conductivity);
            info!(
                "Electrolyzer {} membrane conductivity baseline set to {:.6} S/cm",
                electrolyzer_id, membrane_conductivity
            );
            return None;
        }

        if let Some(baseline) = state.conductivity.baseline_conductivity {
            let degradation_percent = ((baseline - membrane_conductivity) / baseline) * 100.0;

            if degradation_percent > CONDUCTIVITY_DEGRADATION_THRESHOLD && !state.conductivity.triggered
            {
                let alert = Alert {
                    id: Uuid::new_v4(),
                    timestamp,
                    electrolyzer_id,
                    alert_level: AlertLevel::Level3,
                    alert_type: "membrane_degradation".to_string(),
                    message: format!(
                        "Electrolyzer {} membrane conductivity degraded by {:.1}% from baseline",
                        electrolyzer_id, degradation_percent
                    ),
                    value: membrane_conductivity,
                    threshold: baseline * (1.0 - CONDUCTIVITY_DEGRADATION_THRESHOLD / 100.0),
                    acknowledged: false,
                    resolved: false,
                };

                state.conductivity.triggered = true;
                state.conductivity.last_alert_id = Some(alert.id);

                self.handle_alert(&alert).await;
                return Some(alert);
            }
        }

        None
    }

    async fn handle_alert(&self, alert: &Alert) {
        let level_str = match alert.alert_level {
            AlertLevel::Level1 => "LEVEL 1",
            AlertLevel::Level2 => "LEVEL 2",
            AlertLevel::Level3 => "LEVEL 3",
        };

        warn!(
            "⚠️  [{}] Alert triggered for electrolyzer {}: {}",
            level_str, alert.electrolyzer_id, alert.message
        );

        if let Err(e) = self.db.insert_alert(alert).await {
            error!("Failed to insert alert into database: {}", e);
        }

        if let Some(ref opcua) = self.opcua_client {
            if let Err(e) = opcua.send_alert(alert).await {
                error!("Failed to send alert via OPC UA: {}", e);
            }
        }
    }

    pub fn get_alert_state(&self, electrolyzer_id: u8) -> Option<AlertStateSummary> {
        let states = self.states.read();
        states.get(&electrolyzer_id).map(|state| AlertStateSummary {
            voltage_exceeded: state.voltage.start_time.is_some(),
            voltage_duration: state
                .voltage
                .start_time
                .map(|t| (Utc::now() - t).num_seconds()),
            purity_exceeded: state.purity.start_time.is_some(),
            purity_duration: state
                .purity
                .start_time
                .map(|t| (Utc::now() - t).num_seconds()),
            conductivity_degraded: state.conductivity.triggered,
            baseline_conductivity: state.conductivity.baseline_conductivity,
            current_conductivity: state.conductivity.current_value,
        })
    }

    pub async fn acknowledge_alert(&self, alert_id: Uuid) -> Result<(), String> {
        self.db
            .acknowledge_alert(alert_id)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn resolve_alert(&self, alert_id: Uuid) -> Result<(), String> {
        self.db
            .resolve_alert(alert_id)
            .await
            .map_err(|e| e.to_string())
    }

    pub fn reset_baseline_conductivity(&self, electrolyzer_id: u8) {
        let mut states = self.states.write();
        if let Some(state) = states.get_mut(&electrolyzer_id) {
            state.conductivity.baseline_conductivity = None;
            state.conductivity.triggered = false;
            state.conductivity.start_time = None;
            info!(
                "Reset membrane conductivity baseline for electrolyzer {}",
                electrolyzer_id
            );
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AlertStateSummary {
    pub voltage_exceeded: bool,
    pub voltage_duration: Option<i64>,
    pub purity_exceeded: bool,
    pub purity_duration: Option<i64>,
    pub conductivity_degraded: bool,
    pub baseline_conductivity: Option<f64>,
    pub current_conductivity: f64,
}

#[derive(Debug, Clone)]
pub struct AlertConfig {
    pub voltage_threshold: f64,
    pub voltage_duration: Duration,
    pub purity_threshold: f64,
    pub purity_duration: Duration,
    pub conductivity_degradation_threshold: f64,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            voltage_threshold: VOLTAGE_THRESHOLD,
            voltage_duration: Duration::seconds(VOLTAGE_DURATION_SECONDS),
            purity_threshold: PURITY_THRESHOLD,
            purity_duration: Duration::seconds(PURITY_DURATION_SECONDS),
            conductivity_degradation_threshold: CONDUCTIVITY_DEGRADATION_THRESHOLD,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_config_defaults() {
        let config = AlertConfig::default();
        assert_eq!(config.voltage_threshold, 2.0);
        assert_eq!(config.voltage_duration.num_seconds(), 300);
        assert_eq!(config.purity_threshold, 99.9);
        assert_eq!(config.purity_duration.num_seconds(), 180);
        assert_eq!(config.conductivity_degradation_threshold, 20.0);
    }
}
