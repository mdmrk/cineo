//! Schema migrations, versioned by SQLite's `user_version` header field.

use rusqlite::{Connection, TransactionBehavior};
use tracing::info;

use crate::store::StoreError;

/// `MIGRATIONS[n]` upgrades schema version `n` to `n + 1`. Append only: a
/// released migration is never edited, and each released version gets a
/// fixture in `tests/fixtures/store/`.
pub(crate) const MIGRATIONS: &[&str] = &[
    // v1
    "CREATE TABLE addons (
        position INTEGER PRIMARY KEY,
        transport_url TEXT NOT NULL UNIQUE
    ) STRICT;
    CREATE TABLE library_items (
        id TEXT PRIMARY KEY NOT NULL,
        content_type TEXT NOT NULL,
        name TEXT NOT NULL,
        poster TEXT,
        video_id TEXT NOT NULL,
        time_offset_ms INTEGER NOT NULL,
        duration_ms INTEGER NOT NULL,
        updated_ms INTEGER NOT NULL
    ) STRICT;",
    // v2 (M9): user settings as key/value pairs; missing keys mean defaults.
    "CREATE TABLE settings (
        key TEXT PRIMARY KEY NOT NULL,
        value TEXT NOT NULL
    ) STRICT;",
];

/// The schema version this build reads and writes.
#[allow(clippy::cast_possible_wrap)] // a handful of migrations, far below i64::MAX
pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

pub(crate) fn user_version(conn: &Connection) -> rusqlite::Result<i64> {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
}

/// Brings `conn` to `migrations.len()`. All steps run in one transaction, so
/// a failure leaves the database at its previous version.
pub(crate) fn migrate(conn: &mut Connection, migrations: &[&str]) -> Result<(), StoreError> {
    let latest = i64::try_from(migrations.len()).unwrap_or(i64::MAX);
    // Reading the header also detects a file that is not a database, before
    // anything is written.
    if user_version(conn)? == latest {
        return Ok(());
    }
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    // Re-read under the write lock: another process may have migrated.
    let found = user_version(&tx)?;
    if found > latest || found < 0 {
        return Err(StoreError::UnsupportedVersion {
            found,
            supported: latest,
        });
    }
    let start = usize::try_from(found).unwrap_or(usize::MAX);
    for (index, sql) in migrations.iter().enumerate().skip(start) {
        tx.execute_batch(sql)
            .map_err(|source| StoreError::Migration {
                to: index + 1,
                source,
            })?;
    }
    tx.pragma_update(None, "user_version", latest)?;
    tx.commit()?;
    info!(from = found, to = latest, "database migrated");
    Ok(())
}

#[cfg(test)]
mod tests {
    // Test helpers panic on purpose: a panic is a failed assertion.
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn failed_migration_leaves_the_database_at_its_previous_version() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn, &MIGRATIONS[..1]).unwrap();
        let broken = [MIGRATIONS[0], "CREATE TABLE extra (x); THIS IS NOT SQL;"];
        let err = migrate(&mut conn, &broken).unwrap_err();
        assert!(
            matches!(err, StoreError::Migration { to: 2, .. }),
            "{err:?}"
        );
        assert_eq!(user_version(&conn).unwrap(), 1);
        let extra: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'extra'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(extra, 0, "the partial step was rolled back");
    }

    #[test]
    fn migrating_twice_is_a_no_op() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn, MIGRATIONS).unwrap();
        migrate(&mut conn, MIGRATIONS).unwrap();
        assert_eq!(user_version(&conn).unwrap(), SCHEMA_VERSION);
    }
}
