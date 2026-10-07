//! Addon subtitle files for the current playback: fetched through
//! `cineo-net`, written to a private directory for mpv, and deleted when the
//! playback ends (SECURITY.md §Subtitles).

use std::collections::HashMap;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use cineo_core::app::SubtitleGroup;
use cineo_player::embedded::{Track, TrackKind};
use tracing::warn;
use url::Url;

use crate::player::AddonSubtitle;

pub(crate) struct SubtitleFiles {
    dir: PathBuf,
    files: HashMap<Url, PathBuf>,
    written: u64,
}

impl SubtitleFiles {
    /// Starts empty, deleting files a previous run left in `dir`.
    pub(crate) fn new(dir: PathBuf) -> Self {
        remove_dir(&dir);
        Self {
            dir,
            files: HashMap::new(),
            written: 0,
        }
    }

    pub(crate) fn get(&self, url: &Url) -> Option<&Path> {
        self.files.get(url).map(PathBuf::as_path)
    }

    pub(crate) fn write(&mut self, url: &Url, bytes: &[u8]) -> io::Result<PathBuf> {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder.create(&self.dir)?;
        self.written += 1;
        let path = self.dir.join(self.written.to_string());
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        options.open(&path)?.write_all(bytes)?;
        self.files.insert(url.clone(), path.clone());
        Ok(path)
    }

    /// The menu entries: every ready addon subtitle once, in group order.
    pub(crate) fn entries(&self, groups: &[SubtitleGroup], tracks: &[Track]) -> Vec<AddonSubtitle> {
        let selected_file = tracks
            .iter()
            .find(|t| t.kind == TrackKind::Subtitle && t.selected)
            .and_then(|t| t.external_file.as_deref());
        let mut seen = std::collections::HashSet::new();
        groups
            .iter()
            .filter_map(|g| g.subtitles.ready().map(|s| (g, s)))
            .flat_map(|(g, subtitles)| subtitles.iter().map(move |s| (g, s)))
            .filter(|(_, s)| seen.insert(&s.url))
            .map(|(g, s)| AddonSubtitle {
                url: s.url.clone(),
                lang: s.lang.clone(),
                label: s.label.clone(),
                addon_name: g.addon_name.clone(),
                selected: selected_file.is_some()
                    && self.get(&s.url).and_then(Path::to_str) == selected_file,
            })
            .collect()
    }
}

impl Drop for SubtitleFiles {
    fn drop(&mut self) {
        remove_dir(&self.dir);
    }
}

fn remove_dir(dir: &Path) {
    match std::fs::remove_dir_all(dir) {
        Err(err) if err.kind() != io::ErrorKind::NotFound => {
            warn!(%err, "could not delete subtitle files");
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use cineo_core::addon::{Subtitle, TransportUrl};
    use cineo_core::app::Loadable;

    use super::*;

    fn dir(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("cineo-subtitles-{}", std::process::id()))
            .join(name)
    }

    fn url(s: &str) -> Url {
        s.parse().unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn files_are_private_and_deleted_on_drop() {
        let dir = dir("drop");
        let mut files = SubtitleFiles::new(dir.clone());
        let path = files
            .write(&url("https://s.example/a.srt"), b"x")
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            files.get(&url("https://s.example/a.srt")),
            Some(path.as_path())
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = |p: &Path| {
                std::fs::metadata(p)
                    .map(|m| m.permissions().mode() & 0o777)
                    .unwrap_or_else(|e| panic!("{e}"))
            };
            assert_eq!(mode(&dir), 0o700);
            assert_eq!(mode(&path), 0o600);
        }
        let again = |files: &mut SubtitleFiles, u: &str| {
            files.write(&url(u), b"y").unwrap_or_else(|e| panic!("{e}"))
        };
        again(&mut files, "https://s.example/a.srt");
        again(&mut files, "https://s.example/b.srt");
        drop(files);
        assert!(!dir.exists());
    }

    #[test]
    fn entries_list_each_url_once_and_mark_the_loaded_selected_one() {
        let mut files = SubtitleFiles::new(dir("entries"));
        let sub = |u: &str, lang: &str| Subtitle {
            id: u.into(),
            url: url(u),
            lang: lang.into(),
            label: None,
        };
        let group = |name: &str, subtitles| SubtitleGroup {
            addon: TransportUrl::parse("https://a.example/manifest.json")
                .unwrap_or_else(|e| panic!("{e}")),
            addon_name: name.into(),
            path: None,
            subtitles,
        };
        let groups = [
            group(
                "Stream",
                Loadable::Ready(vec![sub("https://s.example/1", "eng")]),
            ),
            group(
                "Subs",
                Loadable::Ready(vec![
                    sub("https://s.example/1", "eng"),
                    sub("https://s.example/2", "spa"),
                ]),
            ),
            group("Slow", Loadable::Loading),
        ];
        let path = files
            .write(&url("https://s.example/2"), b"x")
            .unwrap_or_else(|e| panic!("{e}"));
        let tracks = [Track {
            id: 3,
            kind: TrackKind::Subtitle,
            title: None,
            lang: None,
            selected: true,
            external_file: path.to_str().map(str::to_owned),
        }];
        let entries = files.entries(&groups, &tracks);
        let summary: Vec<_> = entries
            .iter()
            .map(|e| (e.addon_name.as_str(), e.lang.as_str(), e.selected))
            .collect();
        assert_eq!(summary, [("Stream", "eng", false), ("Subs", "spa", true)]);
    }
}
