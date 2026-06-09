mod alerts;
mod api;
mod db;
mod models;
mod optimization;
mod profinet;

use crate::alerts::AlertManager;
use crate::api::{start_api_server, AppState};
use crate::db::Database;
use crate::models::*;
use crate::optimization::OptimizationService;
use crate::profinet::{ProfinetReceiver, SensorDataBatch};
use chrono::Utc;
use log::{error, info, warn};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

const ACTIVE_AREA: f64 = 1.0;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    info!("=" .repeat(60));
    info!("PEM Electrolyzer Monitoring and Efficiency Optimization Platform");
    info!("=" .repeat(60));

    let config = AppConfig::default();
    info!("Configuration loaded:");
    info!("  Profinet port: {}", config.profinet_port);
    info!("  API port: {}", config.api_port);
    info!("  ClickHouse: {}", config.clickhouse_url);
    info!("  Database: {}", config.clickhouse_database);
    info!("  Electrolyzers: {}", config.electrolyzer_count);
    info!("  Sensors per electrolyzer: {}", config.sensors_per_electrolyzer);
    info!("=" .repeat(60));

    let db = Database::new(
        &config.clickhouse_url,
        &config.clickhouse_user,
        &config.clickhouse_password,
        &config.clickhouse_database,
    );

    let alert_manager = AlertManager::new(db.clone(), Some(&config.opcua_server_url));
    let optimization_service = OptimizationService::new();

    let latest_status: Arc<RwLock<HashMap<u8, ElectrolyzerStatus>>> =
        Arc::new(RwLock::new(HashMap::new()));
    let latest_sensors: Arc<RwLock<HashMap<u8, Vec<SensorData>>>> =
        Arc::new(RwLock::new(HashMap::new()));
    let latest_alerts: Arc<RwLock<Vec<Alert>>> = Arc::new(RwLock::new(Vec::new()));
    let latest_optimizations: Arc<RwLock<Vec<OptimizationSuggestion>>> =
        Arc::new(RwLock::new(Vec::new()));

    let app_state = AppState {
        db: db.clone(),
        alert_manager: alert_manager.clone(),
        optimization_service: optimization_service.clone(),
        latest_status: latest_status.clone(),
        latest_sensors: latest_sensors.clone(),
        latest_alerts: latest_alerts.clone(),
        latest_optimizations: latest_optimizations.clone(),
        electrolyzer_count: config.electrolyzer_count,
    };

    let (profinet_receiver, mut data_rx) =
        ProfinetReceiver::new(config.profinet_port, db.clone());

    let profinet_handle = tokio::spawn(async move {
        if let Err(e) = profinet_receiver.run().await {
            error!("Profinet receiver error: {}", e);
        }
    });

    let api_state = app_state.clone();
    let api_port = config.api_port;
    let api_handle = tokio::spawn(async move {
        start_api_server(api_state, api_port).await;
    });

    let processing_handle = tokio::spawn(async move {
        let mut last_summary_time = Utc::now();
        let summary_interval = Duration::from_secs(60);

        loop {
            tokio::select! {
                Some(batch) = data_rx.recv() => {
                    process_batch(
                        &batch,
                        &db,
                        &alert_manager,
                        &optimization_service,
                        &latest_status,
                        &latest_sensors,
                        &latest_alerts,
                        &latest_optimizations,
                    ).await;
                }
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    let now = Utc::now();
                    if (now - last_summary_time).to_std().unwrap_or_default() >= summary_interval {
                        if let Err(e) = generate_system_summary(
                            &db,
                            &latest_status,
                            config.electrolyzer_count,
                        ).await {
                            warn!("Failed to generate system summary: {}", e);
                        }
                        last_summary_time = now;
                    }
                }
            }
        }
    });

    tokio::try_join!(profinet_handle, api_handle, processing_handle)?;

    Ok(())
}

