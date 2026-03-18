use crate::error::AppError;
use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use rand::Rng;
use rand::distr::Alphanumeric;

pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    argon2
        .hash_password(password.as_ref(), &salt)
        .map(|hash| hash.to_string())
}

pub fn generate_client_secret() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

pub fn verify_password(provided_pass: &str, stored_pass: &str) -> Result<(), AppError> {
    let stored_hash = PasswordHash::new(stored_pass)?;

    Argon2::default()
        .verify_password(provided_pass.as_bytes(), &stored_hash)
        .map_err(AppError::Argon2)
}
