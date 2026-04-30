// Authentication for cracha-api's REST surface.
//
// What this module owns:
//   - JWKS fetching from passaporte (Authentik) — cached 1h, refreshed
//     lazily on validation cache miss.
//   - JWT validation (cookie or Authorization: Bearer) against the
//     cached JWKS.
//   - Lazy user upsert: every successful validation hits Repo::upsert_user
//     so cracha sees a `users` row created on a user's first
//     authenticated request, no Authentik post-login hook required.
//   - Tower-layer middleware that injects an `AuthIdentity` into
//     request extensions for downstream handlers.
//
// Routes consume `AuthIdentity` via a custom Axum extractor; the
// middleware short-circuits with 401 when the token is missing or
// invalid.

pub mod jwks;
pub mod middleware;

pub use middleware::{AuthIdentity, RequireAdmin};

use crate::role::Role;
use serde::{Deserialize, Serialize};

/// Claims cracha cares about from a passaporte-issued JWT. Authentik
/// emits these in its OIDC id_token; we ignore the rest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassaporteClaims {
    /// Stable Authentik-side user id. Stored in the `users.google_sub`
    /// column for legacy reasons (saguão's design started Google-first;
    /// the column name predates Authentik's involvement).
    pub sub: String,
    pub email: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub picture: Option<String>,
    pub exp: i64,
}

impl PassaporteClaims {
    /// Best-effort display name. Falls back to the email's local part
    /// when Authentik didn't include `name` in the token.
    #[must_use]
    pub fn display_name(&self) -> String {
        self.name.clone().unwrap_or_else(|| {
            self.email
                .split_once('@')
                .map_or_else(|| self.email.clone(), |(local, _)| local.to_owned())
        })
    }
}

/// Resolved identity attached to every authenticated request.
#[derive(Debug, Clone)]
pub struct ResolvedIdentity {
    pub user_id: uuid::Uuid,
    pub email: String,
    pub display_name: String,
    pub role: Role,
}
