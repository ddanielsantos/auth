use crate::admin;
use crate::auth;
use axum::Router;
use sqlx::{Pool, Postgres};

#[derive(Clone)]
pub struct AppState {
    pub pool: Pool<Postgres>,
}

impl AppState {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

pub fn routes() -> Router<AppState> {
    let admin_router = admin::router::get_router();
    let auth_router = auth::router::get_router();

    Router::new().nest("/admin", admin_router).nest("/auth", auth_router)
}
