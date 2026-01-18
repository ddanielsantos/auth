use crate::error::AppError;
use axum::http::HeaderMap;
use axum::http::header::AUTHORIZATION;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use std::ops::Add;
use std::time::{Duration, SystemTime};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub user_type: String,
    pub exp: usize,
}

fn get_second_word(origin: &str) -> Option<&str> {
    origin.split_whitespace().nth(1)
}

pub fn get_jwt_token(headers: &HeaderMap) -> Result<&str, AppError> {
    headers
        .get(AUTHORIZATION)
        .ok_or(AppError::HeaderNotFound(AUTHORIZATION))
        .and_then(|authorization| authorization.to_str().ok().ok_or_else(|| AppError::InvalidToken))
        .and_then(|value| get_second_word(value).ok_or(AppError::InvalidToken))
}

pub fn decode_token(token: &str) -> Result<TokenData<Claims>, AppError> {
    decode::<Claims>(
        &token,
        &DecodingKey::from_secret("TODO: my secret".as_ref()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|_| AppError::InvalidToken)
}

fn generate_token(user_id: &str, user_type: &str) -> Result<String, AppError> {
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;

    let expiration = now.add(Duration::from_mins(15)).as_secs();

    let claims = Claims {
        sub: user_id.to_string(),
        user_type: user_type.to_string(),
        exp: expiration as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret("TODO: my secret".as_ref()),
    )
    .map_err(AppError::TokenEncodeError)
}

pub fn generate_admin_token(user_id: &str) -> Result<String, AppError> {
    generate_token(user_id, "admin")
}
