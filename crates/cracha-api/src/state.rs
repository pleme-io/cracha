// Shared cracha-api state — passed via axum's `State<Arc<ApiState>>`
// extractor. Holds the SharedIndex (CRD-driven decision tree) + the
// Repo (DB-backed identity registry) + the shikumi-loaded admin
// allowlist.

use cracha_controller::SharedIndex;
use cracha_storage::Repo;
use std::collections::HashSet;

use crate::auth::jwks::JwksCache;

/// Application-wide state. Cheap to clone (Arc inside Repo and
/// SharedIndex; allowlist is owned but small).
pub struct ApiState {
    /// Decision index built from AccessPolicy CRDs by cracha-controller.
    /// Read-only from cracha-api's perspective.
    pub index: SharedIndex,

    /// DB repo — user upserts, grant CRUD, audit log.
    pub repo: Repo,

    /// Admin allowlist loaded from shikumi config at startup. Lowercase
    /// emails. A user whose login email matches any entry gets the
    /// admin role on first login (and on every subsequent /me lookup).
    /// This stays declarative (git-controlled) by design — admin is
    /// not a runtime-mutable property; promote/demote = edit shikumi.
    pub admin_emails: HashSet<String>,

    /// JWKS cache for passaporte-issued JWTs. None when cracha-api
    /// runs in test/dev mode without a passaporte instance — the
    /// auth middleware will short-circuit with 401.
    pub jwks: Option<JwksCache>,

    /// Expected `aud` claim value. None disables audience checking
    /// (useful in dev). Production: set to the cracha OIDC client_id
    /// configured in Authentik's blueprint.
    pub audience: Option<String>,
}
