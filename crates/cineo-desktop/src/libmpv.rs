use std::io;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;

#[cfg(feature = "bundled-libmpv")]
const LIBMPV_GZ: &[u8] = include_bytes!(env!("CINEO_LIBMPV_DLL_GZ"));

pub fn install(cache_dir: &Path) {
    #[cfg(feature = "bundled-libmpv")]
    match extract(&cache_dir.join("libmpv"), "libmpv-2.dll", LIBMPV_GZ) {
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
fn extract(root: &Path, name: &str, gz: &[u8]) -> io::Result<PathBuf> {
    let Some(&[c0, c1, c2, c3, l0, l1, l2, l3]) = gz.last_chunk::<8>() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the bundled libmpv is not gzip",
        ));
    };
    let key = format!("{:08x}", u32::from_le_bytes([c0, c1, c2, c3]));
    let size = u64::from(u32::from_le_bytes([l0, l1, l2, l3]));
    let path = root.join(&key).join(name);
    let current = std::fs::metadata(&path).is_ok_and(|meta| meta.len() == size);
    if !current {
        fastframe_file::write_with(&path, fastframe_file::Access::Default, |file| {
            io::copy(&mut GzDecoder::new(gz), file).map(drop)
        })?;
    }
    remove_other_versions(root, &key);
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

    use std::io::Write;

    use flate2::Compression;
    use flate2::write::GzEncoder;

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cineo-libmpv-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn gz(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn writes_the_library_under_its_checksum() {
        let root = scratch("write");
        let path = extract(&root, "libmpv-2.dll", &gz(b"library")).unwrap();
        assert_eq!(path.file_name().unwrap(), "libmpv-2.dll");
        assert_eq!(path.parent().unwrap().parent().unwrap(), root);
        assert_eq!(std::fs::read(&path).unwrap(), b"library");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_copy_of_the_wrong_size_is_replaced() {
        let root = scratch("replace");
        let path = extract(&root, "libmpv-2.dll", &gz(b"library")).unwrap();
        std::fs::write(&path, b"lib").unwrap();
        let again = extract(&root, "libmpv-2.dll", &gz(b"library")).unwrap();
        assert_eq!(again, path);
        assert_eq!(std::fs::read(&path).unwrap(), b"library");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn another_library_of_the_same_size_replaces_the_old_one() {
        let root = scratch("same-size");
        let old = extract(&root, "libmpv-2.dll", &gz(b"library-1")).unwrap();
        let new = extract(&root, "libmpv-2.dll", &gz(b"library-2")).unwrap();
        assert!(!old.exists());
        assert_eq!(std::fs::read(&new).unwrap(), b"library-2");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_damaged_archive_writes_nothing() {
        let root = scratch("damaged");
        let mut archive = gz(b"library");
        let at = archive.len() - 9;
        archive[at] ^= 0xff;
        assert!(extract(&root, "libmpv-2.dll", &archive).is_err());
        let written = std::fs::read_dir(&root)
            .into_iter()
            .flatten()
            .flatten()
            .flat_map(|dir| {
                std::fs::read_dir(dir.path())
                    .into_iter()
                    .flatten()
                    .flatten()
            })
            .count();
        assert_eq!(written, 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn not_gzip_is_rejected() {
        assert!(extract(&scratch("short"), "libmpv-2.dll", b"lib").is_err());
    }
}
