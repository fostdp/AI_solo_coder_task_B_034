use crate::models::*;
use chrono::{DateTime, Duration, Utc};
use clickhouse::Client;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("ClickHouse error: {0}")]
    ClickHouseError(#[from] clickhouse::error::Error),
    #[error("Data conversion error: {0}")]
    ConversionError(String),
}

pub type Result<T> = std::result::Result<T, DbError>;

#[derive(Clone)]
pub struct Database {
    client: Arc<Client>,
    database: String,
}

impl Database {
    pub fn new(url: &str, user: &str, password: &str, database: &str) -> Self {
        let client = Client::default()
            .with_url(url)
            .with_user(user)
            .with_password(password)
            .with_database(database);

        Self {
            client: Arc::new(client),
            database: database.to_string(),
        }
    }

    pub async fn insert_sensor_data(&self, data: &[SensorData]) -> Result<()> {
        let mut insert = self.client.insert("sensor_data")?;
        for d in data {
            insert.write(d).await?;
        }
        insert.end().await?;
        Ok(())
    }

    pub async fn insert_electrolyzer_status(&self, status: &ElectrolyzerStatus) -> Result<()> {
        let mut insert = self.client.insert("electrolyzer_status")?;
        insert.write(status).await?;
        insert.end().await?;
        Ok(())
    }

    pub async fn insert_alert(&self, alert: &Alert) -> Result<()> {
        let mut insert = self.client.insert("alerts")?;
        insert.write(alert).await?;
        insert.end().await?;
        Ok(())
    }

    pub async fn insert_optimization_suggestion(
        &self,
        suggestion: &OptimizationSuggestion,
    ) -> Result<()> {
        let mut insert = self.client.insert("optimization_suggestions")?;
        insert.write(suggestion).await?;
        insert.end().await?;
        Ok(())
    }

    pub async fn insert_efficiency_history(&self, history: &EfficiencyHistory) -> Result<()> {
        let mut insert = self.client.insert("efficiency_history")?;
        insert.write(history).await?;
        insert.end().await?;
        Ok(())
    }

    pub async fn insert_system_summary(&self, summary: &SystemSummary) -> Result<()> {
        let mut insert = self.client.insert("system_summary")?;
        insert.write(summary).await?;
        insert.end().await?;
        Ok(())
    }

