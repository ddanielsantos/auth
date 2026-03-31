use crate::admin::authorization::AdminId;
use crate::error::{AppError, ValidationErrors};
use crate::router::AppState;
use crate::{id, admin::authorization::{OrgMember, ProjectMember}};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
pub struct CreateOrgInviteRequestBody {
    invitee_username: String,
    role: String,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateProjectInviteRequestBody {
    invitee_username: String,
    role: String,
}

#[derive(Serialize, ToSchema)]
pub struct InviteResponse {
    id: String,
}

fn validate_role(role: &str) -> Result<(), AppError> {
    if role != "owner" && role != "admin" {
        return Err(AppError::ValidationError(ValidationErrors::single_error(
            "role must be 'owner' or 'admin'".to_string(),
        )));
    }
    Ok(())
}

fn invite_expires_at() -> time::OffsetDateTime {
    time::OffsetDateTime::now_utc() + time::Duration::days(7)
}

#[utoipa::path(
    post,
    path = "/orgs/{org_id}/invites",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("org_id" = String, Path, description = "Organization ID (UUID v7)")
    ),
    request_body = CreateOrgInviteRequestBody,
    responses(
        (status = 201, description = "Invite created", body = InviteResponse),
        (status = 400, description = "Validation error"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    )
)]
pub async fn create_org_invite_handler(
    member: OrgMember,
    State(state): State<AppState>,
    Json(body): Json<CreateOrgInviteRequestBody>,
) -> Result<impl IntoResponse, AppError> {
    validate_role(&body.role)?;

    let invite_id = id::new_uuid();
    let expires_at = invite_expires_at();

    sqlx::query!(
        r#"
            INSERT INTO admin_invites (id, invited_by_admin_user_id, org_id, invitee_username, role, status, expires_at)
            VALUES ($1, $2, $3, $4, $5, 'pending', $6)
        "#,
        invite_id,
        member.admin_id,
        member.org_id,
        body.invitee_username,
        body.role,
        expires_at,
    )
    .execute(&state.pool)
    .await?;

    Ok((StatusCode::CREATED, Json(InviteResponse { id: invite_id.to_string() })))
}

#[utoipa::path(
    post,
    path = "/orgs/{org_id}/projects/{project_id}/invites",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("org_id" = String, Path, description = "Organization ID (UUID v7)"),
        ("project_id" = String, Path, description = "Project ID (UUID v7)")
    ),
    request_body = CreateProjectInviteRequestBody,
    responses(
        (status = 201, description = "Invite created", body = InviteResponse),
        (status = 400, description = "Validation error"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    )
)]
pub async fn create_project_invite_handler(
    member: ProjectMember,
    State(state): State<AppState>,
    Json(body): Json<CreateProjectInviteRequestBody>,
) -> Result<impl IntoResponse, AppError> {
    validate_role(&body.role)?;

    let invite_id = id::new_uuid();
    let expires_at = invite_expires_at();

    sqlx::query!(
        r#"
            INSERT INTO admin_invites (id, invited_by_admin_user_id, project_id, invitee_username, role, status, expires_at)
            VALUES ($1, $2, $3, $4, $5, 'pending', $6)
        "#,
        invite_id,
        member.admin_id,
        member.project_id,
        body.invitee_username,
        body.role,
        expires_at,
    )
    .execute(&state.pool)
    .await?;

    Ok((StatusCode::CREATED, Json(InviteResponse { id: invite_id.to_string() })))
}

