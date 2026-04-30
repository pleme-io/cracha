// Route modules for cracha-api's REST surface.
//
// `accessible.rs` is the legacy CRD-only `/accessible-services?user=…`
// shim; it stays for vigia compatibility (gRPC also offers Authorize).
//
// New auth-flow surfaces (every one runs through the JWT middleware
// at `auth::middleware::auth_middleware`):
//
//   GET  /me              — varanda → cracha for the portal manifest
//   POST /admin/grants    — admin UI → cracha to add a per-user grant
//   DEL  /admin/grants    — admin UI → cracha to revoke
//   GET  /admin/audit     — admin UI tail of recent events
//
// User upserts happen lazily inside the middleware on every
// authenticated request — no explicit /users/upsert endpoint.

use axum::{
    middleware as axum_middleware,
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;

pub mod admin;
pub mod me;

use crate::auth::middleware::auth_middleware;
use crate::state::ApiState;

/// Build the authenticated subrouter — all routes here require a
/// valid passaporte JWT (cookie or bearer).
pub fn wire(state: Arc<ApiState>) -> Router {
    Router::new()
        .route("/me", get(me::get_me))
        .route("/admin/grants", post(admin::add_grant))
        .route("/admin/grants", delete(admin::revoke_grant))
        .route("/admin/audit", get(admin::recent_audit))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state)
}
