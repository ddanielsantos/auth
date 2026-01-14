use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tracing::error;

pub enum AppError {
    Argon2(argon2::password_hash::Error),
    Uuid(uuid::Error),
    Sqlx(sqlx::Error),
    InvalidUUIDVersion,
    HeaderNotFound,
    InvalidToken,
    ValidationError(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::Argon2(err) => {
                error!("Error hashing password: {}", err);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
            AppError::Uuid(err) => (StatusCode::BAD_REQUEST, format!("UUID error: {}", err)).into_response(),
            AppError::Sqlx(ref err) => self.handle_sqlx_error(err).into_response(),
            AppError::InvalidUUIDVersion => (StatusCode::BAD_REQUEST, "Invalid UUID version").into_response(),
            AppError::HeaderNotFound => StatusCode::BAD_REQUEST.into_response(),
            AppError::InvalidToken => StatusCode::UNAUTHORIZED.into_response(),
            AppError::ValidationError(field) => {
                (StatusCode::BAD_REQUEST, format!("Validation failed for {}", field)).into_response()
            }
        }
    }
}

impl AppError {
    fn handle_sqlx_error(&self, err: &sqlx::Error) -> (StatusCode, &'static str) {
        if let sqlx::Error::RowNotFound = err {
            return (StatusCode::NOT_FOUND, "The resource does not exist");
        };

        if let Some(db_error) = err.as_database_error() {
            if db_error.is_unique_violation() {
                return (StatusCode::CONFLICT, "The resource already exists");
            }

            if db_error.is_foreign_key_violation() {
                return (StatusCode::BAD_REQUEST, "Reference to a resource that does not exist");
            }
        };

        (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
    }
}

impl From<argon2::password_hash::Error> for AppError {
    fn from(err: argon2::password_hash::Error) -> Self {
        AppError::Argon2(err)
    }
}

impl From<uuid::Error> for AppError {
    fn from(err: uuid::Error) -> Self {
        AppError::Uuid(err)
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::Sqlx(err)
    }
}