#[utoipa::path(
    post,
    path = "/invites/{invite_id}/accept",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("invite_id" = String, Path, description = "Invite ID (UUID v7)")
    ),
    responses(
        (status = 204, description = "Invite accepted"),
        (status = 400, description = "Invite not pending or expired"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden — caller is not the invitee"),
        (status = 404, description = "Invite not found"),
    )
)]
pub async fn accept_invite_handler(
    AdminId { admin_id }: AdminId,
    Path(invite_id): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let invite_id = id::parse_uuid(&invite_id)?;

    let mut tx = state.pool.begin().await?;

    let invite = sqlx::query!(
        "SELECT id, org_id, project_id, invitee_username, role, status, expires_at FROM admin_invites WHERE id = $1",
        invite_id
    )
    .fetch_optional(&mut *tx)
    .await?;

    let Some(invite) = invite else {
        return Err(AppError::Sqlx(sqlx::Error::RowNotFound));
    };

    let caller_username: String = sqlx::query_scalar!(
        "SELECT username FROM admin_users WHERE id = $1",
        admin_id
    )
    .fetch_one(&mut *tx)
    .await?;

    if invite.invitee_username != caller_username {
        return Err(AppError::Forbidden);
    }

    if invite.status != "pending" {
        return Err(AppError::ValidationError(ValidationErrors::single_error(
            "Invite is not in pending status".to_string(),
        )));
    }

    if invite.expires_at < time::OffsetDateTime::now_utc() {
        return Err(AppError::ValidationError(ValidationErrors::single_error(
            "Invite has expired".to_string(),
        )));
    }

    let membership_id = id::new_uuid();
    if let Some(org_id) = invite.org_id {
        sqlx::query!(
            "INSERT INTO admin_org_memberships (id, admin_user_id, org_id, role) VALUES ($1, $2, $3, $4)",
            membership_id,
            admin_id,
            org_id,
            invite.role,
        )
        .execute(&mut *tx)
        .await?;
    } else if let Some(project_id) = invite.project_id {
        sqlx::query!(
            "INSERT INTO admin_project_memberships (id, admin_user_id, project_id, role) VALUES ($1, $2, $3, $4)",
            membership_id,
            admin_id,
            project_id,
            invite.role,
        )
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query!(
        "UPDATE admin_invites SET status = 'accepted', responded_at = NOW() WHERE id = $1",
        invite_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/invites/{invite_id}/decline",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("invite_id" = String, Path, description = "Invite ID (UUID v7)")
    ),
    responses(
        (status = 204, description = "Invite declined"),
        (status = 400, description = "Invite not pending"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden — caller is not the invitee"),
        (status = 404, description = "Invite not found"),
    )
)]
pub async fn decline_invite_handler(
    AdminId { admin_id }: AdminId,
    Path(invite_id): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let invite_id = id::parse_uuid(&invite_id)?;

    let invite = sqlx::query!(
        "SELECT invitee_username, status FROM admin_invites WHERE id = $1",
        invite_id
    )
    .fetch_optional(&state.pool)
    .await?;

    let Some(invite) = invite else {
        return Err(AppError::Sqlx(sqlx::Error::RowNotFound));
    };

    let caller_username: String =
        sqlx::query_scalar!("SELECT username FROM admin_users WHERE id = $1", admin_id)
            .fetch_one(&state.pool)
            .await?;

    if invite.invitee_username != caller_username {
        return Err(AppError::Forbidden);
    }

    if invite.status != "pending" {
        return Err(AppError::ValidationError(ValidationErrors::single_error(
            "Invite is not in pending status".to_string(),
        )));
    }

    sqlx::query!(
        "UPDATE admin_invites SET status = 'declined', responded_at = NOW() WHERE id = $1",
        invite_id
    )
    .execute(&state.pool)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/invites/{invite_id}/revoke",
    tag = "admin",
    security(("bearer_auth" = [])),
    params(
        ("invite_id" = String, Path, description = "Invite ID (UUID v7)")
    ),
    responses(
        (status = 204, description = "Invite revoked"),
        (status = 400, description = "Invite not pending"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden — caller is not the invite issuer"),
        (status = 404, description = "Invite not found"),
    )
)]
pub async fn revoke_invite_handler(
    AdminId { admin_id }: AdminId,
    Path(invite_id): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let invite_id = id::parse_uuid(&invite_id)?;

    let invite = sqlx::query!(
        "SELECT invited_by_admin_user_id, status FROM admin_invites WHERE id = $1",
        invite_id
    )
    .fetch_optional(&state.pool)
    .await?;

    let Some(invite) = invite else {
        return Err(AppError::Sqlx(sqlx::Error::RowNotFound));
    };

    if invite.invited_by_admin_user_id != admin_id {
        return Err(AppError::Forbidden);
    }

    if invite.status != "pending" {
        return Err(AppError::ValidationError(ValidationErrors::single_error(
            "Invite is not in pending status".to_string(),
        )));
    }

    sqlx::query!(
        "UPDATE admin_invites SET status = 'revoked' WHERE id = $1",
        invite_id
    )
    .execute(&state.pool)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}
