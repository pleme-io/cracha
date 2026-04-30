// Repo facade. cracha-api uses these methods exclusively — never
// imports SeaORM types directly. Keeps the routing layer free of
// query-builder noise and gives one chokepoint for "every state
// mutation also writes an audit_log row".

use chrono::{DateTime, Utc};
use sea_orm::{
    sea_query::OnConflict, ActiveModelTrait, ActiveValue, ColumnTrait, Condition,
    DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::{audit_log, user, user_grant, AuditLogModel, UserGrantModel, UserModel};
use crate::error::{Result, StorageError};

/// Pooled handle to the crachá registry. Cheap to clone (Arc internally).
#[derive(Clone)]
pub struct Repo {
    db: DatabaseConnection,
}

/// Input shape for `upsert_user`. Carries everything passaporte
/// learned from Google's id_token claims.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertUser {
    pub google_sub: String,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

/// Input shape for `add_grant`. expires_at=None means "indefinite".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddGrant {
    pub user_id: Uuid,
    pub service: String,
    pub verb: String,
    pub granted_by: Uuid,
    pub expires_at: Option<DateTime<Utc>>,
    pub note: Option<String>,
}

impl Repo {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Upsert a user from passaporte's first-/recurring-login call.
    /// Returns the post-write Model so the caller can include it in
    /// the response. New row → first_seen + last_seen are NOW; existing
    /// row → only last_seen + display_name + avatar_url update.
    ///
    /// Wraps the upsert + audit insert in one transaction so a
    /// "user_first_seen" event is never logged for a user that didn't
    /// land.
    pub async fn upsert_user(&self, input: UpsertUser) -> Result<(UserModel, bool)> {
        let txn = self.db.begin().await?;
        let now = Utc::now();

        let existing = user::Entity::find()
            .filter(user::Column::GoogleSub.eq(&input.google_sub))
            .one(&txn)
            .await?;

        let (model, was_new) = if let Some(row) = existing {
            // Update last_seen + freshen display_name/avatar (Google
            // may change those upstream).
            let mut active: user::ActiveModel = row.into();
            active.display_name = Set(input.display_name.clone());
            active.avatar_url = Set(input.avatar_url.clone());
            active.last_seen = Set(now);
            let updated = active.update(&txn).await?;
            (updated, false)
        } else {
            let id = Uuid::new_v4();
            let active = user::ActiveModel {
                id: Set(id),
                google_sub: Set(input.google_sub.clone()),
                email: Set(input.email.clone()),
                display_name: Set(input.display_name.clone()),
                avatar_url: Set(input.avatar_url.clone()),
                first_seen: Set(now),
                last_seen: Set(now),
            };
            let inserted = active.insert(&txn).await?;
            self.write_audit(
                &txn,
                Some(inserted.id),
                &inserted.email,
                "user_first_seen",
                "user",
                inserted.id.to_string(),
                None,
            )
            .await?;
            (inserted, true)
        };

        txn.commit().await?;
        Ok((model, was_new))
    }

    /// Look up a user by Google sub. Returns None if not yet upserted.
    pub async fn find_by_google_sub(&self, google_sub: &str) -> Result<Option<UserModel>> {
        Ok(user::Entity::find()
            .filter(user::Column::GoogleSub.eq(google_sub))
            .one(&self.db)
            .await?)
    }

    /// Look up a user by email. Case-insensitive (matches the lower(email)
    /// index). Used by the admin allowlist to map declared emails to ids.
    pub async fn find_by_email(&self, email: &str) -> Result<Option<UserModel>> {
        Ok(user::Entity::find()
            .filter(user::Column::Email.eq(email.to_lowercase()))
            .one(&self.db)
            .await?)
    }

    /// All grant rows for a user. Caller (cracha-api /me) merges these
    /// with the declarative AccessPolicy CRDs to compute the final
    /// service list.
    pub async fn list_user_grants(&self, user_id: Uuid) -> Result<Vec<UserGrantModel>> {
        Ok(user_grant::Entity::find()
            .filter(user_grant::Column::UserId.eq(user_id))
            .all(&self.db)
            .await?)
    }

