use crate::alerts::AlertManager;
use crate::db::Database;
use crate::models::*;
use crate::optimization::OptimizationService;
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use chrono::{Duration, Utc};
use log::info;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub alert_manager: AlertManager,
    pub optimization_service: OptimizationService,
    pub latest_status: Arc<RwLock<HashMap<u8, ElectrolyzerStatus>>>,
    pub latest_sensors: Arc<RwLock<HashMap<u8, Vec<SensorData>>>>,
    pub latest_alerts: Arc<RwLock<Vec<Alert>>>,
    pub latest_optimizations: Arc<RwLock<Vec<OptimizationSuggestion>>>,
    pub electrolyzer_count: u8,
}

#[derive(Debug, Deserialize)]
pub struct TimeRangeQuery {
    pub hours: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct AlertActionQuery {
    pub alert_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct OptimizationActionQuery {
    pub suggestion_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: &str) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.to_string()),
        }
    }
}

async fn get_system_summary(
    state: axum::extract::State<AppState>,
) -> impl IntoResponse {
    let latest_status = state.latest_status.read();
    let mut total_hydrogen = 0.0;
    let mut total_efficiency = 0.0;
    let mut total_power = 0.0;
    let mut active_count = 0u8;

    for id in 1..=state.electrolyzer_count {
        if let Some(status) = latest_status.get(&id) {
            total_hydrogen += status.total_hydrogen_production;
            total_efficiency += status.average_efficiency;
            total_power += status.total_power_consumption;
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

    Json(ApiResponse::success(summary))
}

async fn get_electrolyzer_list(
    state: axum::extract::State<AppState>,
) -> impl IntoResponse {
    let latest_status = state.latest_status.read();
    let latest_sensors = state.latest_sensors.read();

    let mut electrolyzers: Vec<ElectrolyzerDetail> = Vec::new();

    for id in 1..=state.electrolyzer_count {
        let status = latest_status.get(&id);
        let sensors = latest_sensors.get(&id);

        let sensor_details = if let Some(sensors) = sensors {
            sensors
                .iter()
                .map(|s| {
                    let deviation_percent = if s.rated_value > 0.0 {
                        ((s.value - s.rated_value) / s.rated_value) * 100.0
                    } else {
                        0.0
                    };

                    SensorDetail {
                        sensor_id: s.sensor_id,
                        sensor_type: format!("{:?}", s.sensor_type).to_lowercase(),
                        location: format!("{:?}", s.location).to_lowercase(),
                        current_value: s.value,
                        rated_value: s.rated_value,
                        deviation_percent,
                        x: s.x,
                        y: s.y,
                        trend_data: Vec::new(),
                        efficiency_data: Vec::new(),
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        let (current_density, water_temp, efficiency, hydrogen_purity, membrane_conductivity) =
            if let Some(status) = status {
                (
                    status.current_density,
                    status.water_temp,
                    status.average_efficiency,
                    status.hydrogen_purity,
                    status.membrane_conductivity,
                )
            } else {
                (0.0, 0.0, 0.0, 0.0, 0.0)
            };

        let alerts = state
            .alert_manager
            .get_alert_state(id)
            .map(|_| Vec::new())
            .unwrap_or_default();

        let status_str = if efficiency < 75.0 {
            "warning".to_string()
        } else if efficiency >= 78.0 {
            "optimal".to_string()
        } else {
            "normal".to_string()
        };

        electrolyzers.push(ElectrolyzerDetail {
            id,
            status: status_str,
            current_density,
            water_temp,
            efficiency,
            hydrogen_purity,
            membrane_conductivity,
            sensors: sensor_details,
            alerts,
        });
    }

    Json(ApiResponse::success(electrolyzers))
}

async fn get_electrolyzer_detail(
    Path(id): Path<u8>,
    state: axum::extract::State<AppState>,
    Query(params): Query<TimeRangeQuery>,
) -> impl IntoResponse {
    if id == 0 || id > state.electrolyzer_count {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<ElectrolyzerDetail>::error(
                "Invalid electrolyzer ID",
            )),
        );
    }

    let hours = params.hours.unwrap_or(2);
    let latest_status = state.latest_status.read();
    let latest_sensors = state.latest_sensors.read();

    let status = latest_status.get(&id);
    let sensors = latest_sensors.get(&id);

    let mut sensor_details = Vec::new();
    if let Some(sensors) = sensors {
        for s in sensors {
            let deviation_percent = if s.rated_value > 0.0 {
                ((s.value - s.rated_value) / s.rated_value) * 100.0
            } else {
                0.0
            };

            let sensor_type_str = format!("{:?}", s.sensor_type).to_lowercase();
            let trend_data = match state
                .db
                .get_sensor_trend(id, &sensor_type_str, hours)
                .await
            {
                Ok(data) => data,
                Err(_) => Vec::new(),
            };

            let efficiency_data = match state.db.get_efficiency_history_range(id, hours).await {
                Ok(history) => history
                    .iter()
                    .map(|h| SensorTrendData {
                        timestamp: h.timestamp,
                        value: h.efficiency,
                    })
                    .collect(),
                Err(_) => Vec::new(),
            };

            sensor_details.push(SensorDetail {
                sensor_id: s.sensor_id,
                sensor_type: sensor_type_str,
                location: format!("{:?}", s.location).to_lowercase(),
                current_value: s.value,
                rated_value: s.rated_value,
                deviation_percent,
                x: s.x,
                y: s.y,
                trend_data,
                efficiency_data,
            });
        }
    }

    let alerts = match state.db.get_alerts_by_electrolyzer(id, 24).await {
        Ok(a) => a,
        Err(_) => Vec::new(),
    };

    let (current_density, water_temp, efficiency, hydrogen_purity, membrane_conductivity) =
        if let Some(status) = status {
            (
                status.current_density,
                status.water_temp,
                status.average_efficiency,
                status.hydrogen_purity,
                status.membrane_conductivity,
            )
        } else {
            (0.0, 0.0, 0.0, 0.0, 0.0)
        };

    let status_str = if efficiency < 75.0 {
        "warning".to_string()
    } else if efficiency >= 78.0 {
        "optimal".to_string()
    } else {
        "normal".to_string()
    };

    let detail = ElectrolyzerDetail {
        id,
        status: status_str,
        current_density,
        water_temp,
        efficiency,
        hydrogen_purity,
        membrane_conductivity,
        sensors: sensor_details,
        alerts,
    };

    (StatusCode::OK, Json(ApiResponse::success(detail)))
}

async fn get_sensor_detail(
    Path((electrolyzer_id, sensor_id)): Path<(u8, u16)>,
    state: axum::extract::State<AppState>,
    Query(params): Query<TimeRangeQuery>,
) -> impl IntoResponse {
    let hours = params.hours.unwrap_or(2);
    let end = Utc::now();
    let start = end - Duration::hours(hours);

    let sensor_data = match state
        .db
        .get_sensor_data_range(electrolyzer_id, sensor_id, start, end)
        .await
    {
        Ok(data) => data,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<SensorDetail>::error(&e.to_string())),
            )
        }
    };

    if sensor_data.is_empty() {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<SensorDetail>::error("No data found for sensor")),
        );
    }

    let latest = &sensor_data[sensor_data.len() - 1];
    let deviation_percent = if latest.rated_value > 0.0 {
        ((latest.value - latest.rated_value) / latest.rated_value) * 100.0
    } else {
        0.0
    };

    let trend_data: Vec<SensorTrendData> = sensor_data
        .iter()
        .map(|s| SensorTrendData {
            timestamp: s.timestamp,
            value: s.value,
        })
        .collect();

    let efficiency_data = match state
        .db
        .get_efficiency_history_range(electrolyzer_id, hours)
        .await
    {
        Ok(history) => history
            .iter()
            .map(|h| SensorTrendData {
                timestamp: h.timestamp,
                value: h.efficiency,
            })
            .collect(),
        Err(_) => Vec::new(),
    };

    let detail = SensorDetail {
        sensor_id: latest.sensor_id,
        sensor_type: format!("{:?}", latest.sensor_type).to_lowercase(),
        location: format!("{:?}", latest.location).to_lowercase(),
        current_value: latest.value,
        rated_value: latest.rated_value,
        deviation_percent,
        x: latest.x,
        y: latest.y,
        trend_data,
        efficiency_data,
    };

    (StatusCode::OK, Json(ApiResponse::success(detail)))
}

