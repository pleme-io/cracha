// JWKS cache — fetches passaporte (Authentik)'s signing keys and
// hands back a DecodingKey by `kid` for jsonwebtoken validation.
//
// Mirrors vigia/src/jwks.rs in shape so the two services validate
// the same way. Refresh strategy: lazy on cache miss + a 1h TTL
// covers the common case (Authentik rotates JWKS rarely; the kid
// header of every JWT names which key was used so a fresh fetch
// resolves any rotation gap).

use anyhow::{anyhow, Context, Result};
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::DecodingKey;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct JwksCache {
    inner: Arc<RwLock<Inner>>,
    jwks_url: String,
    ttl: Duration,
}

struct Inner {
    jwks: Option<JwkSet>,
    fetched_at: Option<Instant>,
}

impl JwksCache {
    #[must_use]
    pub fn new(jwks_url: String) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner {
                jwks: None,
                fetched_at: None,
            })),
            jwks_url,
            ttl: Duration::from_secs(3600),
        }
    }

    /// Resolve `kid` to a DecodingKey. Refreshes the cache if it's
    /// empty, expired, or doesn't contain the requested kid.
    pub async fn key_for(&self, kid: &str) -> Result<DecodingKey> {
        if let Some(k) = self.lookup(kid).await? {
            return Ok(k);
        }
        // Miss — refresh and try again.
        self.refresh().await?;
        self.lookup(kid)
            .await?
            .ok_or_else(|| anyhow!("kid {} not in JWKS after refresh", kid))
    }

    async fn lookup(&self, kid: &str) -> Result<Option<DecodingKey>> {
        let guard = self.inner.read().await;
        let Some(jwks) = &guard.jwks else {
            return Ok(None);
        };
        let Some(fetched) = guard.fetched_at else {
            return Ok(None);
        };
        if fetched.elapsed() > self.ttl {
            return Ok(None);
        }
        let Some(jwk) = jwks.find(kid) else {
            return Ok(None);
        };
        Ok(Some(DecodingKey::from_jwk(jwk)?))
    }

    async fn refresh(&self) -> Result<()> {
        let jwks = fetch(&self.jwks_url).await?;
        let mut guard = self.inner.write().await;
        guard.jwks = Some(jwks);
        guard.fetched_at = Some(Instant::now());
        Ok(())
    }
}

async fn fetch(url: &str) -> Result<JwkSet> {
    let resp = reqwest::Client::new()
        .get(url)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()?;
    let jwks: JwkSet = resp.json().await.context("parse JWKS")?;
    Ok(jwks)
}
