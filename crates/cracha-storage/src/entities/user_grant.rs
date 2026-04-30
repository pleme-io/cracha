// user_grants — per-user grant overrides on top of declarative
// AccessPolicy CRDs. An admin POSTing to /admin/grants creates one row
// here; the next /me call merges declarative + DB rows into a single
// service list for varanda.
//
// (user_id, service, verb) is UNIQUE: at most one row per (who, what,
// how). Re-grant overwrites; revoke deletes the row + writes audit.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "user_grants")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,

    pub user_id: Uuid,

    pub service: String,

    pub verb: String,

    pub granted_by: Option<Uuid>,

    pub granted_at: DateTime<Utc>,

    pub expires_at: Option<DateTime<Utc>>,

    pub note: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::Id",
        on_delete = "Cascade"
    )]
    User,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
