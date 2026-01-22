use std::sync::OnceLock;

static ENV: OnceLock<Env> = OnceLock::new();

pub fn env() -> &'static Env {
    ENV.get_or_init(Env::new)
}

pub struct Env {
    pub database_url: String,
    pub admin_access_token_duration_in_minutes: u8,
    pub postgres_max_connections: u8,
    pub postgres_acquire_timeout_in_secs: u8,
    pub rate_limiter_gc_max_memory_in_mb: u8,
    pub user_access_token_duration_in_minutes: u8,
    pub admin_jwt_secret: String,
    pub user_jwt_secret: String,
}

impl Env {
    fn new() -> Self {
        Self {
            database_url: dotenvy::var("DATABASE_URL").expect("env: DATABASE_URL must be set"),
            admin_access_token_duration_in_minutes: dotenvy::var("ADMIN_ACCESS_TOKEN_DURATION_IN_MINUTES")
                .expect("env: ADMIN_ACCESS_TOKEN_DURATION_IN_MINUTES must be set")
                .parse()
                .unwrap(),
            postgres_max_connections: dotenvy::var("POSTGRES_MAX_CONNECTIONS")
                .expect("env: POSTGRES_MAX_CONNECTIONS must be set")
                .parse()
                .unwrap(),
            postgres_acquire_timeout_in_secs: dotenvy::var("POSTGRES_ACQUIRE_TIMEOUT_IN_SECS")
                .expect("env: POSTGRES_ACQUIRE_TIMEOUT_IN_SECS must be set")
                .parse()
                .unwrap(),
            rate_limiter_gc_max_memory_in_mb: dotenvy::var("RATE_LIMITER_GC_MAX_MEMORY_IN_MB")
                .expect("env: RATE_LIMITER_GC_MAX_MEMORY_IN_MB must be set")
                .parse()
                .unwrap(),
            user_access_token_duration_in_minutes: dotenvy::var("USER_ACCESS_TOKEN_DURATION_IN_MINUTES")
                .expect("env: USER_ACCESS_TOKEN_DURATION_IN_MINUTES must be set")
                .parse()
                .unwrap(),
            admin_jwt_secret: dotenvy::var("ADMIN_JWT_SECRET")
                .expect("env: ADMIN_JWT_SECRET must be set")
                .parse()
                .unwrap(),
            user_jwt_secret: dotenvy::var("USER_JWT_SECRET")
                .expect("env: USER_JWT_SECRET must be set")
                .parse()
                .unwrap(),
        }
    }
}

impl Default for Env {
    fn default() -> Self {
        Self::new()
    }
}
