use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use cineo_net::MAX_IMAGE_BYTES;
use cineo_store::create_private_dir;

const MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const MAX_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct ImageDisk {
    dir: PathBuf,
}

impl ImageDisk {
    pub(crate) fn new(cache_dir: &Path) -> Self {
        Self {
            dir: cache_dir.join("images"),
        }
    }

    fn path(&self, url: &str) -> PathBuf {
        let mut hasher = DefaultHasher::new();
        url.hash(&mut hasher);
        self.dir.join(format!("{:016x}", hasher.finish()))
    }

    pub(crate) fn read(&self, url: &str) -> Option<Vec<u8>> {
        let path = self.path(url);
        let metadata = std::fs::metadata(&path).ok()?;
        if is_expired(metadata.modified().ok()?, SystemTime::now()) {
            self.remove(url);
            return None;
        }
        if usize::try_from(metadata.len()).map_or(true, |len| len > MAX_IMAGE_BYTES) {
            return None;
        }
        std::fs::read(&path).ok()
    }

    pub(crate) fn write(&self, url: &str, bytes: &[u8]) {
        let saved = create_private_dir(&self.dir)
            .and_then(|()| fastframe_file::write_private(&self.path(url), bytes));
        if let Err(err) = saved {
            tracing::debug!(%err, "the image was not cached");
        }
    }

    pub(crate) fn remove(&self, url: &str) {
        let _ = std::fs::remove_file(self.path(url));
    }

    pub(crate) fn trim(&self) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        let files = entries
            .flatten()
            .filter_map(|entry| {
                let metadata = entry.metadata().ok()?;
                metadata.is_file().then_some(())?;
                Some((entry.path(), metadata.modified().ok()?, metadata.len()))
            })
            .collect();
        for path in to_remove(files, SystemTime::now(), MAX_BYTES) {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn is_expired(modified: SystemTime, now: SystemTime) -> bool {
    now.duration_since(modified).is_ok_and(|age| age >= MAX_AGE)
}

fn to_remove<T>(mut files: Vec<(T, SystemTime, u64)>, now: SystemTime, budget: u64) -> Vec<T> {
    files.sort_by_key(|&(_, modified, _)| std::cmp::Reverse(modified));
    let mut kept = 0u64;
    files
        .into_iter()
        .filter_map(|(file, modified, len)| {
            if is_expired(modified, now) || kept.saturating_add(len) > budget {
                return Some(file);
            }
            kept += len;
            None
        })
        .collect()
}

#[cfg(test)]
mod tests {

    use std::fs::OpenOptions;

    use super::*;

    const URL: &str = "https://img.example/poster.jpg?token=secret";

    fn age(disk: &ImageDisk, url: &str, by: Duration) {
        OpenOptions::new()
            .write(true)
            .open(disk.path(url))
            .unwrap()
            .set_modified(SystemTime::now() - by)
            .unwrap();
    }

    #[test]
    fn an_image_comes_back_under_a_name_that_hides_its_url() {
        let dir = tempfile::tempdir().unwrap();
        let disk = ImageDisk::new(dir.path());
        assert_eq!(disk.read(URL), None);
        disk.write(URL, b"bytes");
        assert_eq!(disk.read(URL).unwrap(), b"bytes");
        let name = std::fs::read_dir(dir.path().join("images"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .file_name();
        assert!(!name.to_string_lossy().contains("secret"));
    }

    #[cfg(unix)]
    #[test]
    fn cached_images_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let disk = ImageDisk::new(dir.path());
        disk.write(URL, b"bytes");
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&dir.path().join("images")), 0o700);
        assert_eq!(mode(&disk.path(URL)), 0o600);
    }

    #[test]
    fn an_expired_image_is_dropped_on_reading() {
        let dir = tempfile::tempdir().unwrap();
        let disk = ImageDisk::new(dir.path());
        disk.write(URL, b"bytes");
        age(&disk, URL, MAX_AGE);
        assert_eq!(disk.read(URL), None);
        assert!(!disk.path(URL).exists());
    }

    #[test]
    fn reading_an_image_does_not_extend_its_life() {
        let dir = tempfile::tempdir().unwrap();
        let disk = ImageDisk::new(dir.path());
        disk.write(URL, b"bytes");
        age(&disk, URL, MAX_AGE / 2);
        disk.read(URL).unwrap();
        let modified = std::fs::metadata(disk.path(URL))
            .unwrap()
            .modified()
            .unwrap();
        assert!(modified.elapsed().unwrap() > MAX_AGE / 4);
    }

    #[test]
    fn the_oldest_images_go_first_when_over_the_budget() {
        let now = SystemTime::now();
        let ago = |seconds| now - Duration::from_secs(seconds);
        let files = vec![
            ("oldest", ago(30), 10),
            ("newest", ago(10), 10),
            ("middle", ago(20), 10),
        ];
        assert_eq!(to_remove(files, now, 20), vec!["oldest"]);
    }

    #[test]
    fn expired_images_go_even_within_the_budget() {
        let now = SystemTime::now();
        let files = vec![("old", now - MAX_AGE, 1), ("new", now, 1)];
        assert_eq!(to_remove(files, now, 100), vec!["old"]);
    }

    #[test]
    fn trimming_deletes_expired_files_and_tolerates_a_missing_folder() {
        let dir = tempfile::tempdir().unwrap();
        let disk = ImageDisk::new(dir.path());
        disk.trim();
        disk.write(URL, b"old");
        disk.write("https://img.example/new.jpg", b"new");
        age(&disk, URL, MAX_AGE);
        disk.trim();
        assert!(!disk.path(URL).exists());
        assert!(disk.path("https://img.example/new.jpg").exists());
    }
}
