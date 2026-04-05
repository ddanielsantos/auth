use crate::admin;
use crate::admin::authorization::{AdminId, OrgMember, ProjectMember};
use crate::crypto;
use crate::error::{AppError, ValidationErrors};
use crate::pagination::{self, CursorPage, CursorParams};
use crate::router::AppState;
use crate::id;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, middleware};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use {time, uuid};

mod auth;
mod invites;

pub fn get_router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        // Identity
        .routes(routes!(me_handler))
        .routes(routes!(list_admin_users_handler))
        // Orgs
        .routes(routes!(create_org_handler))
        .routes(routes!(list_orgs_handler))
        .routes(routes!(get_org_handler))
        // Projects
        .routes(routes!(create_project_handler))
        .routes(routes!(list_projects_handler))
        .routes(routes!(get_project_handler))
        // Applications
        .routes(routes!(create_application_handler))
        .routes(routes!(list_applications_handler))
        .routes(routes!(applications_scopes_handler))
        // Monitoring
        .routes(routes!(metrics_handler))
        .routes(routes!(logs_handler))
        // Invites
        .routes(routes!(invites::create_org_invite_handler))
        .routes(routes!(invites::create_project_invite_handler))
        .routes(routes!(invites::accept_invite_handler))
        .routes(routes!(invites::decline_invite_handler))
        .routes(routes!(invites::revoke_invite_handler))
        .layer(middleware::from_fn(admin::validate_admin_api_key_middleware))
        // Public routes — no JWT required
        .routes(routes!(auth::register_admin_handler))
        .routes(routes!(auth::login_admin_handler))
}

// ─── Response / request structs ──────────────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub struct AdminUserResponse {
    id: String,
    username: String,
}

#[derive(Serialize, ToSchema)]
pub struct OrgListItem {
    id: String,
    name: String,
    role: String,
}

#[derive(Serialize, ToSchema)]
pub struct OrgResponse {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateOrgRequestBody {
    name: String,
}

#[derive(Serialize, ToSchema)]
pub struct ProjectListItem {
    id: String,
    name: String,
    shared_identity_context: bool,
}

#[derive(Serialize, ToSchema)]
pub struct ProjectResponse {
    id: String,
    org_id: String,
    name: String,
    shared_identity_context: bool,
}

#[derive(Serialize, ToSchema)]
pub struct CreateProjectResponse {
    id: String,
    name: String,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateProjectRequestBody {
    name: String,
    shared_identity_context: Option<bool>,
}

#[derive(Serialize, ToSchema)]
pub struct ApplicationListItem {
    id: String,
    name: String,
    client_id: String,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateApplicationRequestBody {
    name: String,
    redirect_uris: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub struct ApplicationsResponse {
    client_id: String,
    raw_client_secret: String,
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

// ─── Identity handlers ────────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/me",
    tag = "admin",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Authenticated admin user details", body = AdminUserResponse),
        (status = 401, description = "Unauthorized"),
    )
)]
async fn me_handler(
    AdminId { admin_id }: AdminId,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let record = sqlx::query!(
        "SELECT id, username FROM admin_users WHERE id = $1",
        admin_id
    )
    .fetch_one(&state.pool)
    .await?;

    Ok((
        StatusCode::OK,
        Json(AdminUserResponse {
            id: record.id.to_string(),
            username: record.username,
        }),
    ))
}

#[utoipa::path(
    get,
    path = "/users",
    tag = "admin",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Admin users visible via shared orgs", body = Vec<AdminUserResponse>),
        (status = 401, description = "Unauthorized"),
    )
)]
async fn list_admin_users_handler(
    AdminId { admin_id }: AdminId,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let records = sqlx::query!(
        r#"
            SELECT DISTINCT au.id, au.username
            FROM admin_users au
            JOIN admin_org_memberships m2 ON au.id = m2.admin_user_id
            JOIN admin_org_memberships m1 ON m2.org_id = m1.org_id
            WHERE m1.admin_user_id = $1
            ORDER BY au.username
        "#,
        admin_id
    )
    .fetch_all(&state.pool)
    .await?;

    let users: Vec<AdminUserResponse> = records
        .into_iter()
        .map(|r| AdminUserResponse {
            id: r.id.to_string(),
            username: r.username,
        })
        .collect();

    Ok((StatusCode::OK, Json(users)))
}

