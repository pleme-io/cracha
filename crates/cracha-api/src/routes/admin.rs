// Admin routes — gated by the `RequireAdmin` extractor. Each handler
// authenticates via the auth middleware, then the extractor returns
// 403 unless the caller's role is Admin.

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

use crate::auth::RequireAdmin;
use crate::error::ApiError;
use crate::state::ApiState;

#[derive(Debug, Deserialize)]
pub struct AddGrantRequest {
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
    /// Page size. Capped at 500 to keep responses bounded.
    #[serde(default = "default_audit_limit")]
    pub limit: u64,
}

fn default_audit_limit() -> u64 {
    100
}

pub async fn add_grant(
    State(state): State<Arc<ApiState>>,
    RequireAdmin(actor): RequireAdmin,
    Json(req): Json<AddGrantRequest>,
) -> Result<Json<GrantResponse>, ApiError> {
    let grant = state
        .repo
        .add_grant(AddGrant {
            user_id: req.user_id,
            service: req.service,
            verb: req.verb,
            granted_by: actor.user_id,
            expires_at: req.expires_at,
            note: req.note,
        })
        .await?;
    Ok(Json(GrantResponse { grant }))
}

pub async fn revoke_grant(
    State(state): State<Arc<ApiState>>,
    RequireAdmin(actor): RequireAdmin,
    Json(req): Json<RevokeGrantRequest>,
) -> Result<StatusCode, ApiError> {
    let removed = state
        .repo
        .revoke_grant(req.user_id, &req.service, &req.verb, actor.user_id)
        .await?;
    if removed {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound("no matching grant".into()))
    }
}

pub async fn recent_audit(
    State(state): State<Arc<ApiState>>,
    RequireAdmin(_actor): RequireAdmin,
    Query(q): Query<AuditQuery>,
) -> Result<Json<AuditResponse>, ApiError> {
    let limit = q.limit.min(500);
    let events = state.repo.recent_audit(limit).await?;
    Ok(Json(AuditResponse { events }))
}
