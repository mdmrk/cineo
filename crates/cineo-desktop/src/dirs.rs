use std::path::Path;

use fastframe_dirs::{App, Dirs};

const APP: App = App::new("cineo", "", "", "Cineo");

pub fn installed() -> Option<Dirs> {
    APP.installed()
}

pub const PORTABLE_MARKER: &str = "portable.txt";

pub fn is_portable(flag: bool, cwd: &Path) -> bool {
    flag || cwd.join(PORTABLE_MARKER).is_file()
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn the_flag_or_a_marker_file_in_the_folder_makes_it_portable() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_portable(false, dir.path()));
        assert!(is_portable(true, dir.path()));
        std::fs::create_dir(dir.path().join(PORTABLE_MARKER)).unwrap();
        assert!(!is_portable(false, dir.path()));
        std::fs::remove_dir(dir.path().join(PORTABLE_MARKER)).unwrap();
        std::fs::write(dir.path().join(PORTABLE_MARKER), "").unwrap();
        assert!(is_portable(false, dir.path()));
    }

    #[cfg(target_os = "linux")]
    fn base(variable: &str, fallback: &str) -> Option<PathBuf> {
        std::env::var_os(variable)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(fallback)))
    }

    #[test]
    fn the_folders_are_named_after_the_app() {
        let Some(dirs) = installed() else {
            return;
        };
        let name = if cfg!(target_os = "linux") {
            "cineo"
        } else {
            "Cineo"
        };
        assert!(
            dirs.cache.components().any(|c| c.as_os_str() == name),
            "{dirs:?}"
        );
        assert!(
            dirs.state.components().any(|c| c.as_os_str() == name),
            "{dirs:?}"
        );
        assert_ne!(dirs.state, dirs.cache);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn data_goes_to_the_xdg_state_folder_and_cache_to_the_xdg_cache_folder() {
        let Some(dirs) = installed() else {
            return;
        };
        assert_eq!(
            Some(dirs.state),
            base("XDG_STATE_HOME", ".local/state").map(|b| b.join("cineo"))
        );
        assert_eq!(
            Some(dirs.cache),
            base("XDG_CACHE_HOME", ".cache").map(|b| b.join("cineo"))
        );
    }
}
