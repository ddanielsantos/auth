use crate::error::AppError;
use crate::jwt;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use tracing::info;

pub mod router;

pub async fn validate_admin_api_key_middleware(request: Request, next: Next) -> Result<Response, AppError> {
    info!("Validating admin API key");
    let header = request.headers();
    let jwt_token = jwt::get_jwt_token(header)?;
    let user_type = jwt::decode_token(jwt_token)?.claims.user_type;

    if user_type == "admin" {
        let response = next.run(request).await;
        Ok(response)
    } else {
        Err(AppError::InvalidToken)
    }
}
