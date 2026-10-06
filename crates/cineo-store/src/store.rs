//! The database handle and its queries.

use std::path::{Path, PathBuf};
use std::time::Duration;

use cineo_core::addon::{ContentType, TransportUrl};
use cineo_core::app::{Effect, LibraryItem, Settings};
use etcetera::{AppStrategy, AppStrategyArgs, choose_app_strategy};
use rusqlite::{Connection, ErrorCode, Row, params};
use tracing::warn;
use url::Url;

use crate::migrate::{MIGRATIONS, migrate};

/// File name of the database inside the data directory.
pub const DB_FILE: &str = "cineo.db";

/// How long a write waits for another process (e.g. the CLI next to the GUI)
/// holding the database lock.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StoreError {
    #[error("cannot create the data directory")]
    CreateDir(#[source] std::io::Error),
    #[error("the database file is corrupt or not a SQLite database; it was left untouched")]
    Corrupt(#[source] rusqlite::Error),
    #[error(
        "the database has schema version {found}, but this build supports up to {supported}; \
         it was left untouched"
    )]
    UnsupportedVersion { found: i64, supported: i64 },
    #[error(
        "migrating the database to schema version {to} failed; it was left at its previous version"
    )]
    Migration {
        to: usize,
        #[source]
        source: rusqlite::Error,
    },
    #[error("database error")]
    Sqlite(#[source] rusqlite::Error),
}

impl From<rusqlite::Error> for StoreError {
    fn from(err: rusqlite::Error) -> Self {
        match err.sqlite_error_code() {
            Some(ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt) => Self::Corrupt(err),
            _ => Self::Sqlite(err),
        }
    }
}

/// The platform data directory for Cineo: `$XDG_DATA_HOME/cineo` (default
/// `~/.local/share/cineo`) on Linux, `%APPDATA%\Cineo\data` on Windows,
/// `~/Library/Application Support/Cineo` on macOS. `None` if the platform
/// reports no home directory.
pub fn default_data_dir() -> Option<PathBuf> {
    let strategy = choose_app_strategy(AppStrategyArgs {
        top_level_domain: String::new(),
        author: String::new(),
        app_name: "Cineo".to_owned(),
    })
    .ok()?;
    Some(strategy.data_dir())
}

