use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use anyhow::Context as _;
use librqbit::storage::{
    BoxStorageFactory, StorageFactory, StorageFactoryExt, TorrentStorage, ValidPieceIndex,
};
use librqbit::{ManagedTorrentShared, TorrentMetadata};

use crate::layout::Layout;

#[derive(Debug, Clone, Copy)]
pub(crate) struct MemoryStorageFactory;

impl StorageFactory for MemoryStorageFactory {
    type Storage = MemoryStorage;

    fn create(
        &self,
        _shared: &ManagedTorrentShared,
        metadata: &TorrentMetadata,
    ) -> anyhow::Result<MemoryStorage> {
        Ok(MemoryStorage::new(Layout::of(metadata)))
    }

    fn clone_box(&self) -> BoxStorageFactory {
        self.boxed()
    }
}

struct Inner {
    layout: Layout,
    pieces: RwLock<HashMap<u64, Box<[u8]>>>,
}

#[derive(Clone)]
pub(crate) struct MemoryStorage {
    inner: Arc<Inner>,
}

fn poisoned<T>(_: T) -> anyhow::Error {
    anyhow::anyhow!("the memory storage lock was poisoned")
}

impl MemoryStorage {
    pub(crate) fn new(layout: Layout) -> Self {
        Self {
            inner: Arc::new(Inner {
                layout,
                pieces: RwLock::new(HashMap::new()),
            }),
        }
    }

    #[cfg(test)]
    fn held(&self) -> usize {
        self.inner
            .pieces
            .read()
            .map_or(0, |p| p.values().map(|b| b.len()).sum())
    }
}

impl TorrentStorage for MemoryStorage {
    fn init(
        &mut self,
        _shared: &ManagedTorrentShared,
        _metadata: &TorrentMetadata,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn pread_exact(&self, file_id: usize, offset: u64, buf: &mut [u8]) -> anyhow::Result<()> {
        let pieces = self.inner.pieces.read().map_err(poisoned)?;
        let mut done = 0;
        for (piece, at, len) in self.inner.layout.parts(file_id, offset, buf.len())? {
            let data = pieces
                .get(&piece)
                .with_context(|| format!("piece {piece} is not in memory"))?;
            buf[done..done + len].copy_from_slice(&data[at..at + len]);
            done += len;
        }
        Ok(())
    }

    fn pwrite_all(&self, file_id: usize, offset: u64, buf: &[u8]) -> anyhow::Result<()> {
        let mut pieces = self.inner.pieces.write().map_err(poisoned)?;
        let mut done = 0;
        for (piece, at, len) in self.inner.layout.parts(file_id, offset, buf.len())? {
            let data = pieces
                .entry(piece)
                .or_insert_with(|| vec![0; self.inner.layout.piece_len(piece)].into_boxed_slice());
            data[at..at + len].copy_from_slice(&buf[done..done + len]);
            done += len;
        }
        Ok(())
    }

    fn remove_file(&self, _file_id: usize, _filename: &std::path::Path) -> anyhow::Result<()> {
        Ok(())
    }

    fn remove_directory_if_empty(&self, _path: &std::path::Path) -> anyhow::Result<()> {
        Ok(())
    }

    fn ensure_file_length(&self, _file_id: usize, _length: u64) -> anyhow::Result<()> {
        Ok(())
    }

    fn take(&self) -> anyhow::Result<Box<dyn TorrentStorage>> {
        Ok(Box::new(self.clone()))
    }

    fn discard_piece(&self, piece: ValidPieceIndex) -> anyhow::Result<()> {
        self.inner
            .pieces
            .write()
            .map_err(poisoned)?
            .remove(&u64::from(piece.get()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn storage() -> MemoryStorage {
        MemoryStorage::new(Layout::new(vec![0, 10], 8, 25))
    }

    #[test]
    fn writes_read_back_across_pieces_and_files() {
        let s = storage();
        s.pwrite_all(0, 0, b"0123456789")
            .unwrap_or_else(|e| panic!("{e}"));
        s.pwrite_all(1, 0, b"abcdefghijklmno")
            .unwrap_or_else(|e| panic!("{e}"));
        let mut buf = [0u8; 9];
        s.pread_exact(1, 4, &mut buf)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(&buf, b"efghijklm");
        let mut buf = [0u8; 4];
        s.pread_exact(0, 6, &mut buf)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(&buf, b"6789");
        assert_eq!(s.held(), 25, "the last piece is short");
    }

    #[test]
    fn missing_and_discarded_pieces_cannot_be_read() {
        let s = storage();
        let mut buf = [0u8; 2];
        assert!(s.pread_exact(0, 0, &mut buf).is_err());
        s.pwrite_all(0, 0, b"01234567")
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(s.pread_exact(0, 6, &mut buf).is_ok());
        assert!(
            s.pread_exact(0, 7, &mut buf).is_err(),
            "crosses into piece 1"
        );
        let lengths = librqbit::storage::Lengths::new(25, 8).unwrap_or_else(|e| panic!("{e}"));
        let first = lengths
            .validate_piece_index(0)
            .unwrap_or_else(|| panic!("piece 0"));
        s.discard_piece(first).unwrap_or_else(|e| panic!("{e}"));
        assert!(s.pread_exact(0, 0, &mut buf).is_err());
        assert_eq!(s.held(), 0);
    }

    #[test]
    fn out_of_range_access_is_an_error() {
        let s = storage();
        assert!(s.pwrite_all(2, 0, b"x").is_err(), "no such file");
        assert!(s.pwrite_all(1, 15, b"x").is_err(), "past the end");
    }
}
