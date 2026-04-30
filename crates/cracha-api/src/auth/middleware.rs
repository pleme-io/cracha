// Tower-layer authentication middleware.
//
// Lifecycle per request:
//   1. Pull JWT from `X-Saguao-Session` cookie OR `Authorization: Bearer`.
//   2. Validate against passaporte's JWKS (cached, kid-keyed).
//   3. Lazy-upsert a `users` row from the claims (idempotent).
//   4. Compute role from the shikumi-loaded admin allowlist.
//   5. Attach `AuthIdentity` to request extensions.
//   6. Forward to the handler.
//
// Failure modes:
//   - No token → 401 with WWW-Authenticate hint.
//   - Invalid signature / expired → 401.
//   - Storage error during upsert → 500.
//
// `RequireAdmin` is a separate extractor that returns 403 unless
// the resolved identity carries `role = admin`.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::FromRequestParts,
    http::{header, request::Parts, HeaderMap, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use cracha_storage::repo::UpsertUser;

use crate::role::{compute_role, Role};
use crate::state::ApiState;

use super::PassaporteClaims;

/// Identity carried on every authenticated request.
#[derive(Debug, Clone)]
pub struct AuthIdentity {
    pub user_id: uuid::Uuid,
    pub sub: String,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub role: Role,
}

const COOKIE_NAME: &str = "X-Saguao-Session";

/// Axum middleware: validate JWT, upsert user, attach identity.
pub async fn auth_middleware(
    state_ext: axum::extract::State<Arc<ApiState>>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let state = state_ext.0.clone();
    let token = match extract_jwt(req.headers()) {
        Some(t) => t,
        None => return unauthorized("missing session token"),
    };

    let claims = match validate(&state, &token).await {
        Ok(c) => c,
        Err(e) => {
            tracing::info!(error = %e, "JWT validation failed");
            return unauthorized("invalid session token");
        }
    };

    let upsert = UpsertUser {
        google_sub: claims.sub.clone(),
        email: claims.email.to_lowercase(),
        display_name: claims.display_name(),
        avatar_url: claims.picture.clone(),
    };
    let (user, _was_new) = match state.repo.upsert_user(upsert).await {
        Ok(u) => u,
        Err(e) => {
            tracing::error!(error = %e, "user upsert failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };

    let role = compute_role(&user.email, &state.admin_emails);
    let identity = AuthIdentity {
        user_id: user.id,
        sub: claims.sub,
        email: user.email,
        display_name: user.display_name,
        avatar_url: user.avatar_url,
        role,
    };
    req.extensions_mut().insert(identity);

    next.run(req).await
}

/// Axum extractor — handler signatures pull `AuthIdentity` directly
/// from request extensions.
impl<S> FromRequestParts<S> for AuthIdentity
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthIdentity>()
            .cloned()
            .ok_or_else(|| unauthorized("auth middleware did not run"))
    }
}

/// Extractor that returns 403 unless the caller is admin.
pub struct RequireAdmin(pub AuthIdentity);

impl<S> FromRequestParts<S> for RequireAdmin
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let id = AuthIdentity::from_request_parts(parts, state).await?;
        if id.role != Role::Admin {
            return Err((StatusCode::FORBIDDEN, "admin role required").into_response());
        }
        Ok(RequireAdmin(id))
    }
}

// ── helpers ─────────────────────────────────────────────────────────

fn extract_jwt(headers: &HeaderMap) -> Option<String> {
    if let Some(auth) = headers.get(header::AUTHORIZATION).and_then(|h| h.to_str().ok()) {
        if let Some(stripped) = auth.strip_prefix("Bearer ") {
            return Some(stripped.to_owned());
        }
    }
    let cookies = headers.get(header::COOKIE)?.to_str().ok()?;
    for c in cookies.split(';') {
        let c = c.trim();
        if let Some(stripped) = c.strip_prefix(&format!("{COOKIE_NAME}=")) {
            return Some(stripped.to_owned());
        }
    }
    None
}

async fn validate(state: &Arc<ApiState>, token: &str) -> anyhow::Result<PassaporteClaims> {
    let jwks = state
        .jwks
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("JWKS cache not configured"))?;

    let header = jsonwebtoken::decode_header(token)?;
    let kid = header
        .kid
        .ok_or_else(|| anyhow::anyhow!("JWT has no kid header"))?;
    let key = jwks.key_for(&kid).await?;

    let mut validation = jsonwebtoken::Validation::new(header.alg);
    if let Some(aud) = &state.audience {
        validation.set_audience(&[aud.as_str()]);
    } else {
        validation.validate_aud = false;
    }

    let data = jsonwebtoken::decode::<PassaporteClaims>(token, &key, &validation)?;
    Ok(data.claims)
}

fn unauthorized(msg: &'static str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, "Bearer")],
        msg,
    )
        .into_response()
}
