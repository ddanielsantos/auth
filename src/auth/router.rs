use crate::error::AppError;
use crate::jwt;
use crate::router::AppState;
use crate::{crypto, id};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct LoginRequestBody {
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct RegisterRequestBody {
    identifier: String,
    method_type: String,
    password: String,
    client_id: String,
    profile: serde_json::Value,
}

pub fn get_router() -> Router<AppState> {
    Router::new()
        .route("/me", get(me_handler))
        .route("/login", post(login_handler))
        .route("/register", post(register_handler))
}

async fn register_handler(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequestBody>,
) -> Result<impl IntoResponse, AppError> {
    let client_id = id::parse_uuid(&body.client_id)?;
    let identity_id = id::new_uuid();
    let mut tx = state.pool.begin().await?;

    sqlx::query!("INSERT INTO identities (id) VALUES ($1)", identity_id)
        .execute(&mut *tx)
        .await?;

    let hash = crypto::hash_password(&body.password)?;
    sqlx::query!(
        "INSERT INTO login_methods (identity_id, method_type, identifier, password_hash) VALUES ($1, $2, $3, $4)",
        identity_id,
        body.method_type,
        body.identifier,
        hash
    )
    .execute(&mut *tx)
    .await?;

    let project_id = sqlx::query_scalar!("SELECT project_id FROM applications WHERE client_id = $1", client_id)
        .fetch_one(&mut *tx)
        .await?;

    sqlx::query!(
        "INSERT INTO user_accounts (identity_id, project_id, local_profile_data) VALUES ($1, $2, $3)",
        identity_id,
        project_id,
        body.profile
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(StatusCode::CREATED)
}

#[derive(Debug, Serialize)]
struct MeResponse {
    identity_id: String,
    account_id: String,
    identifier: String,
    profile: Option<serde_json::Value>,
}

async fn me_handler(header: HeaderMap, State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let jwt = jwt::get_jwt_token(&header)?;
    let sub = jwt::decode_admin_token(jwt)?.claims.sub;

    let user_id = id::parse_uuid(&sub)?;
    let user_data = sqlx::query!(
        r#"
        SELECT
            ua.identity_id,
            ua.id as account_id,
            ua.local_profile_data,
            lm.identifier
        FROM user_accounts ua
        JOIN login_methods lm ON lm.identity_id = ua.identity_id
        WHERE ua.id = $1 AND lm.is_verified = true
        LIMIT 1
        "#,
        user_id
    )
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(MeResponse {
        identity_id: user_data.identity_id.to_string(),
        account_id: user_data.account_id.to_string(),
        identifier: user_data.identifier,
        profile: user_data.local_profile_data,
    }))
}

async fn login_handler(Json(_body): Json<LoginRequestBody>) -> impl IntoResponse {}
