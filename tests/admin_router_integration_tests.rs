#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use std::sync::Once;
use tower::ServiceExt;

fn init_test_env() {
    static INIT: Once = Once::new();

    INIT.call_once(|| unsafe {
        std::env::set_var("ADMIN_ACCESS_TOKEN_DURATION_IN_MINUTES", "60");
        std::env::set_var("POSTGRES_MAX_CONNECTIONS", "5");
        std::env::set_var("POSTGRES_ACQUIRE_TIMEOUT_IN_SECS", "5");
        std::env::set_var("RATE_LIMITER_GC_MAX_MEMORY_IN_MB", "64");
        std::env::set_var("USER_ACCESS_TOKEN_DURATION_IN_MINUTES", "60");
        std::env::set_var("ADMIN_JWT_SECRET", "test-admin-secret");
        std::env::set_var("USER_JWT_SECRET", "test-user-secret");
    });
}

fn test_app(pool: PgPool) -> axum::Router {
    let state = study_auth::router::AppState::new(pool);
    study_auth::router::routes().with_state(state)
}

async fn json_body(response: axum::response::Response) -> Value {
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body_bytes).unwrap_or_else(|_| json!({}))
}

async fn text_body(response: axum::response::Response) -> String {
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(body_bytes.to_vec()).unwrap()
}

