// crachá-storage — Postgres-backed identity registry.
//
// What lives here (DB-backed, mutable at runtime):
//   - `users` rows: created on first login via passaporte's /users/upsert
//     call. Carries google_sub, email, display_name, first/last seen.
//   - `user_grants` rows: per-user grant overrides on top of the
//     declarative `AccessPolicy` CRDs reconciled by cracha-controller.
//     Admin mutations land here via /admin/grants.
//   - `audit_log` rows: append-only record of every admin mutation +
//     first-login event. Drives the operator's accountability trail.
//
// What does NOT live here (stays declarative in git, reconciled by
// cracha-controller from CRDs):
//   - The service catalog (which services exist, where they live).
//   - Default group → service grants (e.g. "family → drive,photos").
//   - The admin allowlist (carried by shikumi config in cracha-api).
//
// The merged decision (declarative + DB) is computed in cracha-api's
// /me handler; this crate only owns the persistence half.

#![allow(clippy::module_name_repetitions)]

pub mod entities;
pub mod error;
pub mod migration;
pub mod repo;

pub use entities::{AuditLogModel, UserGrantModel, UserModel};
pub use error::{Result, StorageError};
pub use repo::Repo;

use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use std::time::Duration;
use tracing::info;

/// Open a pooled Postgres connection from a libpq-style URL.
///
/// Sensible defaults for a homelab IdP: small pool, generous timeouts.
/// All knobs configurable via `Connect::with_*` if a caller wants finer
/// control later.
///
/// # Errors
/// Returns [`StorageError::Database`] if the URL is invalid or the
/// initial handshake fails.
pub async fn connect(url: &str) -> Result<DatabaseConnection> {
    let mut opt = ConnectOptions::new(url.to_owned());
    opt.max_connections(8)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(10))
        .acquire_timeout(Duration::from_secs(10))
        .idle_timeout(Duration::from_secs(60))
        .max_lifetime(Duration::from_secs(600))
        .sqlx_logging(false);
    info!(url = redact_url(url).as_str(), "connecting to crachá db");
    let db = Database::connect(opt).await?;
    Ok(db)
}

/// Run pending migrations to bring the DB to current schema. Idempotent
/// — safe to call on every cracha-api startup.
///
/// # Errors
/// Returns [`StorageError::Migration`] if any migration fails to apply.
pub async fn migrate(db: &DatabaseConnection) -> Result<()> {
    use sea_orm_migration::MigratorTrait;
    info!("running crachá db migrations");
    migration::Migrator::up(db, None).await?;
    Ok(())
}

/// Strip the `password@` middle of a Postgres URL so the connection
/// string can be safely logged. Leaves scheme/host/db visible.
fn redact_url(url: &str) -> String {
    if let Some(scheme_end) = url.find("://") {
        let prefix = &url[..scheme_end + 3];
        let rest = &url[scheme_end + 3..];
        if let Some(at) = rest.find('@') {
            return format!("{prefix}***@{}", &rest[at + 1..]);
        }
    }
    url.to_owned()
}
