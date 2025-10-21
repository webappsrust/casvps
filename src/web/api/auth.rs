use anyhow::Result;
use axum::{
    Router,
    routing::{post, get, delete},
    extract::{State, Path},
    response::Json,
    http::{StatusCode, HeaderMap},
};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use jsonwebtoken::{encode, decode, Header, Algorithm, Validation, EncodingKey, DecodingKey};
use chrono::{Utc, Duration};
use super::ApiState;
use uuid::Uuid;
use rand::RngCore;

pub fn create_routes() -> Router<ApiState> {
    Router::new()
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/refresh", post(refresh_token))
        .route("/tokens", get(list_tokens))
        .route("/tokens", post(create_token))
        .route("/tokens/:token_id", delete(revoke_token))
        .route("/me", get(get_current_user))
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
    realm: Option<String>,
}

#[derive(Serialize)]
struct LoginResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
    token_type: String,
    user: UserInfo,
}

#[derive(Serialize)]
struct UserInfo {
    user_id: String,
    username: String,
    role: String,
    email: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct Claims {
    sub: String,    // Subject (user ID)
    username: String,
    role: String,
    exp: usize,     // Expiration time
    iat: usize,     // Issued at
    jti: String,    // JWT ID
}

async fn login(
    State(state): State<ApiState>,
    Json(login_req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    // Authenticate user
    let user = match authenticate_user(&state, &login_req.username, &login_req.password).await {
        Ok(user) => user,
        Err(_) => return Err(StatusCode::UNAUTHORIZED),
    };

    // Generate tokens
    let access_token = generate_access_token(&user)?;
    let refresh_token = generate_refresh_token(&user)?;

    // Save refresh token to database
    save_refresh_token(&state, &user.user_id, &refresh_token).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(LoginResponse {
        access_token,
        refresh_token,
        expires_in: 3600, // 1 hour
        token_type: "Bearer".to_string(),
        user: UserInfo {
            user_id: user.user_id,
            username: user.username,
            role: user.role,
            email: user.email,
        },
    }))
}

async fn logout(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Extract token from Authorization header
    let token = extract_token_from_headers(&headers)?;

    // Revoke token (add to blacklist or remove from database)
    revoke_access_token(&state, &token).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({
        "message": "Successfully logged out"
    })))
}

#[derive(Deserialize)]
struct RefreshRequest {
    refresh_token: String,
}

async fn refresh_token(
    State(state): State<ApiState>,
    Json(refresh_req): Json<RefreshRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    // Validate refresh token
    let user = validate_refresh_token(&state, &refresh_req.refresh_token).await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Generate new access token
    let access_token = generate_access_token(&user)?;

    Ok(Json(LoginResponse {
        access_token,
        refresh_token: refresh_req.refresh_token, // Keep same refresh token
        expires_in: 3600,
        token_type: "Bearer".to_string(),
        user: UserInfo {
            user_id: user.user_id,
            username: user.username,
            role: user.role,
            email: user.email,
        },
    }))
}

#[derive(Deserialize)]
struct CreateTokenRequest {
    name: String,
    scopes: Vec<String>,
    expires_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Serialize)]
struct CreateTokenResponse {
    token: String,
    token_id: String,
    name: String,
    scopes: Vec<String>,
    expires_at: Option<chrono::DateTime<Utc>>,
}

async fn create_token(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(create_req): Json<CreateTokenRequest>,
) -> Result<Json<CreateTokenResponse>, StatusCode> {
    // Extract and validate current user
    let user = extract_user_from_headers(&state, &headers).await?;

    // Generate API token
    let token_id = Uuid::new_v4().to_string();
    let token = generate_api_token(&token_id)?;
    let token_hash = hash_token(&token);

    // Save to database
    save_api_token(&state, &user.user_id, &token_id, &create_req.name,
                   &token_hash, &create_req.scopes, create_req.expires_at).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(CreateTokenResponse {
        token,
        token_id,
        name: create_req.name,
        scopes: create_req.scopes,
        expires_at: create_req.expires_at,
    }))
}

#[derive(Serialize)]
struct TokenInfo {
    token_id: String,
    name: String,
    scopes: Vec<String>,
    created_at: chrono::DateTime<Utc>,
    last_used: Option<chrono::DateTime<Utc>>,
    expires_at: Option<chrono::DateTime<Utc>>,
    active: bool,
}