fn json_request(method: &str, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn auth_json_request(method: &str, uri: &str, body: Value, token: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn auth_request(method: &str, uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

fn admin_token(admin_id: uuid::Uuid) -> String {
    init_test_env();
    study_auth::jwt::generate_admin_token(&admin_id.to_string())
        .unwrap_or_else(|_| panic!("failed to generate admin token for tests"))
}

// ─── DB helpers ───────────────────────────────────────────────────────────────

async fn insert_admin_user(pool: &PgPool, username: &str) -> uuid::Uuid {
    let admin_id = study_auth::id::new_uuid();
    let password_hash = study_auth::crypto::hash_password("password-123").unwrap();
    sqlx::query("INSERT INTO admin_users (id, username, password_hash) VALUES ($1, $2, $3)")
        .bind(admin_id)
        .bind(username)
        .bind(password_hash)
        .execute(pool)
        .await
        .unwrap();
    admin_id
}

/// Creates an admin user and returns (admin_id, jwt_token).
async fn create_admin(pool: &PgPool, username: &str) -> (uuid::Uuid, String) {
    let admin_id = insert_admin_user(pool, username).await;
    let token = admin_token(admin_id);
    (admin_id, token)
}

async fn insert_organization(pool: &PgPool, name: &str) -> uuid::Uuid {
    let org_id = study_auth::id::new_uuid();
    sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, $2)")
        .bind(org_id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    org_id
}

async fn insert_project(pool: &PgPool, org_id: uuid::Uuid, name: &str) -> uuid::Uuid {
    let project_id = study_auth::id::new_uuid();
    sqlx::query(
        "INSERT INTO projects (id, org_id, name, shared_identity_context) VALUES ($1, $2, $3, $4)",
    )
    .bind(project_id)
    .bind(org_id)
    .bind(name)
    .bind(false)
    .execute(pool)
    .await
    .unwrap();
    project_id
}

async fn insert_application(pool: &PgPool, project_id: uuid::Uuid) -> uuid::Uuid {
    let application_id = study_auth::id::new_uuid();
    let client_id = study_auth::id::new_uuid();
    let client_secret_hash = study_auth::crypto::hash_password("existing-secret").unwrap();
    sqlx::query(
        "INSERT INTO applications (id, project_id, name, client_id, client_secret_hash, redirect_uris) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(application_id)
    .bind(project_id)
    .bind("Test Application")
    .bind(client_id)
    .bind(client_secret_hash)
    .bind(vec!["https://example.com/callback"])
    .execute(pool)
    .await
    .unwrap();
    application_id
}

async fn insert_org_membership(pool: &PgPool, admin_id: uuid::Uuid, org_id: uuid::Uuid, role: &str) {
    let id = study_auth::id::new_uuid();
    sqlx::query(
        "INSERT INTO admin_org_memberships (id, admin_user_id, org_id, role) VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(admin_id)
    .bind(org_id)
    .bind(role)
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_project_membership(
    pool: &PgPool,
    admin_id: uuid::Uuid,
    project_id: uuid::Uuid,
    role: &str,
) {
    let id = study_auth::id::new_uuid();
    sqlx::query(
        "INSERT INTO admin_project_memberships (id, admin_user_id, project_id, role) VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(admin_id)
    .bind(project_id)
    .bind(role)
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_auth_event_with_details(
    pool: &PgPool,
    event_type: &str,
    route: &str,
    identifier: Option<&str>,
    application_id: Option<uuid::Uuid>,
    application_name: Option<&str>,
) {
    sqlx::query(
        "INSERT INTO auth_events (id, event_type, success, route, identifier, application_id, application_name, occurred_at) VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())",
    )
    .bind(study_auth::id::new_uuid())
    .bind(event_type)
    .bind(true)
    .bind(route)
    .bind(identifier)
    .bind(application_id)
    .bind(application_name)
    .execute(pool)
    .await
    .unwrap();
}

// ─── Admin auth tests ─────────────────────────────────────────────────────────

#[sqlx::test(migrations = "infra/migrations")]
async fn register_admin_creates_user_and_returns_token(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();

    let response = test_app(pool.clone())
        .oneshot(json_request(
            "POST",
            "/admin/register",
            json!({ "username": "admin-user", "password": "secret-123" }),
        ))
        .await?;

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = json_body(response).await;
    let user_id = body["user_id"].as_str().unwrap();
    let access_token = body["access_token"].as_str().unwrap();

    let persisted_username: String = sqlx::query_scalar("SELECT username FROM admin_users WHERE id = $1")
        .bind(uuid::Uuid::parse_str(user_id)?)
        .fetch_one(&pool)
        .await?;

    assert_eq!(persisted_username, "admin-user");

    let claims = study_auth::jwt::decode_admin_token(access_token)
        .unwrap_or_else(|_| panic!("failed to decode admin token"))
        .claims;
    assert_eq!(claims.sub, user_id);
    assert_eq!(claims.user_type, "admin");

    Ok(())
}

#[sqlx::test(migrations = "infra/migrations")]
async fn register_admin_rejects_duplicate_username(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    insert_admin_user(&pool, "same-admin").await;

    let response = test_app(pool)
        .oneshot(json_request(
            "POST",
            "/admin/register",
            json!({ "username": "same-admin", "password": "secret-123" }),
        ))
        .await?;

    assert_eq!(response.status(), StatusCode::CONFLICT);
    Ok(())
}

#[sqlx::test(migrations = "infra/migrations")]
async fn login_admin_returns_access_token_for_valid_credentials(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let admin_id = insert_admin_user(&pool, "login-admin").await;

    let response = test_app(pool)
        .oneshot(json_request(
            "POST",
            "/admin/login",
            json!({ "username": "login-admin", "password": "password-123" }),
        ))
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    let access_token = body["access_token"].as_str().unwrap();
    let claims = study_auth::jwt::decode_admin_token(access_token)
        .unwrap_or_else(|_| panic!("failed to decode admin token"))
        .claims;

    assert_eq!(claims.sub, admin_id.to_string());
    assert_eq!(claims.user_type, "admin");
    Ok(())
}

#[sqlx::test(migrations = "infra/migrations")]
async fn login_admin_returns_not_found_for_unknown_user(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();

    let response = test_app(pool)
        .oneshot(json_request(
            "POST",
            "/admin/login",
            json!({ "username": "missing-admin", "password": "secret-123" }),
        ))
        .await?;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    Ok(())
}

// ─── GET /admin/me ────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "infra/migrations")]
async fn me_endpoint_returns_admin_details(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let (admin_id, token) = create_admin(&pool, "me-admin").await;

    let response = test_app(pool)
        .oneshot(auth_request("GET", "/admin/me", &token))
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    assert_eq!(body["id"], admin_id.to_string());
    assert_eq!(body["username"], "me-admin");
    Ok(())
}

#[sqlx::test(migrations = "infra/migrations")]
async fn me_endpoint_requires_auth(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    let response = test_app(pool)
        .oneshot(Request::builder().method("GET").uri("/admin/me").body(Body::empty()).unwrap())
        .await?;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

// ─── POST /admin/orgs ─────────────────────────────────────────────────────────

#[sqlx::test(migrations = "infra/migrations")]
async fn create_org_requires_auth(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    let response = test_app(pool)
        .oneshot(json_request("POST", "/admin/orgs", json!({ "name": "Acme" })))
        .await?;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[sqlx::test(migrations = "infra/migrations")]
async fn create_org_creates_org_and_assigns_owner(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let (admin_id, token) = create_admin(&pool, "org-admin").await;

    let response = test_app(pool.clone())
        .oneshot(auth_json_request("POST", "/admin/orgs", json!({ "name": "Acme" }), &token))
        .await?;

    assert_eq!(response.status(), StatusCode::CREATED);

    let org_id = uuid::Uuid::parse_str(text_body(response).await.trim())?;

    let org_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM organizations WHERE id = $1")
        .bind(org_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(org_count, 1);

    let role: String = sqlx::query_scalar(
        "SELECT role FROM admin_org_memberships WHERE admin_user_id = $1 AND org_id = $2",
    )
    .bind(admin_id)
    .bind(org_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(role, "owner");

    Ok(())
}

// ─── GET /admin/orgs ─────────────────────────────────────────────────────────

#[sqlx::test(migrations = "infra/migrations")]
async fn list_orgs_returns_only_orgs_caller_belongs_to(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let (admin_id, token) = create_admin(&pool, "orgs-admin").await;

    let org_a = insert_organization(&pool, "Org A").await;
    let org_b = insert_organization(&pool, "Org B").await;
    let _org_c = insert_organization(&pool, "Org C (not joined)").await;

    insert_org_membership(&pool, admin_id, org_a, "owner").await;
    insert_org_membership(&pool, admin_id, org_b, "admin").await;

    let response = test_app(pool)
        .oneshot(auth_request("GET", "/admin/orgs", &token))
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    let orgs = body.as_array().unwrap();
    assert_eq!(orgs.len(), 2);
    assert!(orgs.iter().any(|o| o["name"] == "Org A"));
    assert!(orgs.iter().any(|o| o["name"] == "Org B"));
    Ok(())
}

// ─── GET /admin/orgs/{org_id} ─────────────────────────────────────────────────

#[sqlx::test(migrations = "infra/migrations")]
async fn get_org_returns_403_for_non_member(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let (_admin_id, token) = create_admin(&pool, "outsider").await;
    let org_id = insert_organization(&pool, "Secret Org").await;

    let response = test_app(pool)
        .oneshot(auth_request("GET", &format!("/admin/orgs/{org_id}"), &token))
        .await?;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    Ok(())
}

#[sqlx::test(migrations = "infra/migrations")]
async fn get_org_returns_org_for_member(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let (admin_id, token) = create_admin(&pool, "member-admin").await;
    let org_id = insert_organization(&pool, "My Org").await;
    insert_org_membership(&pool, admin_id, org_id, "owner").await;

    let response = test_app(pool)
        .oneshot(auth_request("GET", &format!("/admin/orgs/{org_id}"), &token))
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    assert_eq!(body["name"], "My Org");
    Ok(())
}

// ─── POST /admin/orgs/{org_id}/projects ───────────────────────────────────────

#[sqlx::test(migrations = "infra/migrations")]
async fn create_project_forbidden_for_non_member(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let (_admin_id, token) = create_admin(&pool, "outsider").await;
    let org_id = insert_organization(&pool, "Acme").await;

    let response = test_app(pool)
        .oneshot(auth_json_request(
            "POST",
            &format!("/admin/orgs/{org_id}/projects"),
            json!({ "name": "Project X" }),
            &token,
        ))
        .await?;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    Ok(())
}

#[sqlx::test(migrations = "infra/migrations")]
async fn create_project_creates_record_assigns_owner_and_defaults_shared_context(
    pool: PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let (admin_id, token) = create_admin(&pool, "proj-admin").await;
    let org_id = insert_organization(&pool, "Acme").await;
    insert_org_membership(&pool, admin_id, org_id, "owner").await;

    let response = test_app(pool.clone())
        .oneshot(auth_json_request(
            "POST",
            &format!("/admin/orgs/{org_id}/projects"),
            json!({ "name": "Project X" }),
            &token,
        ))
        .await?;

    assert_eq!(response.status(), StatusCode::CREATED);

    let project_id = uuid::Uuid::parse_str(text_body(response).await.trim())?;

    let row = sqlx::query("SELECT org_id, shared_identity_context FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_one(&pool)
        .await?;

    assert_eq!(row.get::<uuid::Uuid, _>("org_id"), org_id);
    assert!(!row.get::<bool, _>("shared_identity_context"));

    let role: String = sqlx::query_scalar(
        "SELECT role FROM admin_project_memberships WHERE admin_user_id = $1 AND project_id = $2",
    )
    .bind(admin_id)
    .bind(project_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(role, "owner");

    Ok(())
}

// ─── POST /admin/orgs/{org_id}/projects/{project_id}/applications ─────────────

#[sqlx::test(migrations = "infra/migrations")]
async fn create_application_validates_redirect_uris(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let (admin_id, token) = create_admin(&pool, "app-admin").await;
    let org_id = insert_organization(&pool, "Acme").await;
    let project_id = insert_project(&pool, org_id, "Project X").await;
    insert_org_membership(&pool, admin_id, org_id, "owner").await;

    let response = test_app(pool)
        .oneshot(auth_json_request(
            "POST",
            &format!("/admin/orgs/{org_id}/projects/{project_id}/applications"),
            json!({ "name": "Test App", "redirect_uris": [] }),
            &token,
        ))
        .await?;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = json_body(response).await;
    assert_eq!(body["errors"]["redirect_uris"][0], "empty");
    Ok(())
}

#[sqlx::test(migrations = "infra/migrations")]
async fn create_application_creates_app_and_stores_hashed_secret(
    pool: PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let (admin_id, token) = create_admin(&pool, "app-admin").await;
    let org_id = insert_organization(&pool, "Acme").await;
    let project_id = insert_project(&pool, org_id, "Project X").await;
    insert_org_membership(&pool, admin_id, org_id, "owner").await;

    let response = test_app(pool.clone())
        .oneshot(auth_json_request(
            "POST",
            &format!("/admin/orgs/{org_id}/projects/{project_id}/applications"),
            json!({
                "name": "My App",
                "redirect_uris": ["https://example.com/callback", "https://example.com/return"]
            }),
            &token,
        ))
        .await?;

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = json_body(response).await;
    let client_id = uuid::Uuid::parse_str(body["client_id"].as_str().unwrap())?;
    let raw_client_secret = body["raw_client_secret"].as_str().unwrap();

    let row = sqlx::query(
        "SELECT project_id, client_secret_hash, redirect_uris FROM applications WHERE client_id = $1",
    )
    .bind(client_id)
    .fetch_one(&pool)
    .await?;

    assert_eq!(row.get::<uuid::Uuid, _>("project_id"), project_id);
    assert_ne!(row.get::<String, _>("client_secret_hash"), raw_client_secret);
    assert_eq!(
        row.get::<Vec<String>, _>("redirect_uris"),
        vec!["https://example.com/callback".to_string(), "https://example.com/return".to_string()]
    );
    Ok(())
}

// ─── PUT /admin/orgs/{org_id}/projects/{project_id}/applications/{app_id}/scopes

#[sqlx::test(migrations = "infra/migrations")]
async fn application_scopes_inserts_and_updates_permissions(
    pool: PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let (admin_id, token) = create_admin(&pool, "scopes-admin").await;
    let org_id = insert_organization(&pool, "Acme").await;
    let project_id = insert_project(&pool, org_id, "Project X").await;
    let application_id = insert_application(&pool, project_id).await;
    insert_org_membership(&pool, admin_id, org_id, "owner").await;

    let first_response = test_app(pool.clone())
        .oneshot(auth_json_request(
            "PUT",
            &format!("/admin/orgs/{org_id}/projects/{project_id}/applications/{application_id}/scopes"),
            json!({
                "application_scopes": [
                    { "name": "read:users", "description": "Read users" },
                    { "name": "write:users", "description": "Write users" }
                ]
            }),
            &token,
        ))
        .await?;

    assert_eq!(first_response.status(), StatusCode::CREATED);

    let update_response = test_app(pool.clone())
        .oneshot(auth_json_request(
            "PUT",
            &format!("/admin/orgs/{org_id}/projects/{project_id}/applications/{application_id}/scopes"),
            json!({
                "application_scopes": [
                    { "name": "read:users", "description": "Read users v2" },
                    { "name": "write:users", "description": "Write users" }
                ]
            }),
            &token,
        ))
        .await?;

    assert_eq!(update_response.status(), StatusCode::CREATED);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM permissions WHERE app_id = $1")
        .bind(application_id)
        .fetch_one(&pool)
        .await?;
    let description: Option<String> =
        sqlx::query_scalar("SELECT description FROM permissions WHERE app_id = $1 AND name = $2")
            .bind(application_id)
            .bind("read:users")
            .fetch_one(&pool)
            .await?;

    assert_eq!(count, 2);
    assert_eq!(description.as_deref(), Some("Read users v2"));
    Ok(())
}

// ─── GET /admin/orgs/{org_id}/metrics ─────────────────────────────────────────

#[sqlx::test(migrations = "infra/migrations")]
async fn metrics_endpoint_forbidden_for_non_member(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let (_admin_id, token) = create_admin(&pool, "outsider").await;
    let org_id = insert_organization(&pool, "Acme").await;

    let response = test_app(pool)
        .oneshot(auth_request("GET", &format!("/admin/orgs/{org_id}/metrics"), &token))
        .await?;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    Ok(())
}

#[sqlx::test(migrations = "infra/migrations")]
async fn metrics_endpoint_returns_org_scoped_counts(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let (admin_id, token) = create_admin(&pool, "metrics-admin").await;
    let org_id = insert_organization(&pool, "Acme").await;
    let project_id = insert_project(&pool, org_id, "Project X").await;
    insert_org_membership(&pool, admin_id, org_id, "owner").await;

    let app1 = insert_application(&pool, project_id).await;
    let app2 = insert_application(&pool, project_id).await;

    // Two recent events on app1, one old event on app2, one event on another org's app
    let other_org = insert_organization(&pool, "Other Org").await;
    let other_project = insert_project(&pool, other_org, "Other Project").await;
    let other_app = insert_application(&pool, other_project).await;

    insert_auth_event_with_details(&pool, "user_login", "/auth/login", None, Some(app1), Some("Test Application")).await;
    insert_auth_event_with_details(&pool, "user_login", "/auth/login", None, Some(app1), Some("Test Application")).await;
    insert_auth_event_with_details(&pool, "user_login", "/auth/login", None, Some(other_app), Some("Other App")).await;

    let response = test_app(pool)
        .oneshot(auth_request("GET", &format!("/admin/orgs/{org_id}/metrics"), &token))
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    assert_eq!(body["total_requests_24h"], 2, "should only count events in this org");
    assert_eq!(body["active_applications"], 2, "should only count apps in this org");
    assert_eq!(body["failed_attempts_24h"], 0);
    assert_eq!(body["uptime_percentage"], 100.0);

    let _ = app2; // inserted but no events
    Ok(())
}

// ─── GET /admin/orgs/{org_id}/logs ────────────────────────────────────────────

#[sqlx::test(migrations = "infra/migrations")]
async fn logs_endpoint_returns_org_scoped_paginated_logs(
    pool: PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let (admin_id, token) = create_admin(&pool, "logs-admin").await;
    let org_id = insert_organization(&pool, "Acme").await;
    let project_id = insert_project(&pool, org_id, "Project X").await;
    let application_id = insert_application(&pool, project_id).await;
    insert_org_membership(&pool, admin_id, org_id, "owner").await;

    // Event in this org
    insert_auth_event_with_details(
        &pool,
        "user_login",
        "/auth/login",
        Some("user-1"),
        Some(application_id),
        Some("Test Application"),
    )
    .await;
    // Second event in this org
    insert_auth_event_with_details(
        &pool,
        "user_register",
        "/auth/register",
        Some("user-2"),
        Some(application_id),
        Some("Test Application"),
    )
    .await;
    // Event in another org — should NOT appear
    let other_org = insert_organization(&pool, "Other Org").await;
    let other_project = insert_project(&pool, other_org, "Other Project").await;
    let other_app = insert_application(&pool, other_project).await;
    insert_auth_event_with_details(
        &pool,
        "user_login",
        "/auth/login",
        None,
        Some(other_app),
        Some("Other App"),
    )
    .await;

    // First page: limit=1 — should return 1 item and a next_cursor.
    let response = test_app(pool.clone())
        .oneshot(auth_request("GET", &format!("/admin/orgs/{org_id}/logs?limit=1"), &token))
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert!(body["items"][0]["occurred_at"].as_str().is_some());

    let next_cursor = body["next_cursor"]
        .as_str()
        .expect("next_cursor should be present on first page");

    // Second page: should return the other org-scoped event and no next_cursor.
    let url = format!("/admin/orgs/{org_id}/logs?limit=1&cursor={next_cursor}");
    let response = test_app(pool)
        .oneshot(auth_request("GET", &url, &token))
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert!(body["next_cursor"].is_null(), "next_cursor should be null on the last page");

    Ok(())
}

// ─── Authorization guard: org_id from URL is enforced ─────────────────────────

#[sqlx::test(migrations = "infra/migrations")]
async fn project_endpoint_returns_404_if_project_not_in_org(
    pool: PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let (admin_id, token) = create_admin(&pool, "wrong-org-admin").await;

    let org_a = insert_organization(&pool, "Org A").await;
    let org_b = insert_organization(&pool, "Org B").await;
    let project_in_b = insert_project(&pool, org_b, "Project in B").await;

    insert_org_membership(&pool, admin_id, org_a, "owner").await;
    insert_project_membership(&pool, admin_id, project_in_b, "owner").await;

    // Try to access project_in_b via org_a's URL — should be 404 (project not in org)
    let response = test_app(pool)
        .oneshot(auth_request(
            "GET",
            &format!("/admin/orgs/{org_a}/projects/{project_in_b}"),
            &token,
        ))
        .await?;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    Ok(())
}

// ─── GET /admin/users ────────────────────────────────────────────────────────

#[sqlx::test(migrations = "infra/migrations")]
async fn list_users_returns_admins_in_shared_orgs(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let (admin_a, token_a) = create_admin(&pool, "admin-a").await;
    let (admin_b, _) = create_admin(&pool, "admin-b").await;
    let (_admin_c, _) = create_admin(&pool, "admin-c").await; // not in same org

    let org = insert_organization(&pool, "Shared Org").await;
    insert_org_membership(&pool, admin_a, org, "owner").await;
    insert_org_membership(&pool, admin_b, org, "admin").await;

    let response = test_app(pool)
        .oneshot(auth_request("GET", "/admin/users", &token_a))
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    let users = body.as_array().unwrap();
    assert_eq!(users.len(), 2, "should see admin-a and admin-b; not admin-c");
    Ok(())
}

// ─── Invite flow ─────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "infra/migrations")]
async fn invite_flow_org_accept(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let (issuer_id, issuer_token) = create_admin(&pool, "invite-issuer").await;
    let (invitee_id, invitee_token) = create_admin(&pool, "invite-target").await;

    let org_id = insert_organization(&pool, "Acme").await;
    insert_org_membership(&pool, issuer_id, org_id, "owner").await;

    // Create invite
    let create_response = test_app(pool.clone())
        .oneshot(auth_json_request(
            "POST",
            &format!("/admin/orgs/{org_id}/invites"),
            json!({ "invitee_username": "invite-target", "role": "admin" }),
            &issuer_token,
        ))
        .await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);

    let invite_id = json_body(create_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Accept invite as invitee
    let accept_response = test_app(pool.clone())
        .oneshot(auth_request(
            "POST",
            &format!("/admin/invites/{invite_id}/accept"),
            &invitee_token,
        ))
        .await?;
    assert_eq!(accept_response.status(), StatusCode::NO_CONTENT);

    // Invitee is now a member
    let role: String = sqlx::query_scalar(
        "SELECT role FROM admin_org_memberships WHERE admin_user_id = $1 AND org_id = $2",
    )
    .bind(invitee_id)
    .bind(org_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(role, "admin");

    // Invite status is accepted
    let status: String = sqlx::query_scalar("SELECT status FROM admin_invites WHERE id = $1")
        .bind(uuid::Uuid::parse_str(&invite_id)?)
        .fetch_one(&pool)
        .await?;
    assert_eq!(status, "accepted");

    Ok(())
}

#[sqlx::test(migrations = "infra/migrations")]
async fn invite_decline_updates_status(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let (issuer_id, issuer_token) = create_admin(&pool, "decliner-issuer").await;
    let (_invitee_id, invitee_token) = create_admin(&pool, "decliner-target").await;

    let org_id = insert_organization(&pool, "Acme").await;
    insert_org_membership(&pool, issuer_id, org_id, "owner").await;

    let create_response = test_app(pool.clone())
        .oneshot(auth_json_request(
            "POST",
            &format!("/admin/orgs/{org_id}/invites"),
            json!({ "invitee_username": "decliner-target", "role": "admin" }),
            &issuer_token,
        ))
        .await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);

    let invite_id = json_body(create_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    let decline_response = test_app(pool.clone())
        .oneshot(auth_request(
            "POST",
            &format!("/admin/invites/{invite_id}/decline"),
            &invitee_token,
        ))
        .await?;
    assert_eq!(decline_response.status(), StatusCode::NO_CONTENT);

    let status: String = sqlx::query_scalar("SELECT status FROM admin_invites WHERE id = $1")
        .bind(uuid::Uuid::parse_str(&invite_id)?)
        .fetch_one(&pool)
        .await?;
    assert_eq!(status, "declined");

    Ok(())
}

#[sqlx::test(migrations = "infra/migrations")]
async fn invite_revoke_by_issuer_updates_status(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let (issuer_id, issuer_token) = create_admin(&pool, "revoker-issuer").await;
    let _invitee_id = insert_admin_user(&pool, "revoker-target").await;

    let org_id = insert_organization(&pool, "Acme").await;
    insert_org_membership(&pool, issuer_id, org_id, "owner").await;

    let create_response = test_app(pool.clone())
        .oneshot(auth_json_request(
            "POST",
            &format!("/admin/orgs/{org_id}/invites"),
            json!({ "invitee_username": "revoker-target", "role": "admin" }),
            &issuer_token,
        ))
        .await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);

    let invite_id = json_body(create_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    let revoke_response = test_app(pool.clone())
        .oneshot(auth_request(
            "POST",
            &format!("/admin/invites/{invite_id}/revoke"),
            &issuer_token,
        ))
        .await?;
    assert_eq!(revoke_response.status(), StatusCode::NO_CONTENT);

    let status: String = sqlx::query_scalar("SELECT status FROM admin_invites WHERE id = $1")
        .bind(uuid::Uuid::parse_str(&invite_id)?)
        .fetch_one(&pool)
        .await?;
    assert_eq!(status, "revoked");

    Ok(())
}

#[sqlx::test(migrations = "infra/migrations")]
async fn invite_accept_forbidden_for_wrong_invitee(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let (issuer_id, issuer_token) = create_admin(&pool, "fi-issuer").await;
    let (_wrong_admin, wrong_token) = create_admin(&pool, "fi-wrong").await;
    let _target = insert_admin_user(&pool, "fi-target").await;

    let org_id = insert_organization(&pool, "Acme").await;
    insert_org_membership(&pool, issuer_id, org_id, "owner").await;

    let create_response = test_app(pool.clone())
        .oneshot(auth_json_request(
            "POST",
            &format!("/admin/orgs/{org_id}/invites"),
            json!({ "invitee_username": "fi-target", "role": "admin" }),
            &issuer_token,
        ))
        .await?;
    let invite_id = json_body(create_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Wrong admin tries to accept
    let response = test_app(pool)
        .oneshot(auth_request(
            "POST",
            &format!("/admin/invites/{invite_id}/accept"),
            &wrong_token,
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    Ok(())
}