    /// Add or update a per-user grant. Idempotent — re-grant updates
    /// granted_by/granted_at/note. Always writes an audit row.
    pub async fn add_grant(&self, input: AddGrant) -> Result<UserGrantModel> {
        let txn = self.db.begin().await?;
        let now = Utc::now();
        let id = Uuid::new_v4();

        let model = user_grant::ActiveModel {
            id: Set(id),
            user_id: Set(input.user_id),
            service: Set(input.service.clone()),
            verb: Set(input.verb.clone()),
            granted_by: Set(Some(input.granted_by)),
            granted_at: Set(now),
            expires_at: Set(input.expires_at),
            note: Set(input.note.clone()),
        };

        // ON CONFLICT (user_id, service, verb) DO UPDATE — re-grant
        // path. Updates granted_by/granted_at/expires_at/note on
        // collision.
        user_grant::Entity::insert(model)
            .on_conflict(
                OnConflict::columns([
                    user_grant::Column::UserId,
                    user_grant::Column::Service,
                    user_grant::Column::Verb,
                ])
                .update_columns([
                    user_grant::Column::GrantedBy,
                    user_grant::Column::GrantedAt,
                    user_grant::Column::ExpiresAt,
                    user_grant::Column::Note,
                ])
                .to_owned(),
            )
            .exec(&txn)
            .await?;

        let row = user_grant::Entity::find()
            .filter(
                Condition::all()
                    .add(user_grant::Column::UserId.eq(input.user_id))
                    .add(user_grant::Column::Service.eq(&input.service))
                    .add(user_grant::Column::Verb.eq(&input.verb)),
            )
            .one(&txn)
            .await?
            .ok_or_else(|| StorageError::Constraint("upsert returned no row".into()))?;

        let actor = self.lookup_email_for_audit(&txn, input.granted_by).await?;
        self.write_audit(
            &txn,
            Some(input.granted_by),
            &actor,
            "grant_added",
            "grant",
            format!("{}:{}:{}", input.user_id, input.service, input.verb),
            Some(serde_json::json!({
                "service": input.service,
                "verb": input.verb,
                "expires_at": input.expires_at,
                "note": input.note,
            })),
        )
        .await?;

        txn.commit().await?;
        Ok(row)
    }

    /// Revoke a per-user grant. Returns true if a row was removed.
    pub async fn revoke_grant(
        &self,
        user_id: Uuid,
        service: &str,
        verb: &str,
        actor: Uuid,
    ) -> Result<bool> {
        let txn = self.db.begin().await?;
        let res = user_grant::Entity::delete_many()
            .filter(
                Condition::all()
                    .add(user_grant::Column::UserId.eq(user_id))
                    .add(user_grant::Column::Service.eq(service))
                    .add(user_grant::Column::Verb.eq(verb)),
            )
            .exec(&txn)
            .await?;

        if res.rows_affected > 0 {
            let actor_email = self.lookup_email_for_audit(&txn, actor).await?;
            self.write_audit(
                &txn,
                Some(actor),
                &actor_email,
                "grant_revoked",
                "grant",
                format!("{user_id}:{service}:{verb}"),
                None,
            )
            .await?;
        }

        txn.commit().await?;
        Ok(res.rows_affected > 0)
    }

    /// Recent audit events, newest first. Bounded by `limit` to keep
    /// the admin tail UI snappy and the response small.
    pub async fn recent_audit(&self, limit: u64) -> Result<Vec<AuditLogModel>> {
        Ok(audit_log::Entity::find()
            .order_by_desc(audit_log::Column::Ts)
            .limit(limit)
            .all(&self.db)
            .await?)
    }

    // ── private helpers ─────────────────────────────────────────────

    async fn lookup_email_for_audit(
        &self,
        txn: &DatabaseTransaction,
        user_id: Uuid,
    ) -> Result<String> {
        Ok(user::Entity::find_by_id(user_id)
            .one(txn)
            .await?
            .map(|u| u.email)
            .unwrap_or_else(|| format!("<unknown:{user_id}>")))
    }

    #[allow(clippy::too_many_arguments)]
    async fn write_audit(
        &self,
        txn: &DatabaseTransaction,
        actor_user_id: Option<Uuid>,
        actor_email: &str,
        action: &str,
        target_kind: &str,
        target_id: impl Into<String>,
        details: Option<serde_json::Value>,
    ) -> Result<()> {
        let model = audit_log::ActiveModel {
            id: Set(Uuid::new_v4()),
            ts: Set(Utc::now()),
            actor_user_id: Set(actor_user_id),
            actor_email: Set(actor_email.to_owned()),
            action: Set(action.to_owned()),
            target_kind: Set(target_kind.to_owned()),
            target_id: Set(target_id.into()),
            details: Set(details),
        };
        audit_log::Entity::insert(model).exec(txn).await?;
        Ok(())
    }
}

// Silence clippy for ActiveValue::Set construction style — pattern is
// SeaORM-idiomatic and trying to avoid it just makes the code worse.
#[allow(unused_imports)]
use ActiveValue as _;
