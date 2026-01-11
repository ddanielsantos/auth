use crate::router::AppState;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{middleware, Json, Router};

use serde::{Deserialize, Serialize};
use tracing::{info};
use crate::crypto;
use crate::uuid;
use crate::error::AppError;

async fn validate_admin_api_key(request: Request, next: Next) -> Result<Response, StatusCode> {
    info!("Validating admin API key");
    let response = next.run(request).await;
    Ok(response)
}

pub fn get_router() -> Router<AppState> {
    Router::new()
        .route("/organizations", post(organizations_handler))
        .route("/projects", post(projects_handler))
        .route("/applications", post(applications_handler))
        .route("/applications/{id}/scopes", put(applications_scopes_handler))
        .route("/metrics", get(metrics_handler))
        .layer(middleware::from_fn(validate_admin_api_key))
}

#[derive(Debug, Deserialize)]
struct OrganizationsRequestBody {
    name: String,
}

async fn organizations_handler(
    State(state): State<AppState>,
    Json(body): Json<OrganizationsRequestBody>,
) -> Result<impl IntoResponse, AppError> {
    let org_id = uuid::new_uuid();
    let project_id = sqlx::query_scalar!(
        "INSERT INTO organizations (name, id) VALUES ($1, $2) RETURNING id",
        body.name,
        org_id
    )
    .fetch_one(&state.pool)
    .await?;

    Ok((StatusCode::CREATED, project_id.to_string()).into_response())
}

#[derive(Deserialize)]
struct ProjectsRequestBody {
    org_id: String,
    name: String,
    shared_identity_context: Option<bool>,
}

async fn projects_handler(
    State(state): State<AppState>,
    Json(body): Json<ProjectsRequestBody>,
) -> Result<impl IntoResponse, AppError> {
    let org_id = uuid::parse_uuid(&body.org_id)?;
    let project_id = sqlx::query_scalar!(
        "INSERT INTO projects (id, org_id, name, shared_identity_context) VALUES ($1, $2, $3, $4) RETURNING id",
        uuid::new_uuid(),
        org_id,
        body.name,
        body.shared_identity_context.unwrap_or(false)
    )
    .fetch_one(&state.pool)
    .await?;

    Ok((StatusCode::CREATED, project_id.to_string()).into_response())
}

#[derive(Deserialize)]
struct ApplicationsRequestBody {
    project_id: String,
    redirect_uris: Vec<String>,
}

#[derive(Serialize)]
struct ApplicationsResponse {
    client_id: String,
    raw_client_secret: String,
}

async fn applications_handler(
    State(state): State<AppState>,
    Json(body): Json<ApplicationsRequestBody>,
) -> Result<impl IntoResponse, AppError> {
    if body.redirect_uris.is_empty() {
        return Err(AppError::ValidationError("redirect_uris".to_string()));
    }

    let project_id = uuid::parse_uuid(&body.project_id)?;
    let client_id = uuid::new_uuid();
    let application_id = uuid::new_uuid();
    let raw_client_secret = crypto::generate_client_secret();
    let client_secret_hash = crypto::hash_password(&raw_client_secret)?;

    sqlx::query!(
        "INSERT INTO applications (id, project_id, client_id, client_secret_hash, redirect_uris) VALUES ($1, $2, $3, $4, $5)",
        application_id,
        project_id,
        client_id,
        client_secret_hash,
        &body.redirect_uris
    )
        .execute(&state.pool)
        .await?;

    let response = ApplicationsResponse {
        raw_client_secret,
        client_id: client_id.to_string(),
    };

    Ok((StatusCode::CREATED, Json(response)))
}

async fn applications_scopes_handler(State(state): State<AppState>) -> Result<impl IntoResponse, StatusCode> {
    Ok(StatusCode::CREATED)
}

async fn metrics_handler(State(state): State<AppState>) -> Result<impl IntoResponse, StatusCode> {
    Ok(StatusCode::CREATED)
}
