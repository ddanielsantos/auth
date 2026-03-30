use crate::admin;
use crate::auth;
use crate::openapi::ApiDoc;
use axum::Router;
use sqlx::{Pool, Postgres};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_swagger_ui::SwaggerUi;

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
    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .nest("/admin", admin::router::get_router())
        .nest("/auth", auth::router::get_router())
        .split_for_parts();

    router.merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", api))
}
