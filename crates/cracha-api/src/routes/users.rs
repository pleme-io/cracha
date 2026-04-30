// POST /users/upsert
//
// passaporte (Authentik wrapper) calls this on every successful login.
// Body carries the Google id_token claims after passaporte has
// validated them. cracha treats the call as authoritative — there's no
// re-verification at this layer; passaporte's mTLS + signed-JWT pair
// is the trust boundary.
//
// Idempotent: existing users get last_seen + display_name + avatar_url
// refreshed; new users get a row + a "user_first_seen" audit entry.
//
// Returns the canonical user view (id, role, services). Role is
// computed each call from the shikumi-loaded admin allowlist —
// promotion/demotion happens entirely declaratively on the next
// allowlist edit, no DB migration needed.

use std::sync::Arc;

use axum::{extract::State, Json};
use cracha_storage::repo::UpsertUser;
use serde::{Deserialize, Serialize};

use crate::state::ApiState;
use crate::{error::ApiError, role::compute_role, role::Role};

#[derive(Debug, Deserialize)]
pub struct UpsertRequest {
    pub google_sub: String,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UpsertResponse {
    pub user_id: uuid::Uuid,
    pub email: String,
    pub display_name: String,
    pub role: Role,
    pub was_new: bool,
}

pub async fn upsert(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<UpsertRequest>,
) -> Result<Json<UpsertResponse>, ApiError> {
    let (user, was_new) = state
        .repo
        .upsert_user(UpsertUser {
            google_sub: req.google_sub,
            email: req.email.to_lowercase(),
            display_name: req.display_name,
            avatar_url: req.avatar_url,
        })
        .await?;

    let role = compute_role(&user.email, &state.admin_emails);

    Ok(Json(UpsertResponse {
        user_id: user.id,
        email: user.email,
        display_name: user.display_name,
        role,
        was_new,
    }))
}