async fn get_active_alerts(
    state: axum::extract::State<AppState>,
) -> impl IntoResponse {
    let latest_alerts = state.latest_alerts.read();
    let alerts: Vec<Alert> = latest_alerts.iter().filter(|a| !a.resolved).cloned().collect();
    Json(ApiResponse::success(alerts))
}

async fn get_optimization_suggestions(
    state: axum::extract::State<AppState>,
) -> impl IntoResponse {
    let latest_optimizations = state.latest_optimizations.read();
    Json(ApiResponse::success(latest_optimizations.clone()))
}

async fn acknowledge_alert(
    Query(params): Query<AlertActionQuery>,
    state: axum::extract::State<AppState>,
) -> impl IntoResponse {
    match state.alert_manager.acknowledge_alert(params.alert_id).await {
        Ok(_) => Json(ApiResponse::success(true)),
        Err(e) => (Json(ApiResponse::<bool>::error(&e))),
    }
}

async fn resolve_alert(
    Query(params): Query<AlertActionQuery>,
    state: axum::extract::State<AppState>,
) -> impl IntoResponse {
    match state.alert_manager.resolve_alert(params.alert_id).await {
        Ok(_) => Json(ApiResponse::success(true)),
        Err(e) => (Json(ApiResponse::<bool>::error(&e))),
    }
}