// ─── Org handlers ─────────────────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/orgs",
    tag = "admin",
    security(("bearer_auth" = [])),
    request_body = CreateOrgRequestBody,
    responses(
        (status = 201, description = "Organization created; returns its UUID"),
        (status = 401, description = "Unauthorized"),
    )
)]
async fn create_org_handler(
    AdminId { admin_id }: AdminId,
    State(state): State<AppState>,
    Json(body): Json<CreateOrgRequestBody>,
) -> Result<impl IntoResponse, AppError> {
    let org_id = id::new_uuid();
    let membership_id = id::new_uuid();

    let mut tx = state.pool.begin().await?;

    sqlx::query!(
        "INSERT INTO organizations (id, name) VALUES ($1, $2)",
        org_id,
        body.name,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "INSERT INTO admin_org_memberships (id, admin_user_id, org_id, role) VALUES ($1, $2, $3, 'owner')",
        membership_id,
        admin_id,
        org_id,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok((StatusCode::CREATED, org_id.to_string()).into_response())
}

#[utoipa::path(
    get,
    path = "/orgs",
    tag = "admin",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Organizations the admin belongs to", body = Vec<OrgListItem>),
        (status = 401, description = "Unauthorized"),
    )
)]
async fn list_orgs_handler(
    AdminId { admin_id }: AdminId,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let records = sqlx::query!(
        r#"
            SELECT o.id, o.name, m.role
            FROM organizations o
            JOIN admin_org_memberships m ON o.id = m.org_id
            WHERE m.admin_user_id = $1
            ORDER BY o.name
        "#,
        admin_id
    )
    .fetch_all(&state.pool)
    .await?;

    let orgs: Vec<OrgListItem> = records
        .into_iter()
        .map(|r| OrgListItem {
            id: r.id.to_string(),
            name: r.name,
            role: r.role,
        })
        .collect();

    Ok((StatusCode::OK, Json(orgs)))
}

#[utoipa::path(
    get,
    path = "/orgs/{org_id}",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(("org_id" = String, Path, description = "Organization ID (UUID v7)")),
    responses(
        (status = 200, description = "Organization details", body = OrgResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Organization not found"),
    )
)]
async fn get_org_handler(
    member: OrgMember,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let record = sqlx::query!(
        "SELECT id, name FROM organizations WHERE id = $1",
        member.org_id
    )
    .fetch_one(&state.pool)
    .await?;

    Ok((
        StatusCode::OK,
        Json(OrgResponse {
            id: record.id.to_string(),
            name: record.name,
        }),
    ))
}

// ─── Project handlers ─────────────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/orgs/{org_id}/projects",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(("org_id" = String, Path, description = "Organization ID (UUID v7)")),
    request_body = CreateProjectRequestBody,
    responses(
        (status = 201, description = "Project created", body = CreateProjectResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    )
)]
async fn create_project_handler(
    member: OrgMember,
    State(state): State<AppState>,
    Json(body): Json<CreateProjectRequestBody>,
) -> Result<impl IntoResponse, AppError> {
    let project_id = id::new_uuid();
    let membership_id = id::new_uuid();

    let mut tx = state.pool.begin().await?;

    sqlx::query!(
        "INSERT INTO projects (id, org_id, name, shared_identity_context) VALUES ($1, $2, $3, $4)",
        project_id,
        member.org_id,
        body.name,
        body.shared_identity_context.unwrap_or(false),
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "INSERT INTO admin_project_memberships (id, admin_user_id, project_id, role) VALUES ($1, $2, $3, 'owner')",
        membership_id,
        member.admin_id,
        project_id,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(CreateProjectResponse {
            id: project_id.to_string(),
            name: body.name,
        }),
    ))
}

#[utoipa::path(
    get,
    path = "/orgs/{org_id}/projects",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(("org_id" = String, Path, description = "Organization ID (UUID v7)")),
    responses(
        (status = 200, description = "Projects in the organization", body = Vec<ProjectListItem>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    )
)]
async fn list_projects_handler(
    member: OrgMember,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let records = sqlx::query!(
        "SELECT id, name, shared_identity_context FROM projects WHERE org_id = $1 ORDER BY name",
        member.org_id
    )
    .fetch_all(&state.pool)
    .await?;

    let projects: Vec<ProjectListItem> = records
        .into_iter()
        .map(|r| ProjectListItem {
            id: r.id.to_string(),
            name: r.name,
            shared_identity_context: r.shared_identity_context,
        })
        .collect();

    Ok((StatusCode::OK, Json(projects)))
}

