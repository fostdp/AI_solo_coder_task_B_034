use pem_electrolyzer::*;

use chrono::Utc;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};
use tracing_subscriber::{fmt, EnvFilter};
use pem_electrolyzer::metrics as pem_metrics;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,pem_electrolyzer=debug")),
        )
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    pem_metrics::init_metrics();

    info!("{}", "=".repeat(60));
    info!("PEM Electrolyzer Monitoring and Efficiency Optimization Platform");
    info!("{}", "=".repeat(60));

    let config = AppConfig::load();
    info!("Configuration loaded:");
    info!("  Profinet port: {}", config.server.profinet_port);
    info!("  API port: {}", config.server.api_port);
    info!("  ClickHouse: {}", config.database.clickhouse_url);
    info!("  Database: {}", config.database.clickhouse_database);
    info!("  Electrolyzers: {}", config.system.electrolyzer_count);
    info!("  Sensors per electrolyzer: {}", config.system.sensors_per_electrolyzer);
    info!("  Optimization threshold: {}%", config.optimization.efficiency_threshold);
    info!("  Target efficiency: {}%", config.optimization.target_efficiency);
    info!("  Max concurrent optimizations: {}", config.optimization.max_concurrent_optimizations);
    info!("{}", "=".repeat(60));

    let db = Database::new(
        &config.database.clickhouse_url,
        &config.database.clickhouse_user,
        &config.database.clickhouse_password,
        &config.database.clickhouse_database,
    );

    let latest_status: Arc<RwLock<HashMap<u8, ElectrolyzerStatus>>> =
        Arc::new(RwLock::new(HashMap::new()));
    let latest_sensors: Arc<RwLock<HashMap<u8, Vec<SensorData>>>> =
        Arc::new(RwLock::new(HashMap::new()));
    let latest_alerts: Arc<RwLock<Vec<Alert>>> = Arc::new(RwLock::new(Vec::new()));
    let latest_optimizations: Arc<RwLock<Vec<OptimizationSuggestion>>> =
        Arc::new(RwLock::new(Vec::new()));

    let (profinet_driver, mut data_rx) = ProfinetDriver::new(config.profinet.clone(), db.clone());

    let (efficiency_analyzer, mut efficiency_rx) = EfficiencyAnalyzer::new(
        &config.efficiency_model,
        config.optimization.clone(),
        config.system.clone(),
    );

    let efficiency_model = Arc::new(EfficiencyModel::from_config(&config.efficiency_model));

    let (optimization_engine, mut optimization_handle) = OptimizationEngine::new(
        efficiency_model.clone(),
        config.genetic_algorithm.clone(),
        config.optimization.clone(),
    );

    let (mut alarm_bridge, mut alert_rx) = AlarmBridge::new(
        db.clone(),
        config.alerts.clone(),
        Some(config.opcua.clone()),
    );

    if let Err(e) = alarm_bridge.connect_opcua().await {
        warn!("Failed to connect to OPC UA server on startup: {}", e);
    }

    let alarm_bridge_arc = Arc::new(alarm_bridge);

    let app_state = api::AppState {
        db: db.clone(),
        alarm_bridge: alarm_bridge_arc.clone(),
        efficiency_analyzer: efficiency_analyzer.clone(),
        latest_status: latest_status.clone(),
        latest_sensors: latest_sensors.clone(),
        latest_alerts: latest_alerts.clone(),
        latest_optimizations: latest_optimizations.clone(),
        electrolyzer_count: config.system.electrolyzer_count,
    };

    let profinet_port = config.server.profinet_port;
    let profinet_handle = tokio::spawn(async move {
        if let Err(e) = profinet_driver.run(profinet_port).await {
            error!("Profinet driver error: {}", e);
        }
    });

    let api_state = app_state.clone();
    let api_port = config.server.api_port;
    let api_handle = tokio::spawn(async move {
        api::start_api_server(api_state, api_port).await;
    });

    let optimization_engine_handle = tokio::spawn(async move {
        optimization_engine.run().await;
    });

    let electrolyzer_count = config.system.electrolyzer_count;

    let main_loop_handle = tokio::spawn(async move {
        let mut last_summary_time = Utc::now();
        let summary_interval = Duration::from_secs(60);

        loop {
            tokio::select! {
                Some(batch) = data_rx.recv() => {
                    let _timer = pem_metrics::MetricsTimer::new("main_loop_process_batch_duration_seconds");
                    let electrolyzer_id = batch.electrolyzer_id;

                    pem_metrics::increment_profinet_packets_received();
                    pem_metrics::increment_sensor_data_points();
                    pem_metrics::set_cell_voltage(electrolyzer_id, batch.avg_voltage);
                    pem_metrics::set_water_temp(electrolyzer_id, batch.avg_water_temp);
                    pem_metrics::set_current_density(electrolyzer_id, batch.avg_current_density);
                    pem_metrics::set_hydrogen_purity(electrolyzer_id, batch.avg_hydrogen_purity);
                    pem_metrics::set_membrane_conductivity(electrolyzer_id, batch.avg_membrane_conductivity);

                    if let Err(e) = efficiency_analyzer.analyze_batch(&batch).await {
                        error!("Failed to analyze batch for electrolyzer {}: {}", electrolyzer_id, e);
                    }

                    let max_cell_voltage = batch.cell_voltages.iter().cloned().fold(f64::NAN, f64::max);

                    if let Err(e) = alarm_bridge_arc.process_sensor_data(
                        electrolyzer_id,
                        max_cell_voltage,
                        batch.avg_voltage,
                        batch.avg_hydrogen_purity,
                        batch.avg_membrane_conductivity,
                        batch.timestamp,
                    ).await {
                        error!("Failed to process sensor data for alerts: {}", e);
                    }

                    {
                        let mut lse = latest_sensors.write();
                        lse.insert(electrolyzer_id, batch.sensors.clone());
                    }
                }

                Some(result) = efficiency_rx.recv() => {
                    let _timer = pem_metrics::MetricsTimer::new("main_loop_process_efficiency_duration_seconds");
                    let electrolyzer_id = result.electrolyzer_id;

                    pem_metrics::set_efficiency(electrolyzer_id, result.efficiency);
                    pem_metrics::set_hydrogen_production(electrolyzer_id, result.hydrogen_production);
                    pem_metrics::set_power_consumption(electrolyzer_id, result.power_consumption);

                    let hydrogen_production = result.hydrogen_production * 2.0 / 3600.0;
                    let power_consumption = result.power_consumption * 2.0 / 3600.0;

                    let electrolyzer_status = ElectrolyzerStatus {
                        timestamp: result.timestamp,
                        electrolyzer_id,
                        total_hydrogen_production: hydrogen_production,
                        average_efficiency: result.efficiency,
                        total_power_consumption: power_consumption,
                        cell_voltage: Vec::new(),
                        current_density: result.current_density,
                        water_temp: result.water_temp,
                        hydrogen_purity: 0.0,
                        membrane_conductivity: 0.0,
                    };

                    {
                        let mut ls = latest_status.write();
                        ls.insert(electrolyzer_id, electrolyzer_status.clone());
                    }

                    let db_clone = db.clone();
                    let status_clone = electrolyzer_status.clone();
                    tokio::spawn(async move {
                        if let Err(e) = db_clone.insert_electrolyzer_status(&status_clone).await {
                            error!("Failed to insert electrolyzer status: {}", e);
                        }
                    });

                    let efficiency_history = EfficiencyHistory {
                        timestamp: result.timestamp,
                        electrolyzer_id,
                        current_density: result.current_density,
                        cell_voltage: result.cell_voltage,
                        efficiency: result.efficiency,
                        water_temp: result.water_temp,
                    };

                    let db_clone = db.clone();
                    tokio::spawn(async move {
                        if let Err(e) = db_clone.insert_efficiency_history(&efficiency_history).await {
                            error!("Failed to insert efficiency history: {}", e);
                        }
                    });

                    if result.needs_optimization {
                        pem_metrics::increment_optimization_tasks_submitted();
                        let task = OptimizationTask {
                            electrolyzer_id,
                            current_density: result.current_density,
                            cell_voltage: result.cell_voltage,
                            water_temp: result.water_temp,
                            current_efficiency: result.efficiency,
                            timestamp: result.timestamp,
                        };

                        if let Err(e) = optimization_handle.submit_optimization(task).await {
                            warn!(
                                "Failed to submit optimization task for electrolyzer {}: {}",
                                electrolyzer_id, e
                            );
                        }
                    }
                }

                Some(alert) = alert_rx.recv() => {
                    let _timer = pem_metrics::MetricsTimer::new("main_loop_process_alert_duration_seconds");
                    let level_str = match alert.alert_level {
                        AlertLevel::Level1 => "level1",
                        AlertLevel::Level2 => "level2",
                        AlertLevel::Level3 => "level3",
                    };
                    pem_metrics::increment_alerts_generated(level_str);

                    {
                        let mut la = latest_alerts.write();
                        la.push(alert.clone());
                        if la.len() > 1000 {
                            la.drain(0..la.len() - 1000);
                        }
                    }

                    let alarm_bridge_clone = alarm_bridge_arc.clone();
                    tokio::spawn(async move {
                        if let Err(e) = alarm_bridge_clone.process_alert(&alert).await {
                            error!("Failed to process alert: {}", e);
                        }
                    });
                }

                Some(suggestion) = async {
                    loop {
                        match optimization_handle.poll_result() {
                            Some(s) => break Some(s),
                            None => {
                                tokio::time::sleep(Duration::from_millis(100)).await;
                            }
                        }
                    }
                } => {
                    pem_metrics::increment_optimization_tasks_completed(true);

                    {
                        let mut lo = latest_optimizations.write();
                        lo.push(suggestion.clone());
                        if lo.len() > 100 {
                            lo.drain(0..lo.len() - 100);
                        }
                    }

                    let db_clone = db.clone();
                    tokio::spawn(async move {
                        pem_metrics::increment_db_writes(true);
                        if let Err(e) = db_clone.insert_optimization_suggestion(&suggestion).await {
                            pem_metrics::increment_db_writes(false);
                            error!("Failed to insert optimization suggestion: {}", e);
                        }
                    });

                    info!(
                        "Optimization suggestion received for electrolyzer {}: expected efficiency {:.2}%",
                        suggestion.electrolyzer_id, suggestion.expected_efficiency
                    );
                }

                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    let now = Utc::now();
                    if (now - last_summary_time).to_std().unwrap_or_default() >= summary_interval {
                        pem_metrics::set_optimization_queue_depth(optimization_handle.queue_depth());
                        if let Err(e) = generate_system_summary(
                            &db,
                            &latest_status,
                            electrolyzer_count,
                        ).await {
                            warn!("Failed to generate system summary: {}", e);
                        }
                        last_summary_time = now;
                    }
                }
            }
        }
    });

    tokio::try_join!(
        profinet_handle,
        api_handle,
        main_loop_handle,
        optimization_engine_handle
    )?;

    Ok(())
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

    pem_metrics::set_active_electrolyzers(active_count);

    pem_metrics::increment_db_writes(true);
    if let Err(e) = db.insert_system_summary(&summary).await {
        pem_metrics::increment_db_writes(false);
        return Err(e.into());
    }

    info!(
        "System summary: H2={:.2} m³, Efficiency={:.2}%, Power={:.2} kWh, Active={}/{}",
        total_hydrogen, avg_efficiency, total_power, active_count, electrolyzer_count
    );

    Ok(())
}
