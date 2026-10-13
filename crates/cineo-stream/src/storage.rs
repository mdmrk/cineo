use std::fs::{File, OpenOptions};
use std::io::{self, IoSlice};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use anyhow::Context as _;
use librqbit::storage::{
    BoxStorageFactory, StorageFactory, StorageFactoryExt, TorrentStorage, ValidPieceIndex,
};
use librqbit::{ManagedTorrentShared, TorrentMetadata};

use crate::layout::Layout;

pub(crate) fn torrent_dir(cache: &Path, info_hash: &str) -> PathBuf {
    cache.join(info_hash)
}

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
            Layout::of(metadata),
        ))
    }

    fn clone_box(&self) -> BoxStorageFactory {
        self.clone().boxed()
    }
}

struct Inner {
    dir: PathBuf,
    layout: Layout,
    discarding: RwLock<()>,
}

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

fn poisoned<T>(_: T) -> anyhow::Error {
    anyhow::anyhow!("a storage lock was poisoned")
}

impl CacheStorage {
    pub(crate) fn new(dir: PathBuf, layout: Layout) -> Self {
        Self {
            inner: Arc::new(Inner {
                dir,
                layout,
                discarding: RwLock::new(()),
            }),
        }
    }

    fn piece_path(&self, piece: u64) -> PathBuf {
        self.inner.dir.join(piece.to_string())
    }

    fn write(
        &self,
        open: &mut Option<(u64, File)>,
        file_id: usize,
        offset: u64,
        buf: &[u8],
    ) -> anyhow::Result<()> {
        let mut done = 0;
        for (piece, at, len) in self.inner.layout.parts(file_id, offset, buf.len())? {
            let file = match open {
                Some((open_piece, file)) if *open_piece == piece => file,
                _ => {
                    let file = OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(false)
                        .open(self.piece_path(piece))
                        .context("cannot open a piece file")?;
                    &open.insert((piece, file)).1
                }
            };
            write_all_at(file, &buf[done..done + len], at as u64)?;
            done += len;
        }
        Ok(())
    }
}

fn read_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<usize> {
    std::os::unix::fs::FileExt::read_at(file, buf, offset)
}

fn write_all_at(file: &File, buf: &[u8], offset: u64) -> io::Result<()> {
    std::os::unix::fs::FileExt::write_all_at(file, buf, offset)
}

impl TorrentStorage for CacheStorage {
    fn init(
        &mut self,
        _shared: &ManagedTorrentShared,
        _metadata: &TorrentMetadata,
    ) -> anyhow::Result<()> {
        crate::cache::create_private_dir(&self.inner.dir).context("cannot create the cache")
    }

    fn pread_exact(&self, file_id: usize, offset: u64, buf: &mut [u8]) -> anyhow::Result<()> {
        let _guard = self.inner.discarding.read().map_err(poisoned)?;
        let mut done = 0;
        for (piece, at, len) in self.inner.layout.parts(file_id, offset, buf.len())? {
            let file = match File::open(self.piece_path(piece)) {
                Ok(file) => file,
                Err(err) if err.kind() == io::ErrorKind::NotFound => {
                    anyhow::bail!("piece {piece} is not on disk")
                }
                Err(err) => return Err(err).context("cannot open a piece file"),
            };
            let part = &mut buf[done..done + len];
            let mut read = 0;
            while read < len {
                match read_at(&file, &mut part[read..], (at + read) as u64)? {
                    0 => break,
                    n => read += n,
                }
            }
            part[read..].fill(0);
            done += len;
        }
        Ok(())
    }

    fn pwrite_all(&self, file_id: usize, offset: u64, buf: &[u8]) -> anyhow::Result<()> {
        let _guard = self.inner.discarding.read().map_err(poisoned)?;
        self.write(&mut None, file_id, offset, buf)
    }

