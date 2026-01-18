use std::collections::HashMap;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use tracing::error;
use validator::ValidationError;

pub enum AppError {
    Argon2(argon2::password_hash::Error),
    Uuid(uuid::Error),
    Sqlx(sqlx::Error),
    InvalidUUIDVersion,
    HeaderNotFound(axum::http::header::HeaderName),
    InvalidToken,
    ValidationError(ValidationErrors),
    TimeError(std::time::SystemTimeError),
    TokenEncodeError(jsonwebtoken::errors::Error),
}

#[derive(Serialize)]
pub struct ValidationErrors {
    errors: HashMap<String, Vec<String>>,
}

impl ValidationErrors {
    pub fn new(errors: HashMap<String, Vec<String>>) -> Self {
        ValidationErrors { errors }
    }
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
            AppError::HeaderNotFound(header) => {
                let status_code = match header {
                    axum::http::header::AUTHORIZATION => StatusCode::UNAUTHORIZED,
                    _ => StatusCode::BAD_REQUEST,
                };
                (status_code, format!("Header not found: {}", header)).into_response()
            },
            AppError::InvalidToken => StatusCode::UNAUTHORIZED.into_response(),
            AppError::ValidationError(errors) => {
                (StatusCode::BAD_REQUEST, axum::Json(errors)).into_response()
            }
            AppError::TimeError(err) => {
                error!("Time error: {}", err);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
            AppError::TokenEncodeError(err) => {
                error!("Token encode error: {}", err);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
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

impl From<std::time::SystemTimeError> for AppError {
    fn from(err: std::time::SystemTimeError) -> Self {
        AppError::TimeError(err)
    }
}

impl From<validator::ValidationErrors> for AppError {
    fn from(err: validator::ValidationErrors) -> Self {
        let mut errors: HashMap<String, Vec<String>> = HashMap::new();

        for (field, validators) in err.field_errors().iter() {
            let messages: Vec<String> = validators
                .iter()
                .filter_map(|v| v.message.as_ref().map(|msg| msg.to_string()))
                .collect();

            errors.insert(field.to_string(), messages);
        }

        AppError::ValidationError(ValidationErrors::new(errors))
    }
}
