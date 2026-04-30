use sea_orm_migration::prelude::*;

use super::m20260430_000001_create_users::Users;

#[derive(DeriveIden)]
pub(super) enum UserGrants {
    Table,
    Id,
    UserId,
    Service,
    Verb,
    GrantedBy,
    GrantedAt,
    ExpiresAt,
    Note,
}

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260430_000002_create_user_grants"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UserGrants::Table)
                    .col(
                        ColumnDef::new(UserGrants::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(UserGrants::UserId).uuid().not_null())
                    .col(ColumnDef::new(UserGrants::Service).text().not_null())
                    .col(ColumnDef::new(UserGrants::Verb).text().not_null())
                    .col(ColumnDef::new(UserGrants::GrantedBy).uuid())
                    .col(
                        ColumnDef::new(UserGrants::GrantedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserGrants::ExpiresAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(ColumnDef::new(UserGrants::Note).text())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_grants_user_id")
                            .from(UserGrants::Table, UserGrants::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_grants_granted_by")
                            .from(UserGrants::Table, UserGrants::GrantedBy)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        // (user_id, service, verb) UNIQUE — at most one row per triple.
        // Re-grant updates in place; revoke deletes.
        manager
            .create_index(
                Index::create()
                    .name("uniq_user_grants_user_service_verb")
                    .table(UserGrants::Table)
                    .col(UserGrants::UserId)
                    .col(UserGrants::Service)
                    .col(UserGrants::Verb)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_user_grants_service")
                    .table(UserGrants::Table)
                    .col(UserGrants::Service)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UserGrants::Table).to_owned())
            .await
    }
}