async fn process_batch(
    batch: &SensorDataBatch,
    db: &Database,
    alert_manager: &AlertManager,
    optimization_service: &OptimizationService,
    latest_status: &Arc<RwLock<HashMap<u8, ElectrolyzerStatus>>>,
    latest_sensors: &Arc<RwLock<HashMap<u8, Vec<SensorData>>>>,
    latest_alerts: &Arc<RwLock<Vec<Alert>>>,
    latest_optimizations: &Arc<RwLock<Vec<OptimizationSuggestion>>>,
) {
    let electrolyzer_id = batch.electrolyzer_id;
    let timestamp = batch.timestamp;

    let max_cell_voltage = batch.cell_voltages.iter().cloned().fold(f64::NAN, f64::max);

    let alerts = alert_manager
        .process_data(
            electrolyzer_id,
            max_cell_voltage,
            batch.avg_voltage,
            batch.avg_hydrogen_purity,
            batch.avg_membrane_conductivity,
            timestamp,
        )
        .await;

    if !alerts.is_empty() {
        let mut la = latest_alerts.write();
        for alert in &alerts {
            la.push(alert.clone());
        }
        if la.len() > 1000 {
            la.drain(0..la.len() - 1000);
        }
    }

    let efficiency = optimization_service.calculate_current_efficiency(
        batch.avg_current_density,
        batch.avg_voltage,
        batch.avg_water_temp,
    );

    let hydrogen_production = optimization_service
        .model
        .calculate_hydrogen_production_rate(batch.avg_current_density, ACTIVE_AREA);

    let power_consumption = optimization_service
        .model
        .calculate_power_consumption(
            batch.avg_current_density,
            batch.avg_voltage,
            ACTIVE_AREA,
        );

    let electrolyzer_status = ElectrolyzerStatus {
        timestamp,
        electrolyzer_id,
        total_hydrogen_production: hydrogen_production * 2.0 / 3600.0,
        average_efficiency: efficiency,
        total_power_consumption: power_consumption * 2.0 / 3600.0,
        cell_voltage: batch.cell_voltages.clone(),
        current_density: batch.avg_current_density,
        water_temp: batch.avg_water_temp,
        hydrogen_purity: batch.avg_hydrogen_purity,
        membrane_conductivity: batch.avg_membrane_conductivity,
    };

    {
        let mut ls = latest_status.write();
        ls.insert(electrolyzer_id, electrolyzer_status.clone());
    }

    {
        let mut lse = latest_sensors.write();
        lse.insert(electrolyzer_id, batch.sensors.clone());
    }

    let db_clone = db.clone();
    let status_clone = electrolyzer_status.clone();
    tokio::spawn(async move {
        if let Err(e) = db_clone.insert_electrolyzer_status(&status_clone).await {
            error!("Failed to insert electrolyzer status: {}", e);
        }
    });

    let efficiency_history = EfficiencyHistory {
        timestamp,
        electrolyzer_id,
        current_density: batch.avg_current_density,
        cell_voltage: batch.avg_voltage,
        efficiency,
        water_temp: batch.avg_water_temp,
    };

    let db_clone = db.clone();
    tokio::spawn(async move {
        if let Err(e) = db_clone.insert_efficiency_history(&efficiency_history).await {
            error!("Failed to insert efficiency history: {}", e);
        }
    });

    if let Some(suggestion) = optimization_service.check_and_optimize(
        electrolyzer_id,
        batch.avg_current_density,
        batch.avg_voltage,
        batch.avg_water_temp,
    ) {
        {
            let mut lo = latest_optimizations.write();
            lo.push(suggestion.clone());
            if lo.len() > 100 {
                lo.drain(0..lo.len() - 100);
            }
        }

        let db_clone = db.clone();
        tokio::spawn(async move {
            if let Err(e) = db_clone.insert_optimization_suggestion(&suggestion).await {
                error!("Failed to insert optimization suggestion: {}", e);
            }
        });
    }
}

async fn generate_system_summary(
    db: &Database,
    latest_status: &Arc<RwLock<HashMap<u8, ElectrolyzerStatus>>>,
    electrolyzer_count: u8,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let status = latest_status.read();

    let mut total_hydrogen = 0.0;
    let mut total_efficiency = 0.0;
    let mut total_power = 0.0;
    let mut active_count = 0u8;

    for id in 1..=electrolyzer_count {
        if let Some(s) = status.get(&id) {
            total_hydrogen += s.total_hydrogen_production;
            total_efficiency += s.average_efficiency;
            total_power += s.total_power_consumption;
            active_count += 1;
        }
    }

    let avg_efficiency = if active_count > 0 {
        total_efficiency / active_count as f64
    } else {
        0.0
    };

    let summary = SystemSummary {
        timestamp: Utc::now(),
        total_hydrogen,
        avg_efficiency,
        total_power,
        active_electrolyzers: active_count,
    };

    db.insert_system_summary(&summary).await?;

    info!(
        "System summary: H2={:.2} m³, Efficiency={:.2}%, Power={:.2} kWh, Active={}/{}",
        total_hydrogen, avg_efficiency, total_power, active_count, electrolyzer_count
    );

    Ok(())
}