/// An open, migrated Cineo database.
///
/// Invariant: the schema is at [`crate::SCHEMA_VERSION`]. Calls are
/// blocking; async shells run them off the runtime's worker threads.
#[derive(Debug)]
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Opens `dir/cineo.db`, creating the directory (private to the user on
    /// Unix) and the database if needed, and migrates it.
    pub fn open_in(dir: &Path) -> Result<Self, StoreError> {
        create_private_dir(dir).map_err(StoreError::CreateDir)?;
        Self::open(&dir.join(DB_FILE))
    }

    /// Opens (or creates) the database at `path` and migrates it.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        Self::init(Connection::open(path)?)
    }

    /// A private in-memory database, for tests.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self, StoreError> {
        conn.busy_timeout(BUSY_TIMEOUT)?;
        migrate(&mut conn, MIGRATIONS)?;
        Ok(Self { conn })
    }

    /// Installed addons in user order. Rows that are no longer valid
    /// transport URLs are skipped with a warning.
    pub fn addons(&self) -> Result<Vec<TransportUrl>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT transport_url FROM addons ORDER BY position")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut addons = Vec::new();
        for (index, raw) in rows.enumerate() {
            match TransportUrl::parse(&raw?) {
                Ok(addon) => addons.push(addon),
                // Never log the URL itself: it may embed addon configuration.
                Err(err) => warn!(index, %err, "skipping stored addon"),
            }
        }
        Ok(addons)
    }

    /// Replaces the installed addon list; the order is kept. Duplicates keep
    /// their first position.
    pub fn save_addons(&mut self, addons: &[TransportUrl]) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM addons", [])?;
        {
            let mut insert = tx.prepare(
                "INSERT INTO addons (position, transport_url) VALUES (?1, ?2)
                 ON CONFLICT (transport_url) DO NOTHING",
            )?;
            for (position, addon) in (0_i64..).zip(addons) {
                insert.execute(params![position, addon.as_url().as_str()])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// All library items, most recently updated first. Rows that no longer
    /// satisfy the domain invariants are skipped with a warning.
    pub fn library(&self) -> Result<Vec<LibraryItem>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content_type, name, poster, video_id,
                    time_offset_ms, duration_ms, updated_ms
             FROM library_items ORDER BY updated_ms DESC, id",
        )?;
        let rows = stmt.query_map([], read_item)?;
        let mut items = Vec::new();
        for row in rows {
            match row? {
                Ok(item) => items.push(item),
                Err((id, reason)) => warn!(id, reason, "skipping stored library item"),
            }
        }
        Ok(items)
    }

    /// Inserts or replaces the item with the same meta id.
    pub fn save_library_item(&self, item: &LibraryItem) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO library_items
                 (id, content_type, name, poster, video_id,
                  time_offset_ms, duration_ms, updated_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT (id) DO UPDATE SET
                 content_type = excluded.content_type,
                 name = excluded.name,
                 poster = excluded.poster,
                 video_id = excluded.video_id,
                 time_offset_ms = excluded.time_offset_ms,
                 duration_ms = excluded.duration_ms,
                 updated_ms = excluded.updated_ms",
            params![
                item.id,
                item.content_type.as_str(),
                item.name,
                item.poster.as_ref().map(Url::as_str),
                item.video_id,
                to_sql_int(item.time_offset_ms),
                to_sql_int(item.duration_ms),
                to_sql_int(item.updated_ms),
            ],
        )?;
        Ok(())
    }

    /// Removes the item with meta id `id`, if any.
    pub fn delete_library_item(&self, id: &str) -> Result<(), StoreError> {
        self.conn
            .execute("DELETE FROM library_items WHERE id = ?1", [id])?;
        Ok(())
    }

    /// The saved settings; defaults for anything never saved or unreadable.
    pub fn settings(&self) -> Result<Settings, StoreError> {
        let mut settings = Settings::default();
        let mut stmt = self.conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (key, value) = row?;
            let flag = match value.as_str() {
                "true" => true,
                "false" => false,
                _ => {
                    warn!(key, "ignoring an unreadable setting");
                    continue;
                }
            };
            match key.as_str() {
                P2P_ENABLED => settings.p2p_enabled = flag,
                P2P_ACKNOWLEDGED => settings.p2p_acknowledged = flag,
                _ => {} // written by a newer version
            }
        }
        Ok(settings)
    }

    /// Saves every setting.
    pub fn save_settings(&mut self, settings: &Settings) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        {
            let mut upsert = tx.prepare(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            )?;
            for (key, flag) in [
                (P2P_ENABLED, settings.p2p_enabled),
                (P2P_ACKNOWLEDGED, settings.p2p_acknowledged),
            ] {
                upsert.execute(params![key, flag.to_string()])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Runs a persistence effect from the core. Other effects are ignored
    /// and return `false`.
    pub fn apply(&mut self, effect: &Effect) -> Result<bool, StoreError> {
        match effect {
            Effect::SaveAddons(addons) => self.save_addons(addons)?,
            Effect::SaveLibraryItem(item) => self.save_library_item(item)?,
            Effect::DeleteLibraryItem(id) => self.delete_library_item(id)?,
            Effect::SaveSettings(settings) => self.save_settings(settings)?,
            _ => return Ok(false),
        }
        Ok(true)
    }
}

const P2P_ENABLED: &str = "p2p_enabled";
const P2P_ACKNOWLEDGED: &str = "p2p_acknowledged";

/// Millisecond values above `i64::MAX` (~292 million years) saturate.
fn to_sql_int(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// Reads one row; `Err((id, reason))` for rows breaking a domain invariant.
fn read_item(row: &Row<'_>) -> rusqlite::Result<Result<LibraryItem, (String, &'static str)>> {
    let id: String = row.get(0)?;
    let content_type: String = row.get(1)?;
    let poster: Option<String> = row.get(3)?;
    let millis = |index| -> rusqlite::Result<Option<u64>> {
        Ok(u64::try_from(row.get::<_, i64>(index)?).ok())
    };
    let (Some(time_offset_ms), Some(duration_ms), Some(updated_ms)) =
        (millis(5)?, millis(6)?, millis(7)?)
    else {
        return Ok(Err((id, "negative time value")));
    };
    if id.is_empty() {
        return Ok(Err((id, "empty id")));
    }
    let Some(content_type) = ContentType::new(content_type) else {
        return Ok(Err((id, "invalid content type")));
    };
    // Same rule as addon images: http(s) only; anything else is dropped.
    let poster = poster
        .and_then(|raw| Url::parse(&raw).ok())
        .filter(|url| matches!(url.scheme(), "http" | "https"));
    Ok(Ok(LibraryItem {
        id,
        content_type,
        name: row.get(2)?,
        poster,
        video_id: row.get(4)?,
        time_offset_ms,
        duration_ms,
        updated_ms,
    }))
}

/// Creates `dir` and missing parents; on Unix the new directories are
/// readable only by the user, since the library is private.
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)
}
