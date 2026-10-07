//! Where torrent data is kept: `<cache>/<info hash>/<file index>`, so file
//! names from torrents never become filesystem paths (SECURITY.md
//! §Streaming engine). Files are created on first write and grow sparsely.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::Context as _;
use librqbit::storage::{BoxStorageFactory, StorageFactory, StorageFactoryExt, TorrentStorage};
use librqbit::{ManagedTorrentShared, TorrentMetadata};

/// A torrent's directory in the cache. `info_hash` is 40 hex characters.
pub(crate) fn torrent_dir(cache: &Path, info_hash: &str) -> PathBuf {
    cache.join(info_hash)
}

/// Creates a [`CacheStorage`] per torrent.
#[derive(Debug, Clone)]
pub(crate) struct CacheStorageFactory {
    pub(crate) cache: PathBuf,
}

impl StorageFactory for CacheStorageFactory {
    type Storage = CacheStorage;

    fn create(
        &self,
        shared: &ManagedTorrentShared,
        metadata: &TorrentMetadata,
    ) -> anyhow::Result<CacheStorage> {
        Ok(CacheStorage::new(
            torrent_dir(&self.cache, &shared.info_hash.as_string()),
            metadata.file_infos.len(),
        ))
    }

    fn clone_box(&self) -> BoxStorageFactory {
        self.clone().boxed()
    }
}

struct Slot {
    path: PathBuf,
    file: Mutex<Option<File>>,
}

struct Inner {
    dir: PathBuf,
    files: Vec<Slot>,
}

/// One torrent's files in the cache.
#[derive(Clone)]
pub(crate) struct CacheStorage {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for CacheStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheStorage")
            .field("dir", &self.inner.dir)
            .finish_non_exhaustive()
    }
}

impl CacheStorage {
    pub(crate) fn new(dir: PathBuf, file_count: usize) -> Self {
        let files = (0..file_count)
            .map(|index| Slot {
                path: dir.join(index.to_string()),
                file: Mutex::new(None),
            })
            .collect();
        Self {
            inner: Arc::new(Inner { dir, files }),
        }
    }

    fn slot(&self, file_id: usize) -> anyhow::Result<&Slot> {
        self.inner
            .files
            .get(file_id)
            .with_context(|| format!("no file {file_id} in this torrent"))
    }

    /// The open file, opening (and with `create`, creating) it on first use.
    /// `None` if it does not exist and `create` is false.
    fn open(slot: &Slot, create: bool) -> anyhow::Result<MutexGuard<'_, Option<File>>> {
        let mut guard = slot
            .file
            .lock()
            .map_err(|_| anyhow::anyhow!("a storage lock was poisoned"))?;
        if guard.is_none() {
            let opened = OpenOptions::new()
                .read(true)
                .write(true)
                .create(create)
                .truncate(false)
                .open(&slot.path);
            match opened {
                Ok(file) => *guard = Some(file),
                Err(err) if !create && err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(err).context("cannot open a cache file"),
            }
        }
        Ok(guard)
    }
}

impl TorrentStorage for CacheStorage {
    fn init(
        &mut self,
        _shared: &ManagedTorrentShared,
        _metadata: &TorrentMetadata,
    ) -> anyhow::Result<()> {
        crate::cache::create_private_dir(&self.inner.dir).context("cannot create the cache")
    }

    /// A file never written is an error, so librqbit's initial check skips
    /// it instead of hashing its whole length. Holes and the tail of a
    /// written file read as zeros, so their pieces fail the hash check and
    /// are downloaded.
    fn pread_exact(&self, file_id: usize, offset: u64, buf: &mut [u8]) -> anyhow::Result<()> {
        let slot = self.slot(file_id)?;
        let mut guard = Self::open(slot, false)?;
        let Some(file) = guard.as_mut() else {
            anyhow::bail!("file {file_id} has no data yet");
        };
        file.seek(SeekFrom::Start(offset))?;
        let mut read = 0;
        while read < buf.len() {
            match file.read(&mut buf[read..])? {
                0 => break,
                n => read += n,
            }
        }
        buf[read..].fill(0);
        Ok(())
    }

    fn pwrite_all(&self, file_id: usize, offset: u64, buf: &[u8]) -> anyhow::Result<()> {
        let slot = self.slot(file_id)?;
        let mut guard = Self::open(slot, true)?;
        let file = guard.as_mut().context("cache file missing after create")?;
        file.seek(SeekFrom::Start(offset))?;
        file.write_all(buf)?;
        Ok(())
    }

    fn remove_file(&self, file_id: usize, _filename: &std::path::Path) -> anyhow::Result<()> {
        let slot = self.slot(file_id)?;
        let mut guard = slot
            .file
            .lock()
            .map_err(|_| anyhow::anyhow!("a storage lock was poisoned"))?;
        *guard = None;
        match std::fs::remove_file(&slot.path) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(err.into()),
            _ => Ok(()),
        }
    }

    fn remove_directory_if_empty(&self, _path: &std::path::Path) -> anyhow::Result<()> {
        // The engine removes the torrent's own directory when it stops;
        // paths derived from the torrent are never touched.
        Ok(())
    }

    fn ensure_file_length(&self, _file_id: usize, _length: u64) -> anyhow::Result<()> {
        // Files grow on write; unread tails read as zeros.
        Ok(())
    }

    fn take(&self) -> anyhow::Result<Box<dyn TorrentStorage>> {
        Ok(Box::new(self.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("cineo-stream-storage-{}", std::process::id()))
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn data_lives_under_the_hash_and_file_index_only() {
        let root = dir("names");
        let storage = CacheStorage::new(torrent_dir(&root, "ab"), 2);
        std::fs::create_dir_all(root.join("ab")).unwrap_or_else(|e| panic!("{e}"));
        storage
            .pwrite_all(1, 4, b"data")
            .unwrap_or_else(|e| panic!("{e}"));
        let names: Vec<String> = std::fs::read_dir(root.join("ab"))
            .unwrap_or_else(|e| panic!("{e}"))
            .map(|e| e.map(|e| e.file_name().to_string_lossy().into_owned()))
            .collect::<Result<_, _>>()
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(names, ["1"]);
        assert!(storage.pwrite_all(2, 0, b"x").is_err(), "no such file");
    }

    /// Regression: a missing file read as zeros, so librqbit's initial
    /// check hashed every byte of every file before a torrent could start.
    /// An error makes it skip the file.
    #[test]
    fn reading_a_file_never_written_is_an_error() {
        let root = dir("missing");
        std::fs::create_dir_all(root.join("ef")).unwrap_or_else(|e| panic!("{e}"));
        let storage = CacheStorage::new(torrent_dir(&root, "ef"), 1);
        let mut buf = [7u8; 8];
        assert!(storage.pread_exact(0, 0, &mut buf).is_err());
        assert!(
            !root.join("ef").join("0").exists(),
            "reading creates nothing"
        );
    }

    #[test]
    fn unwritten_bytes_read_as_zeros() {
        let root = dir("zeros");
        std::fs::create_dir_all(root.join("cd")).unwrap_or_else(|e| panic!("{e}"));
        let storage = CacheStorage::new(torrent_dir(&root, "cd"), 2);
        storage
            .pwrite_all(0, 2, b"ab")
            .unwrap_or_else(|e| panic!("{e}"));
        let mut buf = [7u8; 6];
        storage
            .pread_exact(0, 0, &mut buf)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(&buf, b"\0\0ab\0\0", "holes and the tail read as zeros");
    }
}