    pub async fn get_sensor_data_range(
        &self,
        electrolyzer_id: u8,
        sensor_id: u16,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<SensorData>> {
        let data = self
            .client
            .query(
                "SELECT timestamp, electrolyzer_id, sensor_id, sensor_type, 
                 location, value, rated_value, x, y
                 FROM sensor_data
                 WHERE electrolyzer_id = ? AND sensor_id = ? 
                 AND timestamp BETWEEN ? AND ?
                 ORDER BY timestamp",
            )
            .bind(electrolyzer_id)
            .bind(sensor_id)
            .bind(start)
            .bind(end)
            .fetch_all::<SensorData>()
            .await?;
        Ok(data)
    }

    pub async fn get_latest_sensor_data(
        &self,
        electrolyzer_id: u8,
    ) -> Result<Vec<SensorData>> {
        let data = self
            .client
            .query(
                "SELECT timestamp, electrolyzer_id, sensor_id, sensor_type,
                 location, value, rated_value, x, y
                 FROM sensor_data
                 WHERE electrolyzer_id = ?
                 ORDER BY timestamp DESC
                 LIMIT 50",
            )
            .bind(electrolyzer_id)
            .fetch_all::<SensorData>()
            .await?;
        Ok(data)
    }

    pub async fn get_sensor_trend(
        &self,
        electrolyzer_id: u8,
        sensor_type: &str,
        hours: i64,
    ) -> Result<Vec<SensorTrendData>> {
        let start = Utc::now() - Duration::hours(hours);
        let data = self
            .client
            .query(
                "SELECT timestamp, avg(value) as value
                 FROM sensor_data
                 WHERE electrolyzer_id = ? AND sensor_type = ?
                 AND timestamp >= ?
                 GROUP BY timestamp
                 ORDER BY timestamp",
            )
            .bind(electrolyzer_id)
            .bind(sensor_type)
            .bind(start)
            .fetch_all::<SensorTrendData>()
            .await?;
        Ok(data)
    }

    pub async fn get_efficiency_history_range(
        &self,
        electrolyzer_id: u8,
        hours: i64,
    ) -> Result<Vec<EfficiencyHistory>> {
        let start = Utc::now() - Duration::hours(hours);
        let data = self
            .client
            .query(
                "SELECT timestamp, electrolyzer_id, current_density, 
                 cell_voltage, efficiency, water_temp
                 FROM efficiency_history
                 WHERE electrolyzer_id = ? AND timestamp >= ?
                 ORDER BY timestamp",
            )
            .bind(electrolyzer_id)
            .bind(start)
            .fetch_all::<EfficiencyHistory>()
            .await?;
        Ok(data)
    }

    pub async fn get_active_alerts(&self) -> Result<Vec<Alert>> {
        let data = self
            .client
            .query(
                "SELECT id, timestamp, electrolyzer_id, alert_level, 
                 alert_type, message, value, threshold, acknowledged, resolved
                 FROM alerts
                 WHERE resolved = false
                 ORDER BY timestamp DESC
                 LIMIT 100",
            )
            .fetch_all::<Alert>()
            .await?;
        Ok(data)
    }

    pub async fn get_alerts_by_electrolyzer(
        &self,
        electrolyzer_id: u8,
        hours: i64,
    ) -> Result<Vec<Alert>> {
        let start = Utc::now() - Duration::hours(hours);
        let data = self
            .client
            .query(
                "SELECT id, timestamp, electrolyzer_id, alert_level,
                 alert_type, message, value, threshold, acknowledged, resolved
                 FROM alerts
                 WHERE electrolyzer_id = ? AND timestamp >= ?
                 ORDER BY timestamp DESC",
            )
            .bind(electrolyzer_id)
            .bind(start)
            .fetch_all::<Alert>()
            .await?;
        Ok(data)
    }

    pub async fn get_latest_optimization_suggestions(
        &self,
        limit: u32,
    ) -> Result<Vec<OptimizationSuggestion>> {
        let data = self
            .client
            .query(
                "SELECT id, timestamp, electrolyzer_id, current_efficiency,
                 optimized_current_density, optimized_water_temp, 
                 expected_efficiency, applied
                 FROM optimization_suggestions
                 ORDER BY timestamp DESC
                 LIMIT ?",
            )
            .bind(limit)
            .fetch_all::<OptimizationSuggestion>()
            .await?;
        Ok(data)
    }

    pub async fn get_system_summary_last_hour(&self) -> Result<SystemSummary> {
        let data = self
            .client
            .query(
                "SELECT timestamp, total_hydrogen, avg_efficiency, 
                 total_power, active_electrolyzers
                 FROM system_summary
                 ORDER BY timestamp DESC
                 LIMIT 1",
            )
            .fetch_one::<SystemSummary>()
            .await?;
        Ok(data)
    }

    pub async fn get_total_hydrogen_production(
        &self,
        hours: i64,
    ) -> Result<f64> {
        let start = Utc::now() - Duration::hours(hours);
        let result: Option<f64> = self
            .client
            .query(
                "SELECT sum(total_hydrogen_production)
                 FROM electrolyzer_status
                 WHERE timestamp >= ?",
            )
            .bind(start)
            .fetch_optional()
            .await?;
        Ok(result.unwrap_or(0.0))
    }

    pub async fn get_average_efficiency(&self, hours: i64) -> Result<f64> {
        let start = Utc::now() - Duration::hours(hours);
        let result: Option<f64> = self
            .client
            .query(
                "SELECT avg(average_efficiency)
                 FROM electrolyzer_status
                 WHERE timestamp >= ?",
            )
            .bind(start)
            .fetch_optional()
            .await?;
        Ok(result.unwrap_or(0.0))
    }

    pub async fn get_total_power_consumption(
        &self,
        hours: i64,
    ) -> Result<f64> {
        let start = Utc::now() - Duration::hours(hours);
        let result: Option<f64> = self
            .client
            .query(
                "SELECT sum(total_power_consumption)
                 FROM electrolyzer_status
                 WHERE timestamp >= ?",
            )
            .bind(start)
            .fetch_optional()
            .await?;
        Ok(result.unwrap_or(0.0))
    }

    pub async fn acknowledge_alert(&self, alert_id: Uuid) -> Result<()> {
        self.client
            .query("ALTER TABLE alerts UPDATE acknowledged = true WHERE id = ?")
            .bind(alert_id)
            .execute()
            .await?;
        Ok(())
    }

    pub async fn resolve_alert(&self, alert_id: Uuid) -> Result<()> {
        self.client
            .query("ALTER TABLE alerts UPDATE resolved = true WHERE id = ?")
            .bind(alert_id)
            .execute()
            .await?;
        Ok(())
    }

    pub async fn apply_optimization_suggestion(&self, suggestion_id: Uuid) -> Result<()> {
        self.client
            .query("ALTER TABLE optimization_suggestions UPDATE applied = true WHERE id = ?")
            .bind(suggestion_id)
            .execute()
            .await?;
        Ok(())
    }

    pub async fn get_electrolyzer_status(
        &self,
        electrolyzer_id: u8,
    ) -> Result<Option<ElectrolyzerStatus>> {
        let result = self
            .client
            .query(
                "SELECT timestamp, electrolyzer_id, total_hydrogen_production,
                 average_efficiency, total_power_consumption, cell_voltage,
                 current_density, water_temp, hydrogen_purity, membrane_conductivity
                 FROM electrolyzer_status
                 WHERE electrolyzer_id = ?
                 ORDER BY timestamp DESC
                 LIMIT 1",
            )
            .bind(electrolyzer_id)
            .fetch_optional::<ElectrolyzerStatus>()
            .await?;
        Ok(result)
    }
}
