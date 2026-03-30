use crate::error::AppError;
use crate::router::AppState;
use crate::{crypto, id, jwt};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use tracing::error;
use utoipa::ToSchema;
use validator::Validate;

#[derive(Deserialize, Validate, ToSchema)]
pub struct RegisterAdminRequestBody {
    #[validate(length(min = 6, max = 50, message = "Should have from 6 to 50 characters"))]
    username: String,
    #[validate(length(min = 6, max = 50, message = "Should have from 6 to 50 characters"))]
    password: String,
}

#[derive(Serialize, ToSchema)]
pub struct RegisterAdminResponse {
    user_id: String,
    access_token: String,
}

#[utoipa::path(
    post,
    path = "/register",
    tag = "admin",
    request_body = RegisterAdminRequestBody,
    responses(
        (status = 201, description = "Admin registered successfully", body = RegisterAdminResponse),
        (status = 400, description = "Validation error"),
        (status = 409, description = "Admin already exists"),
    )
)]
pub async fn register_admin_handler(
    State(state): State<AppState>,
    Json(body): Json<RegisterAdminRequestBody>,
) -> Result<impl IntoResponse, AppError> {
    body.validate()?;

    let user_id = id::new_uuid();
    let password_hash = crypto::hash_password(&body.password)?;

    sqlx::query!(
        "INSERT INTO admin_users (id, username, password_hash) VALUES ($1, $2, $3)",
        user_id,
        body.username,
        password_hash,
    )
    .execute(&state.pool)
    .await?;

    write_auth_event(
        &state,
        "admin_register",
        true,
        "/admin/register",
        Some(user_id),
        None,
        None,
        None,
        Some(201),
    )
    .await?;

    let response = RegisterAdminResponse {
        user_id: user_id.to_string(),
        access_token: jwt::generate_admin_token(&user_id.to_string())?,
    };

    Ok((StatusCode::CREATED, Json(response)).into_response())
}

#[derive(Deserialize, Validate, ToSchema)]
pub struct LoginAdminRequestBody {
    #[validate(length(min = 6, max = 50, message = "Should have from 6 to 50 characters"))]
    username: String,
    #[validate(length(min = 6, max = 50, message = "Should have from 6 to 50 characters"))]
    password: String,
}

#[derive(Serialize, ToSchema)]
pub struct LoginAdminResponse {
    access_token: String,
}

#[utoipa::path(
    post,
    path = "/login",
    tag = "admin",
    request_body = LoginAdminRequestBody,
    responses(
        (status = 200, description = "Login successful", body = LoginAdminResponse),
        (status = 400, description = "Validation error"),
        (status = 404, description = "Admin not found"),
    )
)]
pub async fn login_admin_handler(
    State(state): State<AppState>,
    Json(body): Json<LoginAdminRequestBody>,
) -> Result<impl IntoResponse, AppError> {
    body.validate()?;

    let record = sqlx::query!(
        "SELECT username, password_hash, id FROM admin_users WHERE username = $1",
        body.username
    )
    .fetch_optional(&state.pool)
    .await?;

    let Some(record) = record else {
        write_auth_event(
            &state,
            "admin_login",
            false,
            "/admin/login",
            None,
            None,
            None,
            Some(body.username.as_str()),
            Some(404),
        )
        .await?;
        return Err(AppError::Sqlx(sqlx::Error::RowNotFound));
    };

    if crypto::verify_password(body.password.as_ref(), record.password_hash.as_ref()).is_err() {
        error!("Invalid hash password for admin user {}", body.username);
        write_auth_event(
            &state,
            "admin_login",
            false,
            "/admin/login",
            Some(record.id),
            None,
            None,
            Some(body.username.as_str()),
            Some(401),
        )
        .await?;
        return Err(AppError::InvalidToken);
    }

    write_auth_event(
        &state,
        "admin_login",
        true,
        "/admin/login",
        Some(record.id),
        None,
        None,
        Some(body.username.as_str()),
        Some(200),
    )
    .await?;

    let access_token = jwt::generate_admin_token(record.id.to_string().as_ref())?;
    let response = LoginAdminResponse { access_token };

    Ok((StatusCode::OK, Json(response)).into_response())
}

async fn write_auth_event(
    state: &AppState,
    event_type: &str,
    success: bool,
    route: &str,
    admin_user_id: Option<uuid::Uuid>,
    application_id: Option<uuid::Uuid>,
    application_name: Option<&str>,
    identifier: Option<&str>,
    http_status: Option<i32>,
) -> Result<(), AppError> {
    sqlx::query!(
        "INSERT INTO auth_events (id, event_type, success, route, admin_user_id, application_id, application_name, identifier, http_status) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        id::new_uuid(),
        event_type,
        success,
        route,
        admin_user_id,
        application_id,
        application_name,
        identifier,
        http_status,
    )
    .execute(&state.pool)
    .await?;

    Ok(())
}