#[utoipa::path(
    get,
    path = "/orgs/{org_id}/projects/{project_id}",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("org_id" = String, Path, description = "Organization ID (UUID v7)"),
        ("project_id" = String, Path, description = "Project ID (UUID v7)"),
    ),
    responses(
        (status = 200, description = "Project details", body = ProjectResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Project not found"),
    )
)]
async fn get_project_handler(
    member: ProjectMember,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let record = sqlx::query!(
        "SELECT id, org_id, name, shared_identity_context FROM projects WHERE id = $1",
        member.project_id
    )
    .fetch_one(&state.pool)
    .await?;

    Ok((
        StatusCode::OK,
        Json(ProjectResponse {
            id: record.id.to_string(),
            org_id: record.org_id.to_string(),
            name: record.name,
            shared_identity_context: record.shared_identity_context,
        }),
    ))
}

// ─── Application handlers ─────────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/orgs/{org_id}/projects/{project_id}/applications",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("org_id" = String, Path, description = "Organization ID (UUID v7)"),
        ("project_id" = String, Path, description = "Project ID (UUID v7)"),
    ),
    request_body = CreateApplicationRequestBody,
    responses(
        (status = 201, description = "Application created", body = ApplicationsResponse),
        (status = 400, description = "Validation error"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    )
)]
async fn create_application_handler(
    member: ProjectMember,
    State(state): State<AppState>,
    Json(body): Json<CreateApplicationRequestBody>,
) -> Result<impl IntoResponse, AppError> {
    if body.redirect_uris.is_empty() {
        let mut errors = HashMap::new();
        errors.insert("redirect_uris".to_string(), vec!["empty".to_string()]);
        return Err(AppError::ValidationError(ValidationErrors::new(errors)));
    }

    let client_id = id::new_uuid();
    let application_id = id::new_uuid();
    let raw_client_secret = crypto::generate_client_secret();
    let client_secret_hash = crypto::hash_password(&raw_client_secret)?;

    sqlx::query!(
        "INSERT INTO applications (id, project_id, name, client_id, client_secret_hash, redirect_uris) VALUES ($1, $2, $3, $4, $5, $6)",
        application_id,
        member.project_id,
        body.name,
        client_id,
        client_secret_hash,
        &body.redirect_uris,
    )
    .execute(&state.pool)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApplicationsResponse {
            raw_client_secret,
            client_id: client_id.to_string(),
        }),
    ))
}

#[utoipa::path(
    get,
    path = "/orgs/{org_id}/projects/{project_id}/applications",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("org_id" = String, Path, description = "Organization ID (UUID v7)"),
        ("project_id" = String, Path, description = "Project ID (UUID v7)"),
    ),
    responses(
        (status = 200, description = "Applications in the project", body = Vec<ApplicationListItem>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    )
)]
async fn list_applications_handler(
    member: ProjectMember,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let records = sqlx::query!(
        "SELECT id, name, client_id FROM applications WHERE project_id = $1 ORDER BY name",
        member.project_id
    )
    .fetch_all(&state.pool)
    .await?;

    let apps: Vec<ApplicationListItem> = records
        .into_iter()
        .map(|r| ApplicationListItem {
            id: r.id.to_string(),
            name: r.name,
            client_id: r.client_id.to_string(),
        })
        .collect();

    Ok((StatusCode::OK, Json(apps)))
}

/// Path params struct for handlers that need `app_id` in addition to what
/// `ProjectMember` already extracts from `org_id` and `project_id`.
#[derive(Deserialize)]
struct AppIdPath {
    app_id: String,
}

#[utoipa::path(
    put,
    path = "/orgs/{org_id}/projects/{project_id}/applications/{app_id}/scopes",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("org_id" = String, Path, description = "Organization ID (UUID v7)"),
        ("project_id" = String, Path, description = "Project ID (UUID v7)"),
        ("app_id" = String, Path, description = "Application ID (UUID v7)"),
    ),
    request_body = ApplicationScopesRequestBody,
    responses(
        (status = 201, description = "Scopes upserted successfully"),
        (status = 400, description = "Validation error"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    )
)]
async fn applications_scopes_handler(
    member: ProjectMember,
    Path(AppIdPath { app_id }): Path<AppIdPath>,
    State(state): State<AppState>,
    Json(body): Json<ApplicationScopesRequestBody>,
) -> Result<impl IntoResponse, AppError> {
    let app_id = id::parse_uuid(&app_id)?;

    if body.application_scopes.is_empty()
        || body
            .application_scopes
            .iter()
            .any(|s| s.description.is_empty() || s.name.is_empty())
    {
        let mut errors = HashMap::new();
        errors.insert("application_scopes".to_string(), vec!["empty or something".to_string()]);
        return Err(AppError::ValidationError(ValidationErrors::new(errors)));
    }

    // Verify the application belongs to this project.
    let in_project: bool = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM applications WHERE id = $1 AND project_id = $2)",
        app_id,
        member.project_id,
    )
    .fetch_one(&state.pool)
    .await?
    .unwrap_or(false);

    if !in_project {
        return Err(AppError::Sqlx(sqlx::Error::RowNotFound));
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
        &descriptions,
    )
    .execute(&state.pool)
    .await?;

    Ok(StatusCode::CREATED)
}

