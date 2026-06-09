use metrics::{counter, gauge, histogram};
use metrics_exporter_prometheus::PrometheusBuilder;
use std::sync::OnceLock;
use std::time::Instant;

static METRICS_INITIALIZED: OnceLock<()> = OnceLock::new();

pub fn init_metrics() {
    METRICS_INITIALIZED.get_or_init(|| {
        let builder = PrometheusBuilder::new()
            .set_buckets_for_metric(
                metrics_exporter_prometheus::Matcher::Suffix("_duration_seconds".to_string()),
                &[0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0],
            )
            .expect("Failed to set buckets");

        let handle = builder.install_recorder().expect("Failed to install recorder");

        tokio::spawn(async move {
            use axum::{routing::get, Router};
            let app = Router::new().route(
                "/metrics",
                get(|| async move { handle.render() }),
            );

            let listener = tokio::net::TcpListener::bind("0.0.0.0:9000").await.unwrap();
            tracing::info!("Prometheus metrics server listening on 0.0.0.0:9000/metrics");
            axum::serve(listener, app).await.unwrap();
        });
    });
}

pub struct MetricsTimer {
    start: Instant,
    name: &'static str,
}

impl MetricsTimer {
    pub fn new(name: &'static str) -> Self {
        Self {
            start: Instant::now(),
            name,
        }
    }
}

impl Drop for MetricsTimer {
    fn drop(&mut self) {
        let duration = self.start.elapsed().as_secs_f64();
        histogram!(self.name, duration);
    }
}

#[macro_export]
macro_rules! time_metric {
    ($name:expr) => {
        let _timer = $crate::metrics::MetricsTimer::new($name);
    };
}

pub fn increment_profinet_packets_received() {
    counter!("profinet_packets_received_total", 1);
}

pub fn increment_profinet_packets_dropped(reason: &'static str) {
    counter!("profinet_packets_dropped_total", 1, "reason" => reason);
}

pub fn increment_profinet_crc_errors() {
    counter!("profinet_crc_errors_total", 1);
}

pub fn set_efficiency(electrolyzer_id: u8, efficiency: f64) {
    gauge!("efficiency_percent", efficiency, "electrolyzer_id" => electrolyzer_id.to_string());
}

pub fn set_hydrogen_production(electrolyzer_id: u8, production: f64) {
    gauge!("hydrogen_production_m3h", production, "electrolyzer_id" => electrolyzer_id.to_string());
}

pub fn set_power_consumption(electrolyzer_id: u8, power: f64) {
    gauge!("power_consumption_kw", power, "electrolyzer_id" => electrolyzer_id.to_string());
}

pub fn set_current_density(electrolyzer_id: u8, current: f64) {
    gauge!("current_density_a_cm2", current, "electrolyzer_id" => electrolyzer_id.to_string());
}

pub fn set_water_temp(electrolyzer_id: u8, temp: f64) {
    gauge!("water_temp_c", temp, "electrolyzer_id" => electrolyzer_id.to_string());
}

pub fn set_cell_voltage(electrolyzer_id: u8, voltage: f64) {
    gauge!("cell_voltage_v", voltage, "electrolyzer_id" => electrolyzer_id.to_string());
}

pub fn set_membrane_conductivity(electrolyzer_id: u8, conductivity: f64) {
    gauge!("membrane_conductivity_s_cm", conductivity, "electrolyzer_id" => electrolyzer_id.to_string());
}

pub fn set_hydrogen_purity(electrolyzer_id: u8, purity: f64) {
    gauge!("hydrogen_purity_percent", purity, "electrolyzer_id" => electrolyzer_id.to_string());
}

pub fn increment_alerts_generated(level: &'static str) {
    counter!("alerts_generated_total", 1, "level" => level);
}

pub fn increment_alerts_pushed_opcua(success: bool) {
    let status = if success { "success" } else { "failed" };
    counter!("alerts_pushed_opcua_total", 1, "status" => status);
}

pub fn set_opcua_connection_status(connected: bool) {
    let value = if connected { 1.0 } else { 0.0 };
    gauge!("opcua_connection_status", value);
}

pub fn increment_optimization_tasks_submitted() {
    counter!("optimization_tasks_submitted_total", 1);
}

pub fn increment_optimization_tasks_completed(success: bool) {
    let status = if success { "success" } else { "failed" };
    counter!("optimization_tasks_completed_total", 1, "status" => status);
}

pub fn set_optimization_queue_depth(depth: usize) {
    gauge!("optimization_queue_depth", depth as f64);
}

pub fn set_active_electrolyzers(count: u8) {
    gauge!("active_electrolyzers", count as f64);
}

pub fn increment_db_writes(success: bool) {
    let status = if success { "success" } else { "failed" };
    counter!("db_writes_total", 1, "status" => status);
}

pub fn increment_sensor_data_points() {
    counter!("sensor_data_points_total", 1);
}
