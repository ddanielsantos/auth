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

async fn insert_admin_user(pool: &PgPool, username: &str, password: &str) -> uuid::Uuid {
    let admin_id = study_auth::id::new_uuid();
    let password_hash = study_auth::crypto::hash_password(password).unwrap();

    sqlx::query("INSERT INTO admin_users (id, username, password_hash) VALUES ($1, $2, $3)")
        .bind(admin_id)
        .bind(username)
        .bind(password_hash)
        .execute(pool)
        .await
        .unwrap();

    admin_id
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

    sqlx::query("INSERT INTO projects (id, org_id, name, shared_identity_context) VALUES ($1, $2, $3, $4)")
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
        "INSERT INTO applications (id, project_id, client_id, client_secret_hash, redirect_uris) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(application_id)
    .bind(project_id)
    .bind(client_id)
    .bind(client_secret_hash)
    .bind(vec!["https://example.com/callback"])
    .execute(pool)
    .await
    .unwrap();

    application_id
}

#[sqlx::test(migrations = "infra/migrations")]
async fn register_admin_creates_user_and_returns_token(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();

    let response = test_app(pool.clone())
        .oneshot(json_request(
            "POST",
            "/admin/register",
            json!({
                "username": "admin-user",
                "password": "secret-123"
            }),
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
        .unwrap_or_else(|_| panic!("failed to decode admin token from register response"))
        .claims;
    assert_eq!(claims.sub, user_id);
    assert_eq!(claims.user_type, "admin");

    Ok(())
}

#[sqlx::test(migrations = "infra/migrations")]
async fn register_admin_rejects_duplicate_username(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    insert_admin_user(&pool, "same-admin", "secret-123").await;

    let response = test_app(pool)
        .oneshot(json_request(
            "POST",
            "/admin/register",
            json!({
                "username": "same-admin",
                "password": "secret-123"
            }),
        ))
        .await?;

    assert_eq!(response.status(), StatusCode::CONFLICT);

    Ok(())
}

#[sqlx::test(migrations = "infra/migrations")]
async fn login_admin_returns_access_token_for_valid_credentials(
    pool: PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let admin_id = insert_admin_user(&pool, "login-admin", "secret-123").await;

    let response = test_app(pool)
        .oneshot(json_request(
            "POST",
            "/admin/login",
            json!({
                "username": "login-admin",
                "password": "secret-123"
            }),
        ))
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    let access_token = body["access_token"].as_str().unwrap();
    let claims = study_auth::jwt::decode_admin_token(access_token)
        .unwrap_or_else(|_| panic!("failed to decode admin token from login response"))
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
            json!({
                "username": "missing-admin",
                "password": "secret-123"
            }),
        ))
        .await?;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[sqlx::test(migrations = "infra/migrations")]
async fn organizations_endpoint_requires_admin_token(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    let response = test_app(pool)
        .oneshot(json_request("POST", "/admin/organizations", json!({ "name": "Acme" })))
        .await?;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    Ok(())
}

#[sqlx::test(migrations = "infra/migrations")]
async fn organizations_endpoint_creates_record_with_valid_admin_token(
    pool: PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let token = admin_token(study_auth::id::new_uuid());

    let response = test_app(pool.clone())
        .oneshot(auth_json_request(
            "POST",
            "/admin/organizations",
            json!({ "name": "Acme" }),
            &token,
        ))
        .await?;

    assert_eq!(response.status(), StatusCode::CREATED);

    let org_id = uuid::Uuid::parse_str(text_body(response).await.trim())?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM organizations WHERE id = $1")
        .bind(org_id)
        .fetch_one(&pool)
        .await?;

    assert_eq!(count, 1);

    Ok(())
}

#[sqlx::test(migrations = "infra/migrations")]
async fn projects_endpoint_creates_record_and_uses_default_shared_context(
    pool: PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let token = admin_token(study_auth::id::new_uuid());
    let org_id = insert_organization(&pool, "Acme").await;

    let response = test_app(pool.clone())
        .oneshot(auth_json_request(
            "POST",
            "/admin/projects",
            json!({
                "org_id": org_id,
                "name": "Project X"
            }),
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

    Ok(())
}

#[sqlx::test(migrations = "infra/migrations")]
async fn applications_endpoint_validates_redirect_uris(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let token = admin_token(study_auth::id::new_uuid());
    let org_id = insert_organization(&pool, "Acme").await;
    let project_id = insert_project(&pool, org_id, "Project X").await;

    let response = test_app(pool)
        .oneshot(auth_json_request(
            "POST",
            "/admin/applications",
            json!({
                "project_id": project_id,
                "redirect_uris": []
            }),
            &token,
        ))
        .await?;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = json_body(response).await;
    assert_eq!(body["errors"]["redirect_uris"][0], "empty");

    Ok(())
}

#[sqlx::test(migrations = "infra/migrations")]
async fn applications_endpoint_creates_application_and_stores_hashed_secret(
    pool: PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let token = admin_token(study_auth::id::new_uuid());
    let org_id = insert_organization(&pool, "Acme").await;
    let project_id = insert_project(&pool, org_id, "Project X").await;

    let response = test_app(pool.clone())
        .oneshot(auth_json_request(
            "POST",
            "/admin/applications",
            json!({
                "project_id": project_id,
                "redirect_uris": ["https://example.com/callback", "https://example.com/return"]
            }),
            &token,
        ))
        .await?;

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = json_body(response).await;
    let client_id = uuid::Uuid::parse_str(body["client_id"].as_str().unwrap())?;
    let raw_client_secret = body["raw_client_secret"].as_str().unwrap();
    let row =
        sqlx::query("SELECT project_id, client_secret_hash, redirect_uris FROM applications WHERE client_id = $1")
            .bind(client_id)
            .fetch_one(&pool)
            .await?;

    assert_eq!(row.get::<uuid::Uuid, _>("project_id"), project_id);
    assert_ne!(row.get::<String, _>("client_secret_hash"), raw_client_secret);
    assert_eq!(
        row.get::<Vec<String>, _>("redirect_uris"),
        vec![
            "https://example.com/callback".to_string(),
            "https://example.com/return".to_string()
        ]
    );

    Ok(())
}

#[sqlx::test(migrations = "infra/migrations")]
async fn application_scopes_endpoint_inserts_and_updates_permissions(
    pool: PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let token = admin_token(study_auth::id::new_uuid());
    let org_id = insert_organization(&pool, "Acme").await;
    let project_id = insert_project(&pool, org_id, "Project X").await;
    let application_id = insert_application(&pool, project_id).await;

    let first_response = test_app(pool.clone())
        .oneshot(auth_json_request(
            "PUT",
            &format!("/admin/applications/{application_id}/scopes"),
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
            &format!("/admin/applications/{application_id}/scopes"),
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

#[sqlx::test(migrations = "infra/migrations")]
async fn metrics_endpoint_accepts_valid_admin_token(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    init_test_env();
    let token = admin_token(study_auth::id::new_uuid());

    let response = test_app(pool)
        .oneshot(auth_request("GET", "/admin/metrics", &token))
        .await?;

    assert_eq!(response.status(), StatusCode::CREATED);

    Ok(())
}
