use std::path::Path;
use std::time::Duration;

use cineo_core::addon::{ContentType, TransportUrl};
use cineo_core::app::{Effect, LibraryItem, SavedStream, Setting, SettingError, Settings};
use cineo_core::remote::{Hosting, Peer, RemoteKey};
use cineo_core::sync::{Changes, DeviceId, Entry, Pairing, Record, Replica, Stamp, SyncKey};
use rusqlite::{Connection, ErrorCode, OptionalExtension, Row, params};
use tracing::warn;
use url::Url;

use crate::migrate::{MIGRATIONS, migrate};

pub const DB_FILE: &str = "cineo.db";

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

#[derive(Debug)]
pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open_in(dir: &Path) -> Result<Self, StoreError> {
        create_private_dir(dir).map_err(StoreError::CreateDir)?;
        Self::open(&dir.join(DB_FILE))
    }

    pub fn open(path: &Path) -> Result<Self, StoreError> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self, StoreError> {
        conn.busy_timeout(BUSY_TIMEOUT)?;
        migrate(&mut conn, MIGRATIONS)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        Ok(Self { conn })
    }

    pub fn addons(&self) -> Result<Vec<TransportUrl>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT transport_url FROM addons ORDER BY position")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut addons = Vec::new();
        for (index, raw) in rows.enumerate() {
            match TransportUrl::parse(&raw?) {
                Ok(addon) => addons.push(addon),
                Err(err) => warn!(index, %err, "skipping stored addon"),
            }
        }
        Ok(addons)
    }

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

    pub fn library(&self) -> Result<Vec<LibraryItem>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content_type, name, poster, video_id,
                    time_offset_ms, duration_ms, updated_ms, stream, favorite
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

    pub fn save_library_item(&self, item: &LibraryItem) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO library_items
                 (id, content_type, name, poster, video_id,
                  time_offset_ms, duration_ms, updated_ms, stream, favorite)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT (id) DO UPDATE SET
                 content_type = excluded.content_type,
                 name = excluded.name,
                 poster = excluded.poster,
                 video_id = excluded.video_id,
                 time_offset_ms = excluded.time_offset_ms,
                 duration_ms = excluded.duration_ms,
                 updated_ms = excluded.updated_ms,
                 stream = excluded.stream,
                 favorite = excluded.favorite",
            params![
                item.id,
                item.content_type.as_str(),
                item.name,
                item.poster.as_ref().map(Url::as_str),
                item.video_id,
                to_sql_int(item.time_offset_ms),
                to_sql_int(item.duration_ms),
                to_sql_int(item.updated_ms),
                item.stream.as_ref().and_then(|s| s.to_json()),
                to_sql_int(item.favorited.unwrap_or(0)),
            ],
        )?;
        Ok(())
    }

    pub fn delete_library_item(&self, id: &str) -> Result<(), StoreError> {
        self.conn
            .execute("DELETE FROM library_items WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn clear_library(&mut self) -> Result<(), StoreError> {
        self.conn.execute("DELETE FROM library_items", [])?;
        Ok(())
    }

    pub fn settings(&self) -> Result<Settings, StoreError> {
        self.settings_over(Settings::default())
    }

    pub fn settings_over(&self, defaults: Settings) -> Result<Settings, StoreError> {
        let mut settings = defaults;
        let mut stmt = self.conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (key, value) = row?;
            match Setting::parse(&key, &value) {
                Ok(setting) => settings.set(setting),
                Err(SettingError::UnknownKey) => {}
                Err(_) => warn!(key, "ignoring an unreadable setting"),
            }
        }
        Ok(settings)
    }

    pub fn save_settings(&mut self, settings: &Settings) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        {
            let mut upsert = tx.prepare(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            )?;
            for setting in settings.all() {
                upsert.execute(params![setting.key(), setting.value()])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn sync(&self, defaults: Settings) -> Result<Option<(Pairing, Replica)>, StoreError> {
        let Some((key, server, device, clock_ms, clock_counter, since, born)) = self
            .conn
            .query_row(
                "SELECT key, server, device, clock_ms, clock_counter, since, born
                 FROM sync_pairing WHERE id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, Vec<u8>>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                    ))
                },
            )
            .optional()?
        else {
            return Ok(None);
        };
        let (Ok(key), Ok(server)) = (<[u8; 16]>::try_from(key), Url::parse(&server)) else {
            warn!("ignoring an unreadable sync pairing");
            return Ok(None);
        };
        let pairing = Pairing {
            key: SyncKey::from_bytes(key),
            server,
        };
        let clock = Stamp {
            physical_ms: from_sql_int(clock_ms),
            counter: u32::try_from(clock_counter).unwrap_or(0),
            device: DeviceId(device.cast_unsigned()),
        };
        let mut stmt = self
            .conn
            .prepare("SELECT name, record, seq, dirty, seeded FROM sync_records")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, bool>(3)?,
                row.get::<_, bool>(4)?,
            ))
        })?;
        let mut entries = Vec::new();
        for row in rows {
            let (name, record, seq, dirty, seeded) = row?;
            match Record::parse(&record).filter(|r| r.name.to_string() == name) {
                Some(record) => entries.push(Entry {
                    record,
                    seq: from_sql_int(seq),
                    dirty,
                    seeded,
                }),
                None => warn!("skipping an unreadable sync record"),
            }
        }
        let replica = Replica::restore(
            defaults,
            clock,
            from_sql_int(since),
            born.cast_unsigned(),
            entries,
        );
        Ok(Some((pairing, replica)))
    }

    pub fn pair(&mut self, pairing: &Pairing, device: DeviceId) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM sync_records", [])?;
        tx.execute(
            "INSERT OR REPLACE INTO sync_pairing
                 (id, key, server, device, clock_ms, clock_counter, since)
             VALUES (1, ?1, ?2, ?3, 0, 0, 0)",
            params![
                pairing.key.as_bytes().as_slice(),
                pairing.server.as_str(),
                device.0.cast_signed(),
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn save_sync_changes(&mut self, changes: &Changes) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        let paired = tx.execute(
            "UPDATE sync_pairing SET clock_ms = ?1, clock_counter = ?2, since = ?3, born = ?4
             WHERE id = 1",
            params![
                to_sql_int(changes.clock.physical_ms),
                changes.clock.counter,
                to_sql_int(changes.since),
                changes.born.cast_signed(),
            ],
        )?;
        if paired == 0 {
            return Ok(());
        }
        {
            let mut upsert = tx.prepare(
                "INSERT INTO sync_records (name, record, seq, dirty, seeded)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (name) DO UPDATE SET
                     record = excluded.record,
                     seq = excluded.seq,
                     dirty = excluded.dirty,
                     seeded = excluded.seeded",
            )?;
            for entry in &changes.entries {
                upsert.execute(params![
                    entry.record.name.to_string(),
                    entry.record.to_json(),
                    to_sql_int(entry.seq),
                    entry.dirty,
                    entry.seeded,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn forget_sync(&mut self) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM sync_records", [])?;
        tx.execute("DELETE FROM sync_pairing", [])?;
        tx.commit()?;
        Ok(())
    }

    pub fn remote(&self) -> Result<(Hosting, Option<Peer>), StoreError> {
        let host = match self
            .conn
            .query_row("SELECT key FROM remote_host WHERE id = 1", [], |row| {
                row.get::<_, Option<Vec<u8>>>(0)
            })
            .optional()?
        {
            None => Hosting::Unchosen,
            Some(None) => Hosting::Off,
            Some(Some(key)) => match <[u8; 16]>::try_from(key) {
                Ok(key) => Hosting::On(RemoteKey::from_bytes(key)),
                Err(_) => {
                    warn!("ignoring an unreadable remote control key");
                    Hosting::Off
                }
            },
        };
        let peer = self
            .conn
            .query_row(
                "SELECT address, key FROM remote_peer WHERE id = 1",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?)),
            )
            .optional()?
            .and_then(|(address, key)| {
                let peer = cineo_core::remote::parse_address(&address)
                    .zip(<[u8; 16]>::try_from(key).ok())
                    .map(|(address, key)| Peer {
                        address,
                        key: RemoteKey::from_bytes(key),
                    });
                if peer.is_none() {
                    warn!("ignoring an unreadable remote pairing");
                }
                peer
            });
        Ok((host, peer))
    }

    pub fn save_remote_host(&self, host: Option<&RemoteKey>) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO remote_host (id, key) VALUES (1, ?1)",
            params![host.map(|key| key.as_bytes().as_slice())],
        )?;
        Ok(())
    }

    pub fn save_remote_peer(&self, peer: Option<&Peer>) -> Result<(), StoreError> {
        match peer {
            Some(peer) => self.conn.execute(
                "INSERT OR REPLACE INTO remote_peer (id, address, key) VALUES (1, ?1, ?2)",
                params![peer.address.to_string(), peer.key.as_bytes().as_slice()],
            )?,
            None => self.conn.execute("DELETE FROM remote_peer", [])?,
        };
        Ok(())
    }

    pub fn apply(&mut self, effect: &Effect) -> Result<bool, StoreError> {
        match effect {
            Effect::SaveAddons(addons) => self.save_addons(addons)?,
            Effect::SaveLibraryItem(item) => self.save_library_item(item)?,
            Effect::DeleteLibraryItem(id) => self.delete_library_item(id)?,
            Effect::ClearLibrary => self.clear_library()?,
            Effect::SaveSettings(settings) => self.save_settings(settings)?,
            Effect::SavePairing { pairing, device } => self.pair(pairing, *device)?,
            Effect::SaveSync(changes) => self.save_sync_changes(changes)?,
            Effect::ForgetSync => self.forget_sync()?,
            Effect::SaveRemoteHost(host) => self.save_remote_host(host.as_ref())?,
            Effect::SaveRemotePeer(peer) => self.save_remote_peer(peer.as_ref())?,
            _ => return Ok(false),
        }
        Ok(true)
    }
}

fn to_sql_int(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn from_sql_int(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

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
    let poster = poster
        .and_then(|raw| Url::parse(&raw).ok())
        .filter(|url| matches!(url.scheme(), "http" | "https"));
    let stream: Option<String> = row.get(8)?;
    let stream = stream
        .and_then(|raw| SavedStream::parse(&raw))
        .map(Box::new);
    Ok(Ok(LibraryItem {
        id,
        content_type,
        name: row.get(2)?,
        poster,
        video_id: row.get(4)?,
        time_offset_ms,
        duration_ms,
        updated_ms,
        favorited: u64::try_from(row.get::<_, i64>(9)?)
            .ok()
            .filter(|&order| order > 0),
        stream,
    }))
}

pub fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)
}
