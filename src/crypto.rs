use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{SaltString};
use argon2::{Argon2, PasswordHasher};
use rand::distributions::Alphanumeric;
use rand::Rng;

pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    argon2
        .hash_password(password.as_ref(), &salt)
        .map(|hash| hash.to_string())
}

pub fn generate_client_secret() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}