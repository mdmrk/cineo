use std::io;
use std::path::Path;

use tracing::{info, warn};

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

pub(crate) fn remove_torrent(dir: &Path) -> io::Result<()> {
    match std::fs::remove_dir_all(dir) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

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

    #[test]
    fn leftover_torrents_are_deleted_and_nothing_else() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let torrent = root.join("a".repeat(40));
        create_private_dir(&torrent).unwrap();
        std::fs::write(torrent.join("0"), b"data").unwrap();
        let dht = root.join("dht.json");
        std::fs::write(&dht, b"{}").unwrap();
        let other = root.join("not-a-hash");
        create_private_dir(&other).unwrap();

        remove_all(root).unwrap();
        assert!(!torrent.exists());
        assert!(dht.exists() && other.exists(), "only info-hash directories");
    }

    #[test]
    fn a_missing_cache_or_torrent_is_fine() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("nope");
        remove_all(&root).unwrap();
        remove_torrent(&root.join("b".repeat(40))).unwrap();
    }
}
