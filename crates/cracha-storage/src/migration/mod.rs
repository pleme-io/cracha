// sea-orm-migration entry — single Migrator that reports the ordered
// list of migrations. Add new ones to the bottom; never reorder or
// rename, since the timestamp prefix is what sea-orm-migration tracks
// in the `seaql_migrations` table.

use sea_orm_migration::prelude::*;

mod m20260430_000001_create_users;
mod m20260430_000002_create_user_grants;
mod m20260430_000003_create_audit_log;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260430_000001_create_users::Migration),
            Box::new(m20260430_000002_create_user_grants::Migration),
            Box::new(m20260430_000003_create_audit_log::Migration),
        ]
    }
}
