// Storage-layer errors. Caller (cracha-api) maps to HTTP status codes;
// keep this enum focused on database/migration concerns, not transport.

use thiserror::Error;

// sea_orm_migration::DbErr is a re-export of sea_orm::DbErr, so the
// single From impl below covers both migration + runtime errors.
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("user not found: {0}")]
    UserNotFound(String),

    #[error("constraint violation: {0}")]
    Constraint(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;
