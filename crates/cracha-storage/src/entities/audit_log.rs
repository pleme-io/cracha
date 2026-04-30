// audit_log — append-only record of every state mutation.
//
// Why denormalize actor_email alongside actor_user_id: users CAN be
// deleted (eventually). The audit trail must outlive deletion;
// keeping the email string ensures "who did this" is answerable
// without a join, even if the user row is gone.
//
// `details` is JSONB so action-specific shapes (e.g. { "service":
// "drive", "verb": "rw", "expires_at": ... } for a grant_added) can
// land without a schema change.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "audit_log")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,

    pub ts: DateTime<Utc>,

    pub actor_user_id: Option<Uuid>,

    /// Denormalized actor email for after-deletion accountability.
    pub actor_email: String,

    /// e.g. "user_first_seen", "grant_added", "grant_revoked".
    pub action: String,

    /// "user" | "grant" | "service".
    pub target_kind: String,

    /// Free-form id of the target. For grants: "<user_id>:<service>:<verb>".
    pub target_id: String,

    pub details: Option<serde_json::Value>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
