// users — identity registry rows. One per Google account that has ever
// signed in via passaporte.
//
// Invariants the schema enforces:
//   - google_sub UNIQUE: one row per Google account
//   - email UNIQUE: shouldn't happen that two Google subs share an
//     email, but the constraint catches edge cases (acct deletion +
//     re-create with same email, etc.)
//   - timestamps are tz-aware (chrono::DateTime<Utc>)

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,

    #[sea_orm(unique, indexed)]
    pub google_sub: String,

    #[sea_orm(unique)]
    pub email: String,

    pub display_name: String,

    pub avatar_url: Option<String>,

    pub first_seen: DateTime<Utc>,

    pub last_seen: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::user_grant::Entity")]
    UserGrants,
}

impl Related<super::user_grant::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserGrants.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