// ─── Monitoring handlers ──────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/orgs/{org_id}/metrics",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(("org_id" = String, Path, description = "Organization ID (UUID v7)")),
    responses(
        (status = 200, description = "Org-scoped metrics", body = MetricsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    )
)]
async fn metrics_handler(
    member: OrgMember,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let total_requests_24h = sqlx::query!(
        r#"
            SELECT COUNT(*)::bigint AS total_requests_24h
            FROM auth_events ae
            JOIN applications a ON ae.application_id = a.id
            JOIN projects p ON a.project_id = p.id
            WHERE p.org_id = $1
              AND ae.occurred_at >= NOW() - INTERVAL '24 hours'
        "#,
        member.org_id,
    )
    .fetch_one(&state.pool)
    .await?
    .total_requests_24h
    .unwrap_or(0);

    let active_applications = sqlx::query!(
        r#"
            SELECT COUNT(*)::bigint AS active_applications
            FROM applications a
            JOIN projects p ON a.project_id = p.id
            WHERE p.org_id = $1
        "#,
        member.org_id,
    )
    .fetch_one(&state.pool)
    .await?
    .active_applications
    .unwrap_or(0);

    let failed_attempts_24h = sqlx::query!(
        r#"
            SELECT COUNT(*)::bigint AS failed_attempts_24h
            FROM auth_events ae
            JOIN applications a ON ae.application_id = a.id
            JOIN projects p ON a.project_id = p.id
            WHERE p.org_id = $1
              AND ae.occurred_at >= NOW() - INTERVAL '24 hours'
              AND ae.success = false
        "#,
        member.org_id,
    )
    .fetch_one(&state.pool)
    .await?
    .failed_attempts_24h
    .unwrap_or(0);

    Ok((
        StatusCode::OK,
        Json(MetricsResponse {
            total_requests_24h,
            active_applications,
            failed_attempts_24h,
            uptime_percentage: 100.0,
        }),
    ))
}

#[utoipa::path(
    get,
    path = "/orgs/{org_id}/logs",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("org_id" = String, Path, description = "Organization ID (UUID v7)"),
        CursorParams,
    ),
    responses(
        (status = 200, description = "Org-scoped admin logs", body = CursorPage<AdminLogItem>),
        (status = 400, description = "Invalid cursor"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    )
)]
async fn logs_handler(
    member: OrgMember,
    State(state): State<AppState>,
    Query(params): Query<CursorParams>,
) -> Result<impl IntoResponse, AppError> {
    let limit = params.limit();

    let items: Vec<AdminLogItem> = if let Some(ref cursor) = params.cursor {
        let (cursor_time, cursor_id) = pagination::decode_cursor(cursor)?;
        sqlx::query!(
            r#"
                SELECT ae.id, ae.event_type, ae.identifier, ae.application_id,
                       ae.application_name, ae.ip_address, ae.occurred_at
                FROM auth_events ae
                JOIN applications a ON ae.application_id = a.id
                JOIN projects p ON a.project_id = p.id
                WHERE p.org_id = $1
                  AND (ae.occurred_at, ae.id) < ($2, $3)
                ORDER BY ae.occurred_at DESC, ae.id DESC
                LIMIT $4
            "#,
            member.org_id,
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
                SELECT ae.id, ae.event_type, ae.identifier, ae.application_id,
                       ae.application_name, ae.ip_address, ae.occurred_at
                FROM auth_events ae
                JOIN applications a ON ae.application_id = a.id
                JOIN projects p ON a.project_id = p.id
                WHERE p.org_id = $1
                ORDER BY ae.occurred_at DESC, ae.id DESC
                LIMIT $2
            "#,
            member.org_id,
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

