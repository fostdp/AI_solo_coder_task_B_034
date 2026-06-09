use crate::config::{AlertConfig, OpcUaConfig};
use crate::db::Database;
use crate::models::*;
use chrono::{DateTime, Duration, Utc};
use log::{debug, error, info, warn};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration as StdDuration, Instant};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}

struct OpcUaClient {
    config: OpcUaConfig,
    connection_state: Arc<RwLock<ConnectionState>>,
    alert_tx: mpsc::Sender<Alert>,
    heartbeat_task: Option<JoinHandle<()>>,
    reconnect_task: Option<JoinHandle<()>>,
    last_heartbeat: Arc<RwLock<Instant>>,
    reconnect_attempts: Arc<RwLock<u32>>,
}

impl OpcUaClient {
    fn new(config: OpcUaConfig) -> Self {
        let (alert_tx, _alert_rx) = mpsc::channel::<Alert>(config.alert_queue_capacity);
        
        Self {
            config,
            connection_state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            alert_tx,
            heartbeat_task: None,
            reconnect_task: None,
            last_heartbeat: Arc::new(RwLock::new(Instant::now())),
            reconnect_attempts: Arc::new(RwLock::new(0)),
        }
    }

    async fn connect(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Connecting to OPC UA server at {}", self.config.server_url);
        
        *self.connection_state.write() = ConnectionState::Connecting;
        
        match self.attempt_connection().await {
            Ok(_) => {
                info!("Successfully connected to OPC UA server");
                *self.connection_state.write() = ConnectionState::Connected;
                *self.last_heartbeat.write() = Instant::now();
                *self.reconnect_attempts.write() = 0;
                
                self.start_heartbeat_task();
                Ok(())
            }
            Err(e) => {
                error!("Failed to connect to OPC UA server: {}", e);
                *self.connection_state.write() = ConnectionState::Disconnected;
                self.start_reconnect_task();
                Err(e)
            }
        }
    }

    async fn attempt_connection(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Attempting OPC UA connection to {}", self.config.server_url);
        
        #[cfg(feature = "opcua")]
        {
            let client = opcua::client::ClientBuilder::new()
                .application_name("PEM Electrolyzer Monitor")
                .application_uri("urn:pem-monitor")
                .create_sample_identity_token()
                .client()
                .map_err(|e| format!("OPC UA client build error: {}", e))?;
            
            let _session = client
                .connect_to_endpoint(
                    self.config.server_url.as_str(),
                    opcua::client::IdentityToken::Anonymous,
                )
                .await
                .map_err(|e| format!("OPC UA connect error: {}", e))?;
        }
        
        #[cfg(not(feature = "opcua"))]
        {
            tokio::time::sleep(StdDuration::from_millis(500)).await;
        }
        
        Ok(())
    }

    fn start_heartbeat_task(&mut self) {
        let connection_state = self.connection_state.clone();
        let last_heartbeat = self.last_heartbeat.clone();
        let config = self.config.clone();
        let server_url = self.config.server_url.clone();
        
        let handle = tokio::spawn(async move {
            info!("OPC UA heartbeat task started");
            
            loop {
                tokio::time::sleep(StdDuration::from_secs(config.heartbeat_interval_secs)).await;
                
                let state = *connection_state.read();
                if state != ConnectionState::Connected {
                    debug!("Heartbeat task exiting: connection state is {:?}", state);
                    break;
                }
                
                let elapsed = last_heartbeat.read().elapsed();
                if elapsed.as_secs() > config.heartbeat_timeout_secs {
                    warn!(
                        "OPC UA heartbeat timeout: no response for {}s, connection may be lost",
                        elapsed.as_secs()
                    );
                    
                    *connection_state.write() = ConnectionState::Disconnected;
                    break;
                }
                
                debug!("OPC UA heartbeat OK (last: {:?} ago)", elapsed);
            }
            
            info!("OPC UA heartbeat task stopped");
        });
        
        self.heartbeat_task = Some(handle);
    }

