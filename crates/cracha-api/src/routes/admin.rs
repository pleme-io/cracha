// Admin routes — gated by the `Role::Admin` check. The actor's
// google_sub arrives as a query parameter today (varanda → cracha
// over the *.quero.cloud session); a future refactor lifts this to
// a JWT-derived claim once vigia injects auth headers in front of
// cracha.

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use cracha_storage::{repo::AddGrant, AuditLogModel, UserGrantModel};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::role::{compute_role, Role};
use crate::state::ApiState;

#[derive(Debug, Deserialize)]
pub struct ActorQuery {
    /// Google sub of the caller — must resolve to a user with admin role.
    pub actor_sub: String,
}

#[derive(Debug, Deserialize)]
pub struct AddGrantRequest {
    /// User to grant TO. Required.
    pub user_id: Uuid,
    pub service: String,
    pub verb: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RevokeGrantRequest {
    pub user_id: Uuid,
    pub service: String,
    pub verb: String,
}

#[derive(Debug, Serialize)]
pub struct GrantResponse {
    pub grant: UserGrantModel,
}

#[derive(Debug, Serialize)]
pub struct AuditResponse {
    pub events: Vec<AuditLogModel>,
}

#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    pub actor_sub: String,
    /// Page size. Capped at 500 to prevent runaway responses.
    #[serde(default = "default_audit_limit")]
    pub limit: u64,
}

fn default_audit_limit() -> u64 {
    100
}

pub async fn add_grant(
    State(state): State<Arc<ApiState>>,
    Query(actor_q): Query<ActorQuery>,
    Json(req): Json<AddGrantRequest>,
) -> Result<Json<GrantResponse>, ApiError> {
    let actor = require_admin(&state, &actor_q.actor_sub).await?;

    let grant = state
        .repo
        .add_grant(AddGrant {
            user_id: req.user_id,
            service: req.service,
            verb: req.verb,
            granted_by: actor,
            expires_at: req.expires_at,
            note: req.note,
        })
        .await?;

    Ok(Json(GrantResponse { grant }))
}

pub async fn revoke_grant(
    State(state): State<Arc<ApiState>>,
    Query(actor_q): Query<ActorQuery>,
    Json(req): Json<RevokeGrantRequest>,
) -> Result<StatusCode, ApiError> {
    let actor = require_admin(&state, &actor_q.actor_sub).await?;

    let removed = state
        .repo
        .revoke_grant(req.user_id, &req.service, &req.verb, actor)
        .await?;

    if removed {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound("no matching grant".into()))
    }
}

pub async fn recent_audit(
    State(state): State<Arc<ApiState>>,
    Query(q): Query<AuditQuery>,
) -> Result<Json<AuditResponse>, ApiError> {
    let _actor = require_admin(&state, &q.actor_sub).await?;
    let limit = q.limit.min(500);
    let events = state.repo.recent_audit(limit).await?;
    Ok(Json(AuditResponse { events }))
}

/// Resolve the caller's user id and verify they hold the admin role.
/// Returns the user id (for granted_by) on success.
async fn require_admin(state: &Arc<ApiState>, sub: &str) -> Result<Uuid, ApiError> {
    let user = state
        .repo
        .find_by_google_sub(sub)
        .await?
        .ok_or_else(|| ApiError::Forbidden("not signed in".into()))?;
    if compute_role(&user.email, &state.admin_emails) != Role::Admin {
        return Err(ApiError::Forbidden("admin role required".into()));
    }
    Ok(user.id)
}
