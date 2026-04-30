// GET /me
//
// Authenticated endpoint. The auth middleware validates the JWT,
// lazy-upserts the user, and attaches AuthIdentity to extensions.
// This handler reads the identity, merges declarative AccessPolicy
// CRDs + DB-stored per-user grants, and returns the portal manifest
// varanda renders.

use std::sync::Arc;

use axum::{extract::State, Json};
use cracha_core::AccessibleService;
use serde::Serialize;

use crate::auth::AuthIdentity;
use crate::error::ApiError;
use crate::role::Role;
use crate::state::ApiState;

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub user_id: uuid::Uuid,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub role: Role,

    /// Merged services (declarative AccessPolicy CRDs + DB overrides).
    pub services: Vec<AccessibleService>,
}

pub async fn get_me(
    State(state): State<Arc<ApiState>>,
    identity: AuthIdentity,
) -> Result<Json<MeResponse>, ApiError> {
    // Declarative pass: ask the CRD-driven evaluator. Hot path —
    // happens on every page load.
    let mut services = {
        let idx = state.index.read().await;
        idx.accessible_services(&identity.sub)
    };

    // DB-override pass: enrich each per-user grant from the catalog.
    let db_grants = state.repo.list_user_grants(identity.user_id).await?;
    if !db_grants.is_empty() {
        let idx = state.index.read().await;
        for g in db_grants {
            if services.iter().any(|s| s.slug == g.service) {
                continue;
            }
            if let Some(enriched) = idx.lookup_service(&g.service) {
                services.push(enriched);
            }
        }
    }

    Ok(Json(MeResponse {
        user_id: identity.user_id,
        email: identity.email,
        display_name: identity.display_name,
        avatar_url: identity.avatar_url,
        role: identity.role,
        services,
    }))
}