    fn start_reconnect_task(&mut self) {
        if self.reconnect_task.is_some() {
            debug!("Reconnect task already running");
            return;
        }
        
        let connection_state = self.connection_state.clone();
        let reconnect_attempts = self.reconnect_attempts.clone();
        let server_url = self.config.server_url.clone();
        let config = self.config.clone();
        let last_heartbeat = self.last_heartbeat.clone();
        
        let handle = tokio::spawn(async move {
            info!("OPC UA reconnect task started");
            let mut attempt = 0u32;
            
            loop {
                *reconnect_attempts.write() = attempt;
                
                let state = *connection_state.read();
                if state == ConnectionState::Connected {
                    info!("Connection restored, reconnect task exiting");
                    break;
                }
                
                if config.max_reconnect_attempts > 0 && attempt >= config.max_reconnect_attempts {
                    error!("Max reconnect attempts ({}) reached, giving up", config.max_reconnect_attempts);
                    *connection_state.write() = ConnectionState::Disconnected;
                    break;
                }
                
                let delay_ms = calculate_exponential_backoff(attempt, config.initial_reconnect_delay_ms, config.max_reconnect_delay_ms);
                
                info!(
                    "Reconnect attempt {} in {}ms",
                    attempt + 1,
                    delay_ms
                );
                
                tokio::time::sleep(StdDuration::from_millis(delay_ms)).await;
                
                *connection_state.write() = ConnectionState::Reconnecting;
                
                match Self::attempt_connection_static(&server_url).await {
                    Ok(_) => {
                        info!("Successfully reconnected to OPC UA server after {} attempts", attempt + 1);
                        *connection_state.write() = ConnectionState::Connected;
                        *reconnect_attempts.write() = 0;
                        *last_heartbeat.write() = Instant::now();
                        break;
                    }
                    Err(e) => {
                        error!("Reconnect attempt {} failed: {}", attempt + 1, e);
                        *connection_state.write() = ConnectionState::Disconnected;
                        attempt += 1;
                    }
                }
            }
            
            info!("OPC UA reconnect task stopped");
        });
        
        self.reconnect_task = Some(handle);
    }

    async fn attempt_connection_static(
        server_url: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Attempting OPC UA connection to {}", server_url);
        
        #[cfg(feature = "opcua")]
        {
            let client = opcua::client::ClientBuilder::new()
                .application_name("PEM Electrolyzer Monitor")
                .application_uri("urn:pem-monitor")
                .create_sample_identity_token()
                .client()
                .map_err(|e| format!("OPC UA client build error: {}", e))?;
            
            let _session = client
                .connect_to_endpoint(
                    server_url,
                    opcua::client::IdentityToken::Anonymous,
                )
                .await
                .map_err(|e| format!("OPC UA connect error: {}", e))?;
        }
        
        #[cfg(not(feature = "opcua"))]
        {
            tokio::time::sleep(StdDuration::from_millis(500)).await;
        }
        
        Ok(())
    }

    async fn send_alert(
        &self,
        alert: &Alert,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let state = *self.connection_state.read();
        
        if state != ConnectionState::Connected {
            warn!(
                "OPC UA not connected (state: {:?}), alert {} will be sent after reconnection",
                state, alert.id
            );
            return Ok(());
        }
        
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

        *self.last_heartbeat.write() = Instant::now();
        
        Ok(())
    }

    fn get_connection_status(&self) -> OpcUaConnectionStatus {
        let state = *self.connection_state.read();
        let attempts = *self.reconnect_attempts.read();
        
        OpcUaConnectionStatus {
            connected: state == ConnectionState::Connected,
            state: format!("{:?}", state),
            reconnect_attempts: attempts,
            last_heartbeat_seconds: self.last_heartbeat.read().elapsed().as_secs(),
        }
    }
}

fn calculate_exponential_backoff(attempt: u32, initial_delay_ms: u64, max_delay_ms: u64) -> u64 {
    let delay = initial_delay_ms * 2u64.pow(attempt.min(10));
    delay.min(max_delay_ms)
}

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone, Default, Copy)]
struct AlertState {
    voltage: AlertConditionState,
    purity: AlertConditionState,
    conductivity: AlertConditionState,
}

#[derive(Debug, Clone)]
pub struct AlarmBridge {
    db: Database,
    alert_config: AlertConfig,
    states: Arc<RwLock<HashMap<u8, AlertState>>>,
    opcua_client: Option<OpcUaClient>,
    alert_tx: mpsc::Sender<Alert>,
}

impl AlarmBridge {
    pub fn new(
        db: Database,
        alert_config: AlertConfig,
        opcua_config: Option<OpcUaConfig>,
    ) -> (Self, mpsc::Receiver<Alert>) {
        let (alert_tx, alert_rx) = mpsc::channel::<Alert>(1000);
        
        let opcua_client = opcua_config.map(OpcUaClient::new);

        (
            Self {
                db,
                alert_config,
                states: Arc::new(RwLock::new(HashMap::new())),
                opcua_client,
                alert_tx,
            },
            alert_rx,
        )
    }

