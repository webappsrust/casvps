use axum::{
    Router,
    routing::{get, post},
    extract::{State, Path, Query},
    response::Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use super::ApiState;

pub fn create_routes() -> Router<ApiState> {
    Router::new()
        .route("/", get(get_metrics))
        .route("/prometheus", get(prometheus_metrics))
        .route("/status", get(get_system_status))
        .route("/nodes/:node_id/metrics", get(get_node_metrics))
        .route("/alerts", get(list_alerts))
        .route("/alerts", post(create_alert))
}

#[derive(Deserialize)]
struct MetricsQuery {
    start: Option<i64>,
    end: Option<i64>,
    step: Option<i64>,
}

async fn get_metrics(
    State(state): State<ApiState>,
    Query(query): Query<MetricsQuery>,
) -> Json<serde_json::Value> {
    // TODO: Return Victoria Metrics formatted data
    Json(serde_json::json!({
        "status": "success",
        "data": {
            "resultType": "matrix",
            "result": []
        }
    }))
}

async fn prometheus_metrics(State(state): State<ApiState>) -> String {
    // TODO: Return Prometheus formatted metrics
    r#"# HELP casvps_build_info Build information
# TYPE casvps_build_info gauge
casvps_build_info{version="1.0.0"} 1
# HELP casvps_up Whether CasVPS is up
# TYPE casvps_up gauge
casvps_up 1
"#.to_string()
}

#[derive(Serialize)]
struct SystemStatus {
    cpu_usage: f32,
    memory_usage: f32,
    disk_usage: f32,
    network_rx: u64,
    network_tx: u64,
    vm_count: u32,
    container_count: u32,
    uptime: u64,
}

async fn get_system_status(State(state): State<ApiState>) -> Json<SystemStatus> {
    let sys = sysinfo::System::new_all();

    Json(SystemStatus {
        cpu_usage: sys.global_cpu_info().cpu_usage(),
        memory_usage: (sys.used_memory() as f32 / sys.total_memory() as f32) * 100.0,
        disk_usage: 0.0, // TODO: Calculate disk usage
        network_rx: 0,   // TODO: Get network stats
        network_tx: 0,   // TODO: Get network stats
        vm_count: 0,     // TODO: Count VMs
        container_count: 0, // TODO: Count containers
        uptime: 0,       // TODO: Calculate uptime
    })
}

async fn get_node_metrics(
    State(state): State<ApiState>,
    Path(node_id): Path<String>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "node_id": node_id,
        "metrics": {}
    }))
}

async fn list_alerts(State(state): State<ApiState>) -> Json<Vec<serde_json::Value>> {
    Json(vec![])
}

async fn create_alert(
    State(state): State<ApiState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    Err(StatusCode::NOT_IMPLEMENTED)
}