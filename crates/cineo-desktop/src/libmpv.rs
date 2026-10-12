//! The libmpv inside Windows release builds (ADR-0014, amended 2026-10-12),
//! written to the cache directory so the player can load it.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[cfg(feature = "bundled-libmpv")]
const LIBMPV: &[u8] = include_bytes!(env!("CINEO_LIBMPV_DLL"));

/// Writes the bundled libmpv under `cache_dir` and hands its path to the
/// player. Does nothing in builds without the `bundled-libmpv` feature.
pub fn install(cache_dir: &Path) {
    #[cfg(feature = "bundled-libmpv")]
    match extract(
        &cache_dir.join("libmpv"),
        env!("CARGO_PKG_VERSION"),
        "libmpv-2.dll",
        LIBMPV,
    ) {
        Ok(path) => {
            tracing::debug!(path = %path.display(), "bundled libmpv ready");
            cineo_player::embedded::use_bundled_libmpv(path);
        }
        Err(err) => tracing::warn!(%err, "could not write the bundled libmpv"),
    }
    #[cfg(not(feature = "bundled-libmpv"))]
    let _ = cache_dir;
}

#[cfg_attr(
    not(feature = "bundled-libmpv"),
    allow(dead_code, reason = "only builds that bundle libmpv call it")
)]
fn extract(root: &Path, version: &str, name: &str, bytes: &[u8]) -> io::Result<PathBuf> {
    let dir = root.join(version);
    let path = dir.join(name);
    let current = std::fs::metadata(&path).is_ok_and(|meta| meta.len() == bytes.len() as u64);
    if !current {
        std::fs::create_dir_all(&dir)?;
        let partial = dir.join(format!("{name}.partial"));
        let mut file = std::fs::File::create(&partial)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&partial, &path)?;
    }
    remove_other_versions(root, version);
    Ok(path)
}

fn remove_other_versions(root: &Path, keep: &str) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_name() != keep && entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cineo-libmpv-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn writes_the_library_under_its_version() {
        let root = scratch("write");
        let path = extract(&root, "1.0.0", "libmpv-2.dll", b"library").unwrap();
        assert_eq!(path, root.join("1.0.0").join("libmpv-2.dll"));
        assert_eq!(std::fs::read(&path).unwrap(), b"library");
        assert!(!root.join("1.0.0").join("libmpv-2.dll.partial").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_copy_of_the_wrong_size_is_replaced() {
        let root = scratch("replace");
        std::fs::create_dir_all(root.join("1.0.0")).unwrap();
        std::fs::write(root.join("1.0.0").join("libmpv-2.dll"), b"lib").unwrap();
        let path = extract(&root, "1.0.0", "libmpv-2.dll", b"library").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"library");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn other_versions_are_removed() {
        let root = scratch("versions");
        let old = extract(&root, "1.0.0", "libmpv-2.dll", b"old").unwrap();
        let new = extract(&root, "1.1.0", "libmpv-2.dll", b"new").unwrap();
        assert!(!old.exists());
        assert_eq!(std::fs::read(&new).unwrap(), b"new");
        let _ = std::fs::remove_dir_all(&root);
    }
}
