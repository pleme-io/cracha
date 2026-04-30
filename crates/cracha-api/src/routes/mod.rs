// Route modules for cracha-api's REST surface.
//
// `accessible.rs` is the legacy CRD-only `/accessible-services?user=…`
// shim; it stays for vigia compatibility. The new auth-flow surfaces
// (admin allowlist + DB persistence) live in their own modules:
//
//   POST /users/upsert    — passaporte → cracha on every login
//   GET  /me              — varanda → cracha for the portal manifest
//   POST /admin/grants    — admin UI → cracha to add a per-user grant
//   DEL  /admin/grants    — admin UI → cracha to revoke
//   GET  /admin/audit     — admin UI tail of recent events
//
// Each module defines its own request/response shapes and handlers;
// `wire()` in this module assembles them into one Router consumed by
// `main.rs`.

use axum::{routing::{delete, get, post}, Router};
use std::sync::Arc;

pub mod admin;
pub mod me;
pub mod users;

use crate::state::ApiState;

pub fn wire(state: Arc<ApiState>) -> Router {
    Router::new()
        .route("/users/upsert", post(users::upsert))
        .route("/me", get(me::get_me))
        .route("/admin/grants", post(admin::add_grant))
        .route("/admin/grants", delete(admin::revoke_grant))
        .route("/admin/audit", get(admin::recent_audit))
        .with_state(state)
}
