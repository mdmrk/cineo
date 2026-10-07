//! Torrent data on disk: one directory per torrent, named by its info hash,
//! kept only while that torrent plays (SECURITY.md §Streaming engine).

use std::io;
use std::path::Path;

use tracing::{info, warn};

/// Creates `dir` and missing parents, private to the user on Unix.
pub(crate) fn create_private_dir(dir: &Path) -> io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)
}

fn is_info_hash(name: &str) -> bool {
    name.len() == 40 && name.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Deletes one torrent's directory. A missing one is not an error.
pub(crate) fn remove_torrent(dir: &Path) -> io::Result<()> {
    match std::fs::remove_dir_all(dir) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Deletes every torrent directory in `cache`: data left by a run that did
/// not stop cleanly. Only directories named like an info hash are touched.
pub(crate) fn remove_all(cache: &Path) -> io::Result<()> {
    let entries = match std::fs::read_dir(cache) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        if !name.to_str().is_some_and(is_info_hash) || !entry.file_type()?.is_dir() {
            continue;
        }
        match remove_torrent(&entry.path()) {
            Ok(()) => info!("deleted leftover torrent data"),
            Err(err) => warn!(%err, "could not delete leftover torrent data"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn cache(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("cineo-stream-cache-{}", std::process::id()))
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        create_private_dir(&dir).unwrap_or_else(|e| panic!("{e}"));
        dir
    }

    #[test]
    fn leftover_torrents_are_deleted_and_nothing_else() {
        let root = cache("leftover");
        let torrent = root.join("a".repeat(40));
        create_private_dir(&torrent).unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(torrent.join("0"), b"data").unwrap_or_else(|e| panic!("{e}"));
        let dht = root.join("dht.json");
        std::fs::write(&dht, b"{}").unwrap_or_else(|e| panic!("{e}"));
        let other = root.join("not-a-hash");
        create_private_dir(&other).unwrap_or_else(|e| panic!("{e}"));

        remove_all(&root).unwrap_or_else(|e| panic!("{e}"));
        assert!(!torrent.exists());
        assert!(dht.exists() && other.exists(), "only info-hash directories");
    }

    #[test]
    fn a_missing_cache_or_torrent_is_fine() {
        let root = cache("missing").join("nope");
        remove_all(&root).unwrap_or_else(|e| panic!("{e}"));
        remove_torrent(&root.join("b".repeat(40))).unwrap_or_else(|e| panic!("{e}"));
    }
}
