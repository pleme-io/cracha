// GET /me?sub=<google_sub>
//
// varanda calls this immediately after a successful login (cookie
// dropped by passaporte) to render the portal manifest: which
// services this user can see, grouped by location/cluster per the
// saguão hostname pattern.
//
// The returned service list is the UNION of:
//   - declarative grants from cracha-controller's CRD-driven index
//     (CRDs: AccessPolicy, AccessGroup, ServiceCatalog) — already
//     evaluated by `index.accessible_services(sub)`
//   - DB-stored per-user grants from `user_grants`, projected through
//     the service catalog so we get hostname/display_name/etc.
//     (a DB grant for a slug that isn't in the catalog is dropped on
//     the floor — admin must add the service to the declarative
//     catalog first.)
//
// Role flag carries the admin/user split so varanda can show or hide
// the admin panel.

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};
use cracha_core::AccessibleService;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::role::{compute_role, Role};
use crate::state::ApiState;

#[derive(Debug, Deserialize)]
pub struct MeQuery {
    /// Google `sub` claim — the stable account id.
    pub sub: String,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub user_id: uuid::Uuid,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub role: Role,

    /// Merged services (declarative + DB overrides).
    pub services: Vec<AccessibleService>,
}

pub async fn get_me(
    State(state): State<Arc<ApiState>>,
    Query(q): Query<MeQuery>,
) -> Result<Json<MeResponse>, ApiError> {
    let user = state
        .repo
        .find_by_google_sub(&q.sub)
        .await?
        .ok_or_else(|| ApiError::NotFound("user not found — log in first".into()))?;

    let role = compute_role(&user.email, &state.admin_emails);

    // Declarative pass: ask the CRD-driven evaluator. Hot path —
    // happens on every page load.
    let mut services = {
        let idx = state.index.read().await;
        idx.accessible_services(&q.sub)
    };

    // DB-override pass: for each per-user grant, look up the
    // corresponding service entry in the catalog so the response row
    // carries hostname/cluster/location. Skip any DB grant whose slug
    // is not in the catalog (declared inconsistency).
    let db_grants = state.repo.list_user_grants(user.id).await?;
    if !db_grants.is_empty() {
        let idx = state.index.read().await;
        for g in db_grants {
            // Skip if already in the declarative result.
            if services.iter().any(|s| s.slug == g.service) {
                continue;
            }
            // Look up the catalog entry to enrich (hostname etc.).
            if let Some(enriched) = idx.lookup_service(&g.service) {
                services.push(enriched);
            }
        }
    }

    Ok(Json(MeResponse {
        user_id: user.id,
        email: user.email,
        display_name: user.display_name,
        avatar_url: user.avatar_url,
        role,
        services,
    }))
}
