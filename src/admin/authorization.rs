use crate::error::AppError;
use crate::router::AppState;
use axum::extract::{FromRequestParts, Path};
use axum::http::request::Parts;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Owner,
    Admin,
}

impl Role {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "owner" => Some(Role::Owner),
            "admin" => Some(Role::Admin),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Role::Owner => "owner",
            Role::Admin => "admin",
        }
    }
}

/// Extractor that reads the authenticated admin's ID from request extensions.
/// The middleware must run first to inject the ID.
pub struct AdminId {
    pub admin_id: Uuid,
}

/// Extractor that verifies the caller is a member of the org in the URL path.
pub struct OrgMember {
    pub admin_id: Uuid,
    pub org_id: Uuid,
    pub role: Role,
}

/// Extractor that verifies the caller has access to the project in the URL path.
/// Checks direct project membership first; falls back to org membership for org owners.
pub struct ProjectMember {
    pub admin_id: Uuid,
    pub project_id: Uuid,
    pub org_id: Uuid,
    pub role: Role,
}

fn admin_id_from_parts(parts: &Parts) -> Result<Uuid, AppError> {
    parts
        .extensions
        .get::<Uuid>()
        .copied()
        .ok_or(AppError::InvalidToken)
}

async fn path_params(parts: &mut Parts, state: &AppState) -> Result<HashMap<String, String>, AppError> {
    Path::<HashMap<String, String>>::from_request_parts(parts, state)
        .await
        .map(|p| p.0)
        .map_err(|_| AppError::InvalidToken)
}

impl FromRequestParts<AppState> for AdminId {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &AppState) -> Result<Self, Self::Rejection> {
        Ok(AdminId {
            admin_id: admin_id_from_parts(parts)?,
        })
    }
}

impl FromRequestParts<AppState> for OrgMember {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let admin_id = admin_id_from_parts(parts)?;
        let params = path_params(parts, state).await?;

        let org_id_str = params.get("org_id").ok_or(AppError::InvalidToken)?;
        let org_id = crate::id::parse_uuid(org_id_str)?;

        let membership = sqlx::query!(
            "SELECT role FROM admin_org_memberships WHERE admin_user_id = $1 AND org_id = $2",
            admin_id,
            org_id
        )
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::Sqlx)?;

        let Some(m) = membership else {
            return Err(AppError::Forbidden);
        };

        let role = Role::from_str(&m.role).ok_or(AppError::InvalidToken)?;
        Ok(OrgMember { admin_id, org_id, role })
    }
}

impl FromRequestParts<AppState> for ProjectMember {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let admin_id = admin_id_from_parts(parts)?;
        let params = path_params(parts, state).await?;

        let org_id_str = params.get("org_id").ok_or(AppError::InvalidToken)?;
        let org_id = crate::id::parse_uuid(org_id_str)?;

        let project_id_str = params.get("project_id").ok_or(AppError::InvalidToken)?;
        let project_id = crate::id::parse_uuid(project_id_str)?;

        // Verify the project belongs to the specified org.
        let in_org: bool = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id = $1 AND org_id = $2)",
            project_id,
            org_id
        )
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::Sqlx)?
        .unwrap_or(false);

        if !in_org {
            return Err(AppError::Sqlx(sqlx::Error::RowNotFound));
        }

        // Check direct project membership first.
        let project_m = sqlx::query!(
            "SELECT role FROM admin_project_memberships WHERE admin_user_id = $1 AND project_id = $2",
            admin_id,
            project_id
        )
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::Sqlx)?;

        if let Some(m) = project_m {
            let role = Role::from_str(&m.role).ok_or(AppError::InvalidToken)?;
            return Ok(ProjectMember { admin_id, project_id, org_id, role });
        }

        // Fall back: only org owners inherit all project access.
        let org_m = sqlx::query!(
            "SELECT role FROM admin_org_memberships WHERE admin_user_id = $1 AND org_id = $2",
            admin_id,
            org_id
        )
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::Sqlx)?;

        match org_m {
            Some(m) if m.role == "owner" => Ok(ProjectMember {
                admin_id,
                project_id,
                org_id,
                role: Role::Owner,
            }),
            _ => Err(AppError::Forbidden),
        }
    }
}
