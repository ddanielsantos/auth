use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
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

pub fn verify_password(
    password: &str,
    hash: &argon2::password_hash::PasswordHash,
) -> Result<bool, argon2::password_hash::Error> {
    let provided_hash = hash_password(password)?;
    let argon2 = Argon2::default();

    match argon2.verify_password(provided_hash.as_ref(), hash) {
        Ok(_value) => Ok(true),
        Err(err) => Err(err),
    }
}
