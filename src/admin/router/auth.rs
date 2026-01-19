use crate::error::AppError;
use crate::router::AppState;
use crate::{crypto, id, jwt};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Deserialize, Validate)]
pub struct RegisterAdminRequestBody {
    #[validate(length(min = 6, max = 50, message = "Should have from 6 to 50 characters"))]
    username: String,
    #[validate(length(min = 6, max = 50, message = "Should have from 6 to 50 characters"))]
    password: String,
}

#[derive(Serialize)]
struct RegisterAdminResponse {
    user_id: String,
    access_token: String,
}

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

    let response = RegisterAdminResponse {
        user_id: user_id.to_string(),
        access_token: jwt::generate_admin_token(&user_id.to_string())?,
    };

    Ok((StatusCode::CREATED, Json(response)).into_response())
}

#[derive(Deserialize)]
pub struct LoginAdminRequestBody {
    username: String,
    password: String,
}

pub async fn login_admin_handler(
    State(_state): State<AppState>,
    Json(_body): Json<LoginAdminRequestBody>,
) -> Result<impl IntoResponse, AppError> {
    Ok((StatusCode::OK).into_response())
}
