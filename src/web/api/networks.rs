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
        .route("/", get(list_networks))
        .route("/", post(create_network))
        .route("/:network_id", get(get_network))
        .route("/:network_id", put(update_network))
        .route("/:network_id", delete(delete_network))
}

async fn list_networks(State(state): State<ApiState>) -> Json<Vec<serde_json::Value>> {
    Json(vec![])
}

async fn create_network(
    State(state): State<ApiState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    Err(StatusCode::NOT_IMPLEMENTED)
}

async fn get_network(
    State(state): State<ApiState>,
    Path(network_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    Err(StatusCode::NOT_FOUND)
}

async fn update_network(
    State(state): State<ApiState>,
    Path(network_id): Path<String>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    Err(StatusCode::NOT_IMPLEMENTED)
}

async fn delete_network(
    State(state): State<ApiState>,
    Path(network_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    Err(StatusCode::NOT_IMPLEMENTED)
}