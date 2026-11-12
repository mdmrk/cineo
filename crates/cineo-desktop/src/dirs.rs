use std::ffi::OsString;
use std::path::{Path, PathBuf};

use cineo_store::DB_FILE;
use fastframe_dirs::{App, Dirs};

const APP: App = App::new("cineo", "", "", "Cineo");

pub fn installed() -> Option<Dirs> {
    APP.installed()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeftBehind {
    pub earlier: PathBuf,
    pub current: PathBuf,
}

pub fn earlier_data_dir() -> Option<PathBuf> {
    earlier_from(|name| std::env::var_os(name))
}

fn earlier_from(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let absolute = |name| var(name).map(PathBuf::from).filter(|p| p.is_absolute());
    if cfg!(windows) {
        absolute("APPDATA").map(|dir| dir.join("Cineo").join("data"))
    } else if cfg!(target_os = "linux") {
        absolute("XDG_DATA_HOME")
            .or_else(|| absolute("HOME").map(|home| home.join(".local").join("share")))
            .map(|dir| dir.join("cineo"))
    } else {
        None
    }
}

pub fn left_behind(current: &Path, earlier: &Path) -> Option<LeftBehind> {
    (current != earlier && !current.join(DB_FILE).exists() && earlier.join(DB_FILE).is_file()).then(
        || LeftBehind {
            earlier: earlier.to_path_buf(),
            current: current.to_path_buf(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn a_database_in_the_earlier_folder_is_found_only_while_the_new_folder_has_none() {
        let root = tempfile::tempdir().unwrap();
        let (earlier, current) = (root.path().join("share"), root.path().join("state"));
        assert_eq!(left_behind(&current, &earlier), None);
        std::fs::create_dir_all(&earlier).unwrap();
        std::fs::write(earlier.join(DB_FILE), b"").unwrap();
        let found = Some(LeftBehind {
            earlier: earlier.clone(),
            current: current.clone(),
        });
        assert_eq!(left_behind(&current, &earlier), found);
        std::fs::create_dir_all(&current).unwrap();
        assert_eq!(left_behind(&current, &earlier), found);
        std::fs::write(current.join(DB_FILE), b"").unwrap();
        assert_eq!(left_behind(&current, &earlier), None);
        assert_eq!(left_behind(&earlier, &earlier), None);
    }

    fn vars(vars: &'static [(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        move |name| {
            vars.iter()
                .find(|(n, _)| *n == name)
                .map(|(_, value)| OsString::from(value))
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn earlier_versions_kept_data_in_the_xdg_data_folder() {
        assert_eq!(
            earlier_from(vars(&[("HOME", "/home/u")])),
            Some(PathBuf::from("/home/u/.local/share/cineo"))
        );
        assert_eq!(
            earlier_from(vars(&[("HOME", "/home/u"), ("XDG_DATA_HOME", "/data")])),
            Some(PathBuf::from("/data/cineo"))
        );
        assert_eq!(
            earlier_from(vars(&[("HOME", "/home/u"), ("XDG_DATA_HOME", "data")])),
            Some(PathBuf::from("/home/u/.local/share/cineo"))
        );
        assert_eq!(earlier_from(vars(&[])), None);
    }

    #[cfg(windows)]
    #[test]
    fn earlier_versions_kept_data_in_the_roaming_app_data_folder() {
        assert_eq!(
            earlier_from(vars(&[("APPDATA", r"C:\Users\u\AppData\Roaming")])),
            Some(PathBuf::from(r"C:\Users\u\AppData\Roaming\Cineo\data"))
        );
        assert_eq!(earlier_from(vars(&[])), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn no_earlier_version_ran_on_macos() {
        assert_eq!(earlier_from(vars(&[("HOME", "/Users/u")])), None);
    }
}
