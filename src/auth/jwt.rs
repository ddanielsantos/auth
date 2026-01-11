use axum::http::HeaderMap;
use axum::http::header::AUTHORIZATION;
use jsonwebtoken::{Algorithm, DecodingKey, TokenData, Validation, decode};
use serde::{Deserialize, Serialize};
use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    exp: usize,
}

pub fn get_jwt_token(header: &HeaderMap) -> Result<&str, AppError> {
    let opt_auth_header = header
        .get(AUTHORIZATION)
        .and_then(|hv| hv.to_str().ok())
        .and_then(|s| s.split_whitespace().nth(1));
    
    if let Some(auth_header) = opt_auth_header {
        Ok(auth_header)
    } else {
        Err(AppError::HeaderNotFound)
    }
}

pub fn decode_token(token: &str) -> Result<TokenData<Claims>, AppError> {
    let decoded = decode::<Claims>(
        &token,
        &DecodingKey::from_secret("TODO: my secret".as_ref()),
        &Validation::new(Algorithm::HS256),
    );
    
    if let Ok(decoded) = decoded {
        Ok(decoded)
    } else {
        Err(AppError::InvalidToken)
    }
}