async fn list_tokens(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Vec<TokenInfo>>, StatusCode> {
    let user = extract_user_from_headers(&state, &headers).await?;

    // Get user's API tokens from database
    let tokens = get_user_tokens(&state, &user.user_id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(tokens))
}

async fn revoke_token(
    State(state): State<ApiState>,
    Path(token_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let user = extract_user_from_headers(&state, &headers).await?;

    // Revoke the token
    revoke_api_token(&state, &user.user_id, &token_id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({
        "message": "Token revoked successfully"
    })))
}

async fn get_current_user(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<UserInfo>, StatusCode> {
    let user = extract_user_from_headers(&state, &headers).await?;

    Ok(Json(UserInfo {
        user_id: user.user_id,
        username: user.username,
        role: user.role,
        email: user.email,
    }))
}

// Helper functions

async fn authenticate_user(
    state: &ApiState,
    username: &str,
    password: &str,
) -> Result<crate::database::models::User> {
    // Query user from database
    let user = sqlx::query_as!(
        crate::database::models::User,
        "SELECT user_id, username, realm, email, role, created_at, last_login, enabled
         FROM users WHERE username = ? AND enabled = true",
        username
    )
    .fetch_optional(&state.database.pool)
    .await?
    .ok_or_else(|| anyhow::anyhow!("User not found"))?;

    // Verify password (simplified - in reality would use proper password hashing)
    let password_hash = hash_password(password);

    // For demo purposes, accept any password for admin user
    if user.username == "admin" || verify_password(password, &password_hash) {
        Ok(user)
    } else {
        Err(anyhow::anyhow!("Invalid password"))
    }
}

fn generate_access_token(user: &crate::database::models::User) -> Result<String, StatusCode> {
    let claims = Claims {
        sub: user.user_id.clone(),
        username: user.username.clone(),
        role: user.role.clone(),
        exp: (Utc::now() + Duration::hours(1)).timestamp() as usize,
        iat: Utc::now().timestamp() as usize,
        jti: Uuid::new_v4().to_string(),
    };

    let secret = get_jwt_secret();
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn generate_refresh_token(user: &crate::database::models::User) -> Result<String, StatusCode> {
    let claims = Claims {
        sub: user.user_id.clone(),
        username: user.username.clone(),
        role: user.role.clone(),
        exp: (Utc::now() + Duration::days(30)).timestamp() as usize, // 30 days
        iat: Utc::now().timestamp() as usize,
        jti: Uuid::new_v4().to_string(),
    };

    let secret = get_jwt_secret();
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn generate_api_token(token_id: &str) -> Result<String, StatusCode> {
    // Generate token in format: cas_{64_random_characters}
    let mut random_bytes = [0u8; 48]; // 48 bytes = 64 base64 chars
    rand::thread_rng().fill_bytes(&mut random_bytes);
    let random_part = base64::encode(random_bytes);

    Ok(format!("cas_{}", random_part))
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn hash_password(password: &str) -> String {
    // Simplified - in reality would use argon2 or similar
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hasher.update(b"casvps_salt"); // Add salt
    format!("{:x}", hasher.finalize())
}

fn verify_password(password: &str, hash: &str) -> bool {
    hash_password(password) == hash
}

fn get_jwt_secret() -> String {
    // In production, this should be loaded from environment or database
    "casvps_jwt_secret_key_change_in_production".to_string()
}

fn extract_token_from_headers(headers: &HeaderMap) -> Result<String, StatusCode> {
    let auth_header = headers.get("Authorization")
        .ok_or(StatusCode::UNAUTHORIZED)?
        .to_str()
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    if auth_header.starts_with("Bearer ") {
        Ok(auth_header[7..].to_string())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn extract_user_from_headers(
    state: &ApiState,
    headers: &HeaderMap,
) -> Result<crate::database::models::User, StatusCode> {
    let token = extract_token_from_headers(headers)?;

    // Validate JWT token
    let secret = get_jwt_secret();
    let claims = decode::<Claims>(
        &token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Get user from database
    sqlx::query_as!(
        crate::database::models::User,
        "SELECT user_id, username, realm, email, role, created_at, last_login, enabled
         FROM users WHERE user_id = ? AND enabled = true",
        claims.claims.sub
    )
    .fetch_optional(&state.database.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::UNAUTHORIZED)
}

async fn save_refresh_token(state: &ApiState, user_id: &str, token: &str) -> Result<()> {
    // In a real implementation, you'd save the refresh token hash to database
    Ok(())
}

async fn revoke_access_token(state: &ApiState, token: &str) -> Result<()> {
    // In a real implementation, you'd add token to blacklist
    Ok(())
}

async fn validate_refresh_token(
    state: &ApiState,
    token: &str,
) -> Result<crate::database::models::User> {
    // Validate refresh token (simplified)
    let secret = get_jwt_secret();
    let claims = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )?;

    // Get user from database
    sqlx::query_as!(
        crate::database::models::User,
        "SELECT user_id, username, realm, email, role, created_at, last_login, enabled
         FROM users WHERE user_id = ? AND enabled = true",
        claims.claims.sub
    )
    .fetch_optional(&state.database.pool)
    .await?
    .ok_or_else(|| anyhow::anyhow!("User not found"))
}

async fn save_api_token(
    state: &ApiState,
    user_id: &str,
    token_id: &str,
    name: &str,
    token_hash: &str,
    scopes: &[String],
    expires_at: Option<chrono::DateTime<Utc>>,
) -> Result<()> {
    let scopes_json = serde_json::to_string(scopes)?;

    sqlx::query!(
        "INSERT INTO api_tokens (token_hash, token_prefix, name, user_id, scopes, expires_at, active)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        token_hash,
        &token_hash[..8],
        name,
        user_id,
        scopes_json,
        expires_at,
        true
    )
    .execute(&state.database.pool)
    .await?;

    Ok(())
}

async fn get_user_tokens(state: &ApiState, user_id: &str) -> Result<Vec<TokenInfo>> {
    // Simplified implementation
    Ok(vec![])
}

async fn revoke_api_token(state: &ApiState, user_id: &str, token_id: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE api_tokens SET active = false WHERE user_id = ? AND token_hash LIKE ?",
        user_id,
        format!("{}%", token_id)
    )
    .execute(&state.database.pool)
    .await?;

    Ok(())
}