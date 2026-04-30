use sea_orm_migration::prelude::*;

use super::m20260430_000001_create_users::Users;

#[derive(DeriveIden)]
enum AuditLog {
    Table,
    Id,
    Ts,
    ActorUserId,
    ActorEmail,
    Action,
    TargetKind,
    TargetId,
    Details,
}

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260430_000003_create_audit_log"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AuditLog::Table)
                    .col(ColumnDef::new(AuditLog::Id).uuid().not_null().primary_key())
                    .col(
                        ColumnDef::new(AuditLog::Ts)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(AuditLog::ActorUserId).uuid())
                    .col(ColumnDef::new(AuditLog::ActorEmail).text().not_null())
                    .col(ColumnDef::new(AuditLog::Action).text().not_null())
                    .col(ColumnDef::new(AuditLog::TargetKind).text().not_null())
                    .col(ColumnDef::new(AuditLog::TargetId).text().not_null())
                    .col(ColumnDef::new(AuditLog::Details).json_binary())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_audit_log_actor_user_id")
                            .from(AuditLog::Table, AuditLog::ActorUserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        // Recent-events listing is the dominant query (admin UI tail
        // + alerting rules). Index ts DESC for cheap LIMIT N reads.
        manager
            .create_index(
                Index::create()
                    .name("idx_audit_log_ts_desc")
                    .table(AuditLog::Table)
                    .col(AuditLog::Ts)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_audit_log_actor")
                    .table(AuditLog::Table)
                    .col(AuditLog::ActorUserId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AuditLog::Table).to_owned())
            .await
    }
}
