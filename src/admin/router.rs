use crate::router::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::{Json, Router, middleware};
use std::collections::HashMap;

use crate::crypto;
use crate::error::{AppError, ValidationErrors};
use crate::{admin, id};

use serde::{Deserialize, Serialize};

mod auth;

pub fn get_router() -> Router<AppState> {
    Router::new()
        .route("/organizations", post(organizations_handler))
        .route("/projects", post(projects_handler))
        .route("/applications", post(applications_handler))
        .route("/applications/{app_id}/scopes", put(applications_scopes_handler))
        .route("/metrics", get(metrics_handler))
        .layer(middleware::from_fn(admin::validate_admin_api_key_middleware))
        .route("/register", post(auth::register_admin_handler))
        .route("/login", post(auth::login_admin_handler))
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
    .execute(&state.pool)
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
        // TODO: move to validator crate
        let mut errors = HashMap::new();
        errors.insert("redirect_uris".to_string(), vec!["empty".to_string()]);
        return Err(AppError::ValidationError(ValidationErrors::new(errors)));
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

#[axum::debug_handler]
async fn applications_scopes_handler(
    Path(app_id): Path<String>,
    State(state): State<AppState>,
    Json(body): Json<ApplicationScopesRequestBody>,
) -> Result<impl IntoResponse, AppError> {
    let app_id = id::parse_uuid(&app_id)?;

    if body.application_scopes.is_empty()
        || body
            .application_scopes
            .iter()
            .any(|application_scope: &ApplicationScope| {
                application_scope.description.is_empty() || application_scope.name.is_empty()
            })
    {
        // TODO: move to validator crate
        let mut errors = HashMap::new();
        errors.insert("application_scopes".to_string(), vec!["empty or something".to_string()]);
        return Err(AppError::ValidationError(ValidationErrors::new(errors)));
    }

    let scopes_len = body.application_scopes.len();
    let mut permission_ids = Vec::with_capacity(scopes_len);
    let mut names = Vec::with_capacity(scopes_len);
    let mut descriptions = Vec::with_capacity(scopes_len);

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

async fn metrics_handler(State(_state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    Ok(StatusCode::CREATED)
}
