use sea_orm_migration::prelude::*;

#[derive(DeriveIden)]
pub(super) enum Users {
    Table,
    Id,
    GoogleSub,
    Email,
    DisplayName,
    AvatarUrl,
    FirstSeen,
    LastSeen,
}

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260430_000001_create_users"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .col(ColumnDef::new(Users::Id).uuid().not_null().primary_key())
                    .col(
                        ColumnDef::new(Users::GoogleSub)
                            .text()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Users::Email).text().not_null().unique_key())
                    .col(ColumnDef::new(Users::DisplayName).text().not_null())
                    .col(ColumnDef::new(Users::AvatarUrl).text())
                    .col(
                        ColumnDef::new(Users::FirstSeen)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Users::LastSeen)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Functional index on lower(email) for case-insensitive lookup.
        // SeaORM's IndexCreateStatement doesn't model expression
        // indexes natively, so drop down to raw SQL — it's a one-line
        // statement and runs in the same migration transaction.
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX idx_users_email_lower ON users (lower(email))",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Users::Table).to_owned())
            .await
    }
}
