mod archive;
mod archive_storage;
mod sql_archive;

pub use archive::*;
pub use archive_storage::{ArchiveStorage, ArchiveStorageRef};
pub use sql_archive::*;
use crate::AnsiDBTransactionSqlCreator;

pub type ArchiveDatabase = sqlx::Sqlite;
pub type ArchiveConnection = sqlx::pool::PoolConnection<ArchiveDatabase>;
pub type ArchiveConnectionOptions = sqlx::sqlite::SqliteConnectOptions;
pub type ArchiveTransactionSqlCreator = AnsiDBTransactionSqlCreator;
