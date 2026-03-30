use crate::error::AppError;
use crate::jwt;
use crate::router::AppState;
use crate::{crypto, id};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequestBody {
    email: String,
    password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RegisterRequestBody {
    identifier: String,
    method_type: String,
    password: String,
    client_id: String,
    #[schema(value_type = Object)]
    profile: serde_json::Value,
}

pub fn get_router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(me_handler))
        .routes(routes!(login_handler))
        .routes(routes!(register_handler))
}

#[utoipa::path(
    post,
    path = "/register",
    tag = "auth",
    request_body = RegisterRequestBody,
    responses(
        (status = 201, description = "User registered successfully"),
        (status = 400, description = "Validation error or bad request"),
        (status = 409, description = "User already exists"),
    )
)]
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

#[derive(Debug, Serialize, ToSchema)]
pub struct MeResponse {
    identity_id: String,
    account_id: String,
    identifier: String,
    #[schema(value_type = Option<Object>)]
    profile: Option<serde_json::Value>,
}

#[utoipa::path(
    get,
    path = "/me",
    tag = "auth",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Current user info", body = MeResponse),
        (status = 401, description = "Unauthorized"),
    )
)]
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

#[utoipa::path(
    post,
    path = "/login",
    tag = "auth",
    request_body = LoginRequestBody,
    responses(
        (status = 200, description = "Login successful"),
    )
)]
async fn login_handler(Json(_body): Json<LoginRequestBody>) -> impl IntoResponse {}
