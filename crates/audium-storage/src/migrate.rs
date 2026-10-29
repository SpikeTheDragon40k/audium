//! Versioned schema migrations, tracked in `PRAGMA user_version`.

use std::path::Path;

use rusqlite::Connection;

use crate::{Result, StorageError, now_ms};

pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "initial",
    sql: include_str!("../../../migrations/0001_initial.sql"),
}];

pub const LATEST_VERSION: i64 = 1;

pub(crate) fn run(conn: &mut Connection, path: &Path, migrations: &[Migration]) -> Result<()> {
    let latest = migrations.last().map_or(0, |m| m.version);
    let current: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if current > latest {
        return Err(StorageError::TooNew {
            found: current,
            supported: latest,
        });
    }
    let pending: Vec<&Migration> = migrations.iter().filter(|m| m.version > current).collect();
    if pending.is_empty() {
        return Ok(());
    }

    // A brand-new, empty database has nothing to back up.
    let backup = if current > 0 || has_tables(conn)? {
        backup(conn, path, current)?
    } else {
        String::from("(none: new database)")
    };

    for m in pending {
        tracing::info!(version = m.version, name = m.name, "applying migration");
        let tx = conn.transaction()?;
        let result = tx
            .execute_batch(m.sql)
            .and_then(|()| tx.pragma_update(None, "user_version", m.version));
        match result {
            Ok(()) => tx.commit()?,
            Err(source) => {
                // Dropping the transaction rolls back every statement of this
                // migration, including the user_version change.
                drop(tx);
                return Err(StorageError::MigrationFailed {
                    version: m.version,
                    name: m.name,
                    backup,
                    source,
                });
            }
        }
    }
    Ok(())
}

fn has_tables(conn: &Connection) -> Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE type = 'table'",
        [],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// Writes a consistent copy next to the database and returns its path.
fn backup(conn: &Connection, path: &Path, version: i64) -> Result<String> {
    let mut name = path.as_os_str().to_owned();
    name.push(format!(".bak-v{version}-{}", now_ms()));
    let target = name
        .to_str()
        .ok_or_else(|| StorageError::Backup("database path is not valid UTF-8".into()))?
        .to_owned();
    conn.execute("VACUUM INTO ?1", [&target])
        .map_err(|e| StorageError::Backup(e.to_string()))?;
    tracing::info!(backup = %target, "database backed up before migration");
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;

    const V1: Migration = Migration {
        version: 1,
        name: "one",
        sql: "CREATE TABLE a (x INTEGER); INSERT INTO a VALUES (42);",
    };
    const V2_BROKEN: Migration = Migration {
        version: 2,
        name: "broken",
        sql: "CREATE TABLE b (y INTEGER); THIS IS NOT SQL;",
    };
    const V2_GOOD: Migration = Migration {
        version: 2,
        name: "two",
        sql: "CREATE TABLE b (y INTEGER);",
    };

    fn version(db: &Database) -> i64 {
        db.conn()
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .expect("version")
    }

    fn backups(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .expect("read dir")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".bak-"))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn real_schema_applies_and_reopens() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("audium.db");
        let db = Database::open(&path).expect("open");
        assert_eq!(version(&db), LATEST_VERSION);
        let fk: i64 = db
            .conn()
            .pragma_query_value(None, "foreign_keys", |r| r.get(0))
            .expect("fk");
        let wal: String = db
            .conn()
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
            .expect("wal");
        assert_eq!((fk, wal.as_str()), (1, "wal"));
        drop(db);
        let db = Database::open(&path).expect("reopen");
        assert_eq!(version(&db), LATEST_VERSION);
        // New database: no backup; reopening at the latest version: none either.
        assert!(backups(dir.path()).is_empty());
    }

    #[test]
    fn backs_up_before_upgrading() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("audium.db");
        drop(Database::open_with(&path, &[V1]).expect("v1"));
        let db = Database::open_with(&path, &[V1, V2_GOOD]).expect("v2");
        assert_eq!(version(&db), 2);
        let b = backups(dir.path());
        assert_eq!(b.len(), 1, "{b:?}");
        assert!(b[0].starts_with("audium.db.bak-v1-"));
        // The backup holds the old data at the old version.
        let old = Connection::open(dir.path().join(&b[0])).expect("open backup");
        let x: i64 = old
            .query_row("SELECT x FROM a", [], |r| r.get(0))
            .expect("data");
        let v: i64 = old
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .expect("v");
        assert_eq!((x, v), (42, 1));
    }

    #[test]
    fn failed_migration_leaves_database_unchanged() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("audium.db");
        drop(Database::open_with(&path, &[V1]).expect("v1"));

        let err = Database::open_with(&path, &[V1, V2_BROKEN])
            .err()
            .expect("must fail");
        assert!(
            matches!(err, StorageError::MigrationFailed { version: 2, .. }),
            "{err}"
        );

        let db = Database::open_with(&path, &[V1]).expect("still opens at v1");
        assert_eq!(version(&db), 1);
        let tables: i64 = db
            .conn()
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name = 'b'",
                [],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(tables, 0, "partial migration must be rolled back");
        let x: i64 = db
            .conn()
            .query_row("SELECT x FROM a", [], |r| r.get(0))
            .expect("data kept");
        assert_eq!(x, 42);
    }

    #[test]
    fn refuses_newer_schema_without_touching_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("audium.db");
        drop(Database::open_with(&path, &[V1, V2_GOOD]).expect("v2"));
        let err = Database::open_with(&path, &[V1])
            .err()
            .expect("must refuse");
        assert!(matches!(
            err,
            StorageError::TooNew {
                found: 2,
                supported: 1
            }
        ));
        let db = Database::open_with(&path, &[V1, V2_GOOD]).expect("v2 again");
        assert_eq!(version(&db), 2);
    }

    #[test]
    fn readonly_requires_latest_schema() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("audium.db");
        assert!(crate::ReadOnly::open(&path).is_err(), "missing file");
        drop(Database::open(&path).expect("open"));
        assert!(crate::ReadOnly::open(&path).is_ok());
    }
}
