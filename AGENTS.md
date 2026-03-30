# AGENTS Guide

## Big Picture
- This is a single Rust service (`study-auth`) exposing two route groups: `/admin` and `/auth`.
- Startup flow is in `src/main.rs`: tracing + in-memory rate limiter init -> Postgres pool -> app router + layers -> bind `0.0.0.0:3000`.
- Route composition is centralized in `src/router.rs`; OpenAPI is generated via `utoipa_axum` and served at `/docs`.
- Cross-cutting layers (trace, CORS, real-ip/governor) are added in `src/main.rs`, not per-module.
- `AppState` currently only carries `sqlx::Pool<Postgres>`.

## Service Boundaries and Data Flow
- `src/admin/router.rs` handles admin CRUD-like setup flows (organizations, projects, applications, scopes, metrics stub).
- `src/admin/router/auth.rs` handles admin account auth (`/admin/register`, `/admin/login`) and issues admin JWTs.
- `src/auth/router.rs` handles end-user registration and `/auth/me`; registration is a DB transaction across `identities` -> `login_methods` -> `user_accounts`.
- Admin-protected routes use middleware from `src/admin.rs` (`validate_admin_api_key_middleware`) which decodes JWT and enforces `claims.user_type == "admin"`.
- JWT helpers live in `src/jwt.rs`; secrets/durations are split between admin vs user tokens.

## Project-Specific Conventions
- IDs are UUIDv7 by policy: parse external IDs with `id::parse_uuid` and generate with `id::new_uuid` (`src/id.rs`).
- Error-to-HTTP mapping is centralized in `AppError` (`src/error.rs`); prefer returning `Result<impl IntoResponse, AppError>` in handlers.
- SQL uses `sqlx::query!`/`query_scalar!` macros; keep queries compile-checked and typed.
- Validation style is mixed today: `validator::Validate` in admin auth, manual `HashMap` validation payloads in some admin endpoints.
- API docs are declaration-driven (`#[utoipa::path]` + `routes!(...)`); keep route registration and handler annotations together.

## Developer Workflows
- Local DB is defined in `infra/compose.yml` (Postgres `auth_user/auth_pass`, db `auth_db`, port `5432`).
- Required env vars are enforced in `src/config/env.rs`; missing vars panic early. Use `.env` as baseline values.
- Common local commands:
  - `docker compose -f infra/compose.yml up -d db`
  - `cargo run`
  - `cargo test`
  - `cargo test --test admin_router_integration_tests`
- Integration tests use `#[sqlx::test(migrations = "infra/migrations")]` and seed data helpers in `tests/admin_router_integration_tests.rs`.
- Container build uses offline SQLx metadata (`SQLX_OFFLINE=true` in `infra/Dockerfile`) backed by checked-in `.sqlx/` files.

## Integration Points
- Primary dependency is PostgreSQL + migrations in `infra/migrations/`.
- Schema intent is documented in `docs/ERD.md`; table relationships map directly to registration/login flows.
- OpenAPI security scheme (`bearer_auth`) is configured in `src/openapi.rs` and referenced in endpoint annotations.
- Rate-limit rules are configured in `config::net::init_rate_limiting`; update this list when adding high-traffic auth/admin routes.

## Safe Change Checklist for Agents
- When adding/changing an endpoint, update router registration and add/adjust a `#[utoipa::path]` annotation in the same file.
- For new DB writes, prefer transactions when multiple tables must stay consistent (see `register_handler` in `src/auth/router.rs`).
- Keep secrets one-way: never return stored hashes (`password_hash`, `client_secret_hash`) in responses.
- Add/extend `sqlx::test` integration coverage for behavior changes in admin/auth flows.
