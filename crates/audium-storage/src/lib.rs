//! SQLite storage: migrations, queries, export and import.
//!
//! The database is never recreated silently: a failed or unknown migration
//! leaves the file untouched and returns an error for the UI to show.

mod migrate;
pub mod paths;
pub mod read;
#[cfg(test)]
mod tests;
pub mod write;

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OpenFlags};

pub use migrate::{LATEST_VERSION, MIGRATIONS, Migration};

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error(
        "the database was created by a newer version of Audium (schema {found}, this version supports up to {supported})"
    )]
    TooNew { found: i64, supported: i64 },
    #[error(
        "database migration to schema {version} ({name}) failed: {source}. The database was left unchanged; a backup is at {backup}"
    )]
    MigrationFailed {
        version: i64,
        name: &'static str,
        backup: String,
        #[source]
        source: rusqlite::Error,
    },
    #[error("could not back up the database before migrating: {0}")]
    Backup(String),
    #[error("the database schema is {found}, expected {expected}")]
    WrongVersion { found: i64, expected: i64 },
    #[error("invalid data in database: {0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;

/// A read-write database connection.
pub struct Database {
    conn: Connection,
    path: PathBuf,
}

impl Database {
    /// Opens (creating if needed) and migrates the database at `path`.
    pub fn open(path: &Path) -> Result<Self> {
        Self::open_with(path, MIGRATIONS)
    }

    /// As [`Database::open`] with a custom migration list (for tests).
    pub fn open_with(path: &Path, migrations: &[Migration]) -> Result<Self> {
        let mut conn = Connection::open(path)?;
        configure(&conn)?;
        migrate::run(&mut conn, path, migrations)?;
        Ok(Database {
            conn,
            path: path.to_owned(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}

/// A read-only connection for UI models. Never migrates.
pub struct ReadOnly {
    conn: Connection,
}

impl ReadOnly {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update(None, "foreign_keys", true)?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version != LATEST_VERSION {
            return Err(StorageError::WrongVersion {
                found: version,
                expected: LATEST_VERSION,
            });
        }
        Ok(ReadOnly { conn })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}

fn configure(conn: &Connection) -> Result<()> {
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(())
}

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}
