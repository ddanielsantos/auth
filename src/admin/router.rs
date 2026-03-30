use crate::router::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, middleware};
use std::collections::HashMap;

use crate::crypto;
use crate::error::{AppError, ValidationErrors};
use crate::pagination::{self, CursorPage, CursorParams};
use crate::{admin, id};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use {time, uuid};

mod auth;

pub fn get_router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(organizations_handler))
        .routes(routes!(projects_handler))
        .routes(routes!(applications_handler))
        .routes(routes!(applications_scopes_handler))
        .routes(routes!(metrics_handler))
        .routes(routes!(logs_handler))
        .layer(middleware::from_fn(admin::validate_admin_api_key_middleware))
        .routes(routes!(auth::register_admin_handler))
        .routes(routes!(auth::login_admin_handler))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct OrganizationsRequestBody {
    name: String,
}

#[utoipa::path(
    post,
    path = "/organizations",
    tag = "admin",
    security(("bearer_auth" = [])),
    request_body = OrganizationsRequestBody,
    responses(
        (status = 201, description = "Organization created, returns its UUID"),
        (status = 401, description = "Unauthorized"),
    )
)]
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

#[derive(Deserialize, ToSchema)]
pub struct ProjectsRequestBody {
    org_id: String,
    name: String,
    shared_identity_context: Option<bool>,
}

#[utoipa::path(
    post,
    path = "/projects",
    tag = "admin",
    security(("bearer_auth" = [])),
    request_body = ProjectsRequestBody,
    responses(
        (status = 201, description = "Project created, returns its UUID"),
        (status = 400, description = "Invalid org_id"),
        (status = 401, description = "Unauthorized"),
    )
)]
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

#[derive(Deserialize, ToSchema)]
pub struct ApplicationsRequestBody {
    project_id: String,
    name: String,
    redirect_uris: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub struct ApplicationsResponse {
    client_id: String,
    raw_client_secret: String,
}

#[derive(Serialize, ToSchema)]
pub struct MetricsResponse {
    total_requests_24h: i64,
    active_applications: i64,
    failed_attempts_24h: i64,
    uptime_percentage: f64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AdminLogItem {
    id: String,
    event_type: String,
    identifier: Option<String>,
    application_id: Option<String>,
    application_name: Option<String>,
    ip_address: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String, format = DateTime)]
    occurred_at: time::OffsetDateTime,
}

#[utoipa::path(
    post,
    path = "/applications",
    tag = "admin",
    security(("bearer_auth" = [])),
    request_body = ApplicationsRequestBody,
    responses(
        (status = 201, description = "Application created", body = ApplicationsResponse),
        (status = 400, description = "Validation error or invalid project_id"),
        (status = 401, description = "Unauthorized"),
    )
)]
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
        "INSERT INTO applications (id, project_id, name, client_id, client_secret_hash, redirect_uris) VALUES ($1, $2, $3, $4, $5, $6)",
        application_id,
        project_id,
        body.name,
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

#[derive(Deserialize, ToSchema)]
pub struct ApplicationScope {
    name: String,
    description: String,
}

#[derive(Deserialize, ToSchema)]
pub struct ApplicationScopesRequestBody {
    application_scopes: Vec<ApplicationScope>,
}

#[axum::debug_handler]
#[utoipa::path(
    put,
    path = "/applications/{app_id}/scopes",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("app_id" = String, Path, description = "Application ID (UUID v7)")
    ),
    request_body = ApplicationScopesRequestBody,
    responses(
        (status = 201, description = "Scopes upserted successfully"),
        (status = 400, description = "Validation error or invalid app_id"),
        (status = 401, description = "Unauthorized"),
    )
)]
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

#[utoipa::path(
    get,
    path = "/metrics",
    tag = "admin",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Metrics retrieved successfully", body = MetricsResponse),
        (status = 401, description = "Unauthorized"),
    )
)]
async fn metrics_handler(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let total_requests_24h = sqlx::query!(
        "SELECT COUNT(*)::bigint AS total_requests_24h FROM auth_events WHERE occurred_at >= NOW() - INTERVAL '24 hours'"
    )
    .fetch_one(&state.pool)
    .await?
    .total_requests_24h
    .unwrap_or(0);

    let active_applications = sqlx::query!("SELECT COUNT(*)::bigint AS active_applications FROM applications")
        .fetch_one(&state.pool)
        .await?
        .active_applications
        .unwrap_or(0);

    let failed_attempts_24h = sqlx::query!(
        "SELECT COUNT(*)::bigint AS failed_attempts_24h FROM auth_events WHERE occurred_at >= NOW() - INTERVAL '24 hours' AND success = false"
    )
    .fetch_one(&state.pool)
    .await?
    .failed_attempts_24h
    .unwrap_or(0);

    let response = MetricsResponse {
        total_requests_24h,
        active_applications,
        failed_attempts_24h,
        // Temporary fixed value until uptime is backed by external monitoring.
        uptime_percentage: 100.0,
    };

    Ok((StatusCode::OK, Json(response)))
}

#[utoipa::path(
    get,
    path = "/logs",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(CursorParams),
    responses(
        (status = 200, description = "Admin logs retrieved successfully", body = CursorPage<AdminLogItem>),
        (status = 400, description = "Invalid cursor"),
        (status = 401, description = "Unauthorized"),
    )
)]
async fn logs_handler(
    State(state): State<AppState>,
    Query(params): Query<CursorParams>,
) -> Result<impl IntoResponse, AppError> {
    let limit = params.limit();

    let items: Vec<AdminLogItem> = if let Some(ref cursor) = params.cursor {
        let (cursor_time, cursor_id) = pagination::decode_cursor(cursor)?;
        sqlx::query!(
            r#"
                SELECT
                    id,
                    event_type,
                    identifier,
                    application_id,
                    application_name,
                    ip_address,
                    occurred_at
                FROM auth_events
                WHERE (occurred_at, id) < ($1, $2)
                ORDER BY occurred_at DESC, id DESC
                LIMIT $3
            "#,
            cursor_time,
            cursor_id,
            limit + 1,
        )
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(|row| AdminLogItem {
            id: row.id.to_string(),
            event_type: row.event_type,
            identifier: row.identifier,
            application_id: row.application_id.map(|v: uuid::Uuid| v.to_string()),
            application_name: row.application_name,
            ip_address: row.ip_address,
            occurred_at: row.occurred_at,
        })
        .collect()
    } else {
        sqlx::query!(
            r#"
                SELECT
                    id,
                    event_type,
                    identifier,
                    application_id,
                    application_name,
                    ip_address,
                    occurred_at
                FROM auth_events
                ORDER BY occurred_at DESC, id DESC
                LIMIT $1
            "#,
            limit + 1,
        )
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(|row| AdminLogItem {
            id: row.id.to_string(),
            event_type: row.event_type,
            identifier: row.identifier,
            application_id: row.application_id.map(|v: uuid::Uuid| v.to_string()),
            application_name: row.application_name,
            ip_address: row.ip_address,
            occurred_at: row.occurred_at,
        })
        .collect()
    };

    let page = CursorPage::from_rows(items, limit, |item| {
        let id = uuid::Uuid::parse_str(&item.id).expect("id from DB is always a valid UUID");
        pagination::encode_cursor(item.occurred_at, id)
    });

    Ok((StatusCode::OK, Json(page)))
}

