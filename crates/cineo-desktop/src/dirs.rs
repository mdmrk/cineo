use fastframe_dirs::{App, Dirs};

const APP: App = App::new("cineo", "", "", "Cineo");

pub fn installed() -> Option<Dirs> {
    APP.installed()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

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