    fn pwrite_all_vectored(
        &self,
        file_id: usize,
        offset: u64,
        bufs: [IoSlice<'_>; 2],
    ) -> anyhow::Result<usize> {
        let _guard = self.inner.discarding.read().map_err(poisoned)?;
        let mut open = None;
        let mut at = offset;
        for buf in &bufs {
            self.write(&mut open, file_id, at, buf)?;
            at += buf.len() as u64;
        }
        Ok(bufs.iter().map(|b| b.len()).sum())
    }

    fn remove_file(&self, _file_id: usize, _filename: &Path) -> anyhow::Result<()> {
        Ok(())
    }

    fn remove_directory_if_empty(&self, _path: &Path) -> anyhow::Result<()> {
        Ok(())
    }

    fn ensure_file_length(&self, _file_id: usize, _length: u64) -> anyhow::Result<()> {
        Ok(())
    }

    fn take(&self) -> anyhow::Result<Box<dyn TorrentStorage>> {
        Ok(Box::new(self.clone()))
    }

    fn discard_piece(&self, piece: ValidPieceIndex) -> anyhow::Result<()> {
        let _guard = self.inner.discarding.write().map_err(poisoned)?;
        match std::fs::remove_file(self.piece_path(u64::from(piece.get()))) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err.into()),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn storage(name: &str) -> (PathBuf, CacheStorage) {
        let dir = std::env::temp_dir()
            .join(format!("cineo-stream-storage-{}", std::process::id()))
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
        let storage = CacheStorage::new(dir.clone(), Layout::new(vec![0, 10], 8, 25));
        (dir, storage)
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("{e}"))
            .map(|e| e.map(|e| e.file_name().to_string_lossy().into_owned()))
            .collect::<Result<_, _>>()
            .unwrap_or_else(|e| panic!("{e}"));
        names.sort();
        names
    }

    #[test]
    fn data_lives_under_piece_indexes_only() {
        let (dir, s) = storage("names");
        s.pwrite_all(1, 4, b"data")
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(names(&dir), ["1", "2"], "bytes 14..18 span pieces 1 and 2");
        assert!(s.pwrite_all(2, 0, b"x").is_err(), "no such file");
        assert!(s.pwrite_all(1, 15, b"x").is_err(), "past the end");
    }

    #[test]
    fn writes_read_back_across_pieces_and_files() {
        let (_, s) = storage("roundtrip");
        s.pwrite_all(0, 0, b"0123456789")
            .unwrap_or_else(|e| panic!("{e}"));
        s.pwrite_all(1, 0, b"abcdefghijklmno")
            .unwrap_or_else(|e| panic!("{e}"));
        let mut buf = [0u8; 9];
        s.pread_exact(1, 4, &mut buf)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(&buf, b"efghijklm");
    }

    #[test]
    fn vectored_writes_read_back_across_pieces() {
        let (_, s) = storage("vectored");
        let written = s
            .pwrite_all_vectored(0, 2, [IoSlice::new(b"abcde"), IoSlice::new(b"fghijklm")])
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(written, 13);
        let mut buf = [0u8; 13];
        s.pread_exact(0, 2, &mut buf)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(&buf, b"abcdefghijklm");
    }

    #[test]
    fn reading_a_piece_never_written_is_an_error() {
        let (dir, s) = storage("missing");
        let mut buf = [7u8; 8];
        assert!(s.pread_exact(0, 0, &mut buf).is_err());
        assert!(names(&dir).is_empty(), "reading creates nothing");
    }

    #[test]
    fn unwritten_bytes_of_a_piece_read_as_zeros() {
        let (_, s) = storage("zeros");
        s.pwrite_all(0, 2, b"ab").unwrap_or_else(|e| panic!("{e}"));
        let mut buf = [7u8; 6];
        s.pread_exact(0, 0, &mut buf)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(&buf, b"\0\0ab\0\0", "holes and the tail read as zeros");
    }

    #[test]
    fn a_discarded_piece_frees_its_file() {
        let (dir, s) = storage("discard");
        s.pwrite_all(0, 0, b"0123456789")
            .unwrap_or_else(|e| panic!("{e}"));
        let lengths = librqbit::storage::Lengths::new(25, 8).unwrap_or_else(|e| panic!("{e}"));
        let first = lengths
            .validate_piece_index(0)
            .unwrap_or_else(|| panic!("piece 0"));
        s.discard_piece(first).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(names(&dir), ["1"]);
        let mut buf = [0u8; 2];
        assert!(s.pread_exact(0, 0, &mut buf).is_err());
        s.discard_piece(first).unwrap_or_else(|e| panic!("{e}"));
    }
}
