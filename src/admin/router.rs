use crate::router::AppState;
use axum::extract::{Query, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{middleware, Json, Router};

use serde::{Deserialize, Serialize};
use tracing::{info};
use crate::crypto;
use crate::id;
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
    let org_id = id::new_uuid();
    sqlx::query!(
        "INSERT INTO organizations (name, id) VALUES ($1, $2) RETURNING id",
        body.name,
        org_id
    )
    .fetch_one(&state.pool)
    .await?;

    Ok((StatusCode::CREATED, org_id.to_string()).into_response())
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
    let org_id = id::parse_uuid(&body.org_id)?;
    let project_id = id::new_uuid();
    sqlx::query!(
        "INSERT INTO projects (id, org_id, name, shared_identity_context) VALUES ($1, $2, $3, $4)",
        project_id,
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

    let project_id = id::parse_uuid(&body.project_id)?;
    let client_id = id::new_uuid();
    let application_id = id::new_uuid();
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

#[derive(Deserialize)]
struct ApplicationScopesParams {
    app_id: String,
}

#[derive(Deserialize)]
struct ApplicationScope {
    name: String,
    description: String,
}

#[derive(Deserialize)]
struct ApplicationScopesRequestBody {
    application_scopes: Vec<ApplicationScope>,
}

async fn applications_scopes_handler(Query(params): Query<ApplicationScopesParams>, State(state): State<AppState>, Json(body): Json<ApplicationScopesRequestBody>) -> Result<impl IntoResponse, AppError> {
    if params.app_id.is_empty() {
        return Err(AppError::ValidationError("app_id".to_string()));
    }

    if body.application_scopes.is_empty()
        || body.application_scopes.iter().any(|application_scope: &ApplicationScope| { application_scope.description.is_empty() || application_scope.name.is_empty() }) {
        return Err(AppError::ValidationError("application_scopes".to_string()));
    }

    let app_id = id::parse_uuid(&params.app_id)?;

    let mut permission_ids = Vec::with_capacity(body.application_scopes.len());
    let mut names = Vec::with_capacity(body.application_scopes.len());
    let mut descriptions = Vec::with_capacity(body.application_scopes.len());

    for scope in &body.application_scopes {
        permission_ids.push(id::new_uuid());
        names.push(scope.name.clone());
        descriptions.push(scope.description.clone());
    }

    sqlx::query!(
        r#"
            INSERT INTO permissions (id, app_id, name, description)
            SELECT ids, $2, names, description
            FROM UNNEST($1::uuid[], $3::text[], $4::text[])
            AS t(ids, names, description)
            ON CONFLICT (app_id, name)
            DO UPDATE SET description = EXCLUDED.description
        "#,
        &permission_ids,
        app_id,
        &names,
        &descriptions
    )
        .execute(&state.pool)
        .await?;

    Ok(StatusCode::CREATED)
}

async fn metrics_handler(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    Ok(StatusCode::CREATED)
}