    pub async fn connect_opcua(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(ref mut client) = self.opcua_client {
            client.connect().await?;
        }
        Ok(())
    }

    pub async fn process_sensor_data(
        &self,
        electrolyzer_id: u8,
        max_cell_voltage: f64,
        avg_cell_voltage: f64,
        hydrogen_purity: f64,
        membrane_conductivity: f64,
        timestamp: DateTime<Utc>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut alerts = Vec::new();

        if let Some(alert) = self
            .check_voltage_alert(electrolyzer_id, max_cell_voltage, avg_cell_voltage, timestamp)
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

        for alert in &alerts {
            if let Err(e) = self.alert_tx.send(alert.clone()).await {
                error!("Failed to send alert to channel: {}", e);
            }
        }

        Ok(())
    }

    pub async fn process_alert(&self, alert: &Alert) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.handle_alert(alert).await;
        Ok(())
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

        if max_voltage > self.alert_config.voltage_threshold {
            if state.voltage.start_time.is_none() {
                state.voltage.start_time = Some(timestamp);
                info!(
                    "Electrolyzer {} voltage {:.3}V exceeds threshold {:.3}V, starting timer",
                    electrolyzer_id, max_voltage, self.alert_config.voltage_threshold
                );
            }

            if !state.voltage.triggered {
                if let Some(start_time) = state.voltage.start_time {
                    let duration = timestamp - start_time;
                    if duration.num_seconds() >= self.alert_config.voltage_duration_seconds {
                        let alert = Alert {
                            id: Uuid::new_v4(),
                            timestamp,
                            electrolyzer_id,
                            alert_level: AlertLevel::Level1,
                            alert_type: "high_voltage".to_string(),
                            message: format!(
                                "Electrolyzer {} voltage exceeded {:.3}V for more than {} seconds",
                                electrolyzer_id, self.alert_config.voltage_threshold, self.alert_config.voltage_duration_seconds
                            ),
                            value: max_voltage,
                            threshold: self.alert_config.voltage_threshold,
                            acknowledged: false,
                            resolved: false,
                        };

                        state.voltage.triggered = true;
                        state.voltage.last_alert_id = Some(alert.id);

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

        if hydrogen_purity < self.alert_config.purity_threshold {
            if state.purity.start_time.is_none() {
                state.purity.start_time = Some(timestamp);
                info!(
                    "Electrolyzer {} hydrogen purity {:.3}% below threshold {:.3}%, starting timer",
                    electrolyzer_id, hydrogen_purity, self.alert_config.purity_threshold
                );
            }

            if !state.purity.triggered {
                if let Some(start_time) = state.purity.start_time {
                    let duration = timestamp - start_time;
                    if duration.num_seconds() >= self.alert_config.purity_duration_seconds {
                        let alert = Alert {
                            id: Uuid::new_v4(),
                            timestamp,
                            electrolyzer_id,
                            alert_level: AlertLevel::Level2,
                            alert_type: "low_hydrogen_purity".to_string(),
                            message: format!(
                                "Electrolyzer {} hydrogen purity dropped below {:.3}% for more than {} seconds",
                                electrolyzer_id, self.alert_config.purity_threshold, self.alert_config.purity_duration_seconds
                            ),
                            value: hydrogen_purity,
                            threshold: self.alert_config.purity_threshold,
                            acknowledged: false,
                            resolved: false,
                        };

                        state.purity.triggered = true;
                        state.purity.last_alert_id = Some(alert.id);

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

            if degradation_percent > self.alert_config.conductivity_degradation_threshold && !state.conductivity.triggered
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
                    threshold: baseline * (1.0 - self.alert_config.conductivity_degradation_threshold / 100.0),
                    acknowledged: false,
                    resolved: false,
                };

                state.conductivity.triggered = true;
                state.conductivity.last_alert_id = Some(alert.id);

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

    pub fn get_opcua_status(&self) -> Option<OpcUaConnectionStatus> {
        self.opcua_client.as_ref().map(|c| c.get_connection_status())
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OpcUaConnectionStatus {
    pub connected: bool,
    pub state: String,
    pub reconnect_attempts: u32,
    pub last_heartbeat_seconds: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AlertStateSummary {
    pub voltage_exceeded: bool,
    pub voltage_duration: Option<i64>,
    pub purity_exceeded: bool,
    pub purity_duration: Option<i64>,
    pub conductivity_degraded: bool,
    pub baseline_conductivity: Option<f64>,
    pub current_conductivity: f64,
}
