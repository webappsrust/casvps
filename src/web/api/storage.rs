use axum::{
    Router,
    routing::{get, post, put, delete},
    extract::{State, Path},
    response::Json,
    http::StatusCode,
};
use super::ApiState;

pub fn create_routes() -> Router<ApiState> {
    Router::new()
        .route("/pools", get(list_storage_pools))
        .route("/pools", post(create_storage_pool))
        .route("/pools/:pool_id", get(get_storage_pool))
        .route("/pools/:pool_id", delete(delete_storage_pool))
        .route("/volumes", get(list_volumes))
        .route("/volumes", post(create_volume))
        .route("/volumes/:volume_id", get(get_volume))
        .route("/volumes/:volume_id", delete(delete_volume))
        .route("/backups", get(list_backups))
        .route("/backups", post(create_backup))
}

async fn list_storage_pools(State(state): State<ApiState>) -> Json<Vec<serde_json::Value>> {
    Json(vec![])
}

async fn create_storage_pool(
    State(state): State<ApiState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    Err(StatusCode::NOT_IMPLEMENTED)
}

async fn get_storage_pool(
    State(state): State<ApiState>,
    Path(pool_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    Err(StatusCode::NOT_FOUND)
}

async fn delete_storage_pool(
    State(state): State<ApiState>,
    Path(pool_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    Err(StatusCode::NOT_IMPLEMENTED)
}

async fn list_volumes(State(state): State<ApiState>) -> Json<Vec<serde_json::Value>> {
    Json(vec![])
}

async fn create_volume(
    State(state): State<ApiState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    Err(StatusCode::NOT_IMPLEMENTED)
}

async fn get_volume(
    State(state): State<ApiState>,
    Path(volume_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    Err(StatusCode::NOT_FOUND)
}

async fn delete_volume(
    State(state): State<ApiState>,
    Path(volume_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    Err(StatusCode::NOT_IMPLEMENTED)
}

async fn list_backups(State(state): State<ApiState>) -> Json<Vec<serde_json::Value>> {
    Json(vec![])
}

async fn create_backup(
    State(state): State<ApiState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    Err(StatusCode::NOT_IMPLEMENTED)
}