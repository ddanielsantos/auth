pub struct Env {
    pub database_url: String,
}

impl Env {
    pub fn new() -> Self {
        Self {
            database_url: dotenvy::var("DATABASE_URL").expect("env: DATABASE_URL must be set"),
        }
    }
}

impl Default for Env {
    fn default() -> Self {
        Self::new()
    }
}
