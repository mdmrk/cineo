//! The bounded torrent cache: one directory per torrent, named by its info
//! hash, evicted least recently used first (SECURITY.md §Streaming engine).

use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use tracing::{info, warn};

/// Marker file whose modification time records the last use of a torrent.
const USED_MARKER: &str = "used";

/// Creates `dir` and missing parents, private to the user on Unix.
pub(crate) fn create_private_dir(dir: &Path) -> io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)
}

/// Records that the torrent in `dir` was just used.
pub(crate) fn touch(dir: &Path) -> io::Result<()> {
    create_private_dir(dir)?;
    std::fs::write(dir.join(USED_MARKER), b"")
}

fn is_info_hash(name: &str) -> bool {
    name.len() == 40 && name.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Space a file takes on disk; sparse files count only what is allocated
/// where the platform says so.
fn disk_usage(meta: &std::fs::Metadata) -> u64 {
    #[cfg(unix)]
    {
        std::os::unix::fs::MetadataExt::blocks(meta).saturating_mul(512)
    }
    #[cfg(not(unix))]
    {
        meta.len()
    }
}

fn dir_usage(dir: &Path) -> io::Result<(u64, SystemTime)> {
    let mut total = 0;
    let mut used = SystemTime::UNIX_EPOCH;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        if meta.is_file() {
            total += disk_usage(&meta);
            if entry.file_name() == USED_MARKER {
                used = meta.modified().unwrap_or(used);
            }
        }
    }
    Ok((total, used))
}

/// Deletes the least recently used torrents in `cache` until the rest
/// takes at most `limit` bytes. `keep` (an info hash) is never deleted.
/// Only directories named like an info hash are considered. Returns the
/// bytes still in use.
pub(crate) fn evict(cache: &Path, limit: u64, keep: Option<&str>) -> io::Result<u64> {
    let mut torrents: Vec<(PathBuf, u64, SystemTime)> = Vec::new();
    let entries = match std::fs::read_dir(cache) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(err) => return Err(err),
    };
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str().filter(|n| is_info_hash(n)) else {
            continue;
        };
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let (size, used) = dir_usage(&entry.path())?;
        if keep.is_some_and(|k| k.eq_ignore_ascii_case(name)) {
            // Counted, never deleted.
            torrents.push((entry.path(), size, SystemTime::now() + YEAR));
        } else {
            torrents.push((entry.path(), size, used));
        }
    }
    let mut total: u64 = torrents.iter().map(|(_, size, _)| size).sum();
    torrents.sort_by_key(|(_, _, used)| *used);
    for (path, size, used) in torrents {
        if total <= limit || used > SystemTime::now() {
            break;
        }
        match std::fs::remove_dir_all(&path) {
            Ok(()) => {
                total = total.saturating_sub(size);
                info!(bytes = size, "evicted a torrent from the cache");
            }
            Err(err) => warn!(%err, "could not evict a torrent from the cache"),
        }
    }
    Ok(total)
}

const YEAR: std::time::Duration = std::time::Duration::from_secs(365 * 24 * 3600);

#[cfg(test)]
mod tests {
    use super::*;

    fn cache(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("cineo-stream-cache-{}", std::process::id()))
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        create_private_dir(&dir).unwrap_or_else(|e| panic!("{e}"));
        dir
    }

    fn torrent(cache: &Path, hash_char: char, bytes: usize) -> PathBuf {
        let dir = cache.join(hash_char.to_string().repeat(40));
        touch(&dir).unwrap_or_else(|e| panic!("{e}"));
        // Real data, so it is allocated even on sparse-aware filesystems.
        std::fs::write(dir.join("0"), vec![1u8; bytes]).unwrap_or_else(|e| panic!("{e}"));
        dir
    }

    fn set_used(dir: &Path, secs_ago: u64) {
        let file = std::fs::File::options()
            .write(true)
            .open(dir.join(USED_MARKER))
            .unwrap_or_else(|e| panic!("{e}"));
        file.set_modified(SystemTime::now() - std::time::Duration::from_secs(secs_ago))
            .unwrap_or_else(|e| panic!("{e}"));
    }

    #[test]
    fn the_least_recently_used_torrents_go_first_until_the_cache_fits() {
        let root = cache("lru");
        let old = torrent(&root, 'a', 64 * 1024);
        let mid = torrent(&root, 'b', 64 * 1024);
        let new = torrent(&root, 'c', 64 * 1024);
        set_used(&old, 300);
        set_used(&mid, 200);
        set_used(&new, 100);
        let unrelated = root.join("blocklist.txt");
        std::fs::write(&unrelated, vec![0u8; 512 * 1024]).unwrap_or_else(|e| panic!("{e}"));

        let left = evict(&root, 150 * 1024, None).unwrap_or_else(|e| panic!("{e}"));
        assert!(!old.exists() && mid.exists() && new.exists());
        assert!(left <= 150 * 1024, "{left}");
        assert!(unrelated.exists(), "only info-hash directories are evicted");
    }

    #[test]
    fn the_torrent_in_use_is_kept_even_over_the_limit() {
        let root = cache("keep");
        let current = torrent(&root, 'd', 64 * 1024);
        let other = torrent(&root, 'e', 64 * 1024);
        set_used(&current, 1000);
        evict(&root, 0, Some(&"d".repeat(40))).unwrap_or_else(|e| panic!("{e}"));
        assert!(current.exists());
        assert!(!other.exists());
    }

    #[test]
    fn a_missing_cache_is_empty() {
        let root = cache("missing").join("nope");
        assert_eq!(evict(&root, 0, None).unwrap_or_else(|e| panic!("{e}")), 0);
    }
}