async fn apply_optimization_suggestion(
    Query(params): Query<OptimizationActionQuery>,
    state: axum::extract::State<AppState>,
) -> impl IntoResponse {
    match state.db.apply_optimization_suggestion(params.suggestion_id).await {
        Ok(_) => Json(ApiResponse::success(true)),
        Err(e) => (Json(ApiResponse::<bool>::error(&e.to_string()))),
    }
}

async fn get_efficiency_curves(
    Path(electrolyzer_id): Path<u8>,
    state: axum::extract::State<AppState>,
) -> impl IntoResponse {
    let latest_status = state.latest_status.read();
    let current_temp = latest_status
        .get(&electrolyzer_id)
        .map(|s| s.water_temp)
        .unwrap_or(60.0);

    let efficiency_curve = state
        .optimization_service
        .get_efficiency_curve(0.5..4.0, 50, current_temp);

    let polarization_curve = state
        .optimization_service
        .get_polarization_curve(0.5..4.0, 50, current_temp);

    Json(ApiResponse::success(serde_json::json!({
        "efficiency_curve": efficiency_curve,
        "polarization_curve": polarization_curve,
        "current_temperature": current_temp
    })))
}

async fn health_check() -> impl IntoResponse {
    Json(ApiResponse::success(serde_json::json!({
        "status": "healthy",
        "timestamp": Utc::now().to_rfc3339()
    })))
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/system/summary", get(get_system_summary))
        .route("/api/electrolyzers", get(get_electrolyzer_list))
        .route("/api/electrolyzers/:id", get(get_electrolyzer_detail))
        .route(
            "/api/electrolyzers/:id/sensors/:sensor_id",
            get(get_sensor_detail),
        )
        .route(
            "/api/electrolyzers/:id/curves",
            get(get_efficiency_curves),
        )
        .route("/api/alerts/active", get(get_active_alerts))
        .route("/api/alerts/acknowledge", put(acknowledge_alert))
        .route("/api/alerts/resolve", put(resolve_alert))
        .route(
            "/api/optimizations",
            get(get_optimization_suggestions),
        )
        .route(
            "/api/optimizations/apply",
            put(apply_optimization_suggestion),
        )
        .with_state(state)
}

pub async fn start_api_server(state: AppState, port: u16) {
    let router = create_router(state);
    let addr = format!("0.0.0.0:{}", port);
    info!("API server starting on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, router).await.unwrap();
}
