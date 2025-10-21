use axum::{
    Router,
    routing::{get, post, put, delete},
    extract::{State, Path},
    response::Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use super::ApiState;

pub fn create_routes() -> Router<ApiState> {
    Router::new()
        .route("/", get(list_containers))
        .route("/", post(create_container))
        .route("/:container_id", get(get_container))
        .route("/:container_id", put(update_container))
        .route("/:container_id", delete(delete_container))
        .route("/:container_id/start", post(start_container))
        .route("/:container_id/stop", post(stop_container))
        .route("/:container_id/restart", post(restart_container))
}

async fn list_containers(State(state): State<ApiState>) -> Json<Vec<serde_json::Value>> {
    Json(vec![])
}

async fn create_container(
    State(state): State<ApiState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    Err(StatusCode::NOT_IMPLEMENTED)
}

async fn get_container(
    State(state): State<ApiState>,
    Path(container_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    Err(StatusCode::NOT_FOUND)
}

async fn update_container(
    State(state): State<ApiState>,
    Path(container_id): Path<String>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    Err(StatusCode::NOT_IMPLEMENTED)
}

async fn delete_container(
    State(state): State<ApiState>,
    Path(container_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    Err(StatusCode::NOT_IMPLEMENTED)
}

async fn start_container(
    State(state): State<ApiState>,
    Path(container_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    Err(StatusCode::NOT_IMPLEMENTED)
}

async fn stop_container(
    State(state): State<ApiState>,
    Path(container_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    Err(StatusCode::NOT_IMPLEMENTED)
}

async fn restart_container(
    State(state): State<ApiState>,
    Path(container_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    Err(StatusCode::NOT_IMPLEMENTED)
}