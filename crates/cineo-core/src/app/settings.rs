//! User settings: typed values with their defaults, one [`Setting`] per
//! change, and the key/value text each is saved as.

use super::language::Language;

/// Why a saved key/value pair was not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SettingError {
    #[error("unknown setting")]
    UnknownKey,
    #[error("unreadable setting value")]
    Unreadable,
}

trait SettingValue: Sized {
    fn encode(self) -> String;
    fn decode(text: &str) -> Option<Self>;
}

impl SettingValue for bool {
    fn encode(self) -> String {
        self.to_string()
    }

    fn decode(text: &str) -> Option<Self> {
        text.parse().ok()
    }
}

impl SettingValue for Option<Language> {
    fn encode(self) -> String {
        self.map_or_else(String::new, |l| l.code.to_owned())
    }

    fn decode(text: &str) -> Option<Self> {
        if text.is_empty() {
            Some(None)
        } else {
            Language::from_code(text).map(Some)
        }
    }
}

macro_rules! settings {
    ($(
        $(#[$doc:meta])*
        $field:ident: $ty:ty = $default:expr => $variant:ident($key:literal),
    )*) => {
        /// User settings. Every field holds a valid value by its type.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct Settings {
            $($(#[$doc])* pub $field: $ty,)*
        }

        impl Default for Settings {
            fn default() -> Self {
                Self { $($field: $default,)* }
            }
        }

        /// One setting with its new value.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Setting {
            $($(#[$doc])* $variant($ty),)*
        }

        impl Setting {
            /// The key the setting is saved under. Never changes.
            pub fn key(self) -> &'static str {
                match self {
                    $(Self::$variant(_) => $key,)*
                }
            }

            /// The value as saved.
            pub fn value(self) -> String {
                match self {
                    $(Self::$variant(value) => value.encode(),)*
                }
            }

            /// Reads a saved pair.
            pub fn parse(key: &str, value: &str) -> Result<Self, SettingError> {
                match key {
                    $($key => <$ty as SettingValue>::decode(value)
                        .map(Self::$variant)
                        .ok_or(SettingError::Unreadable),)*
                    _ => Err(SettingError::UnknownKey),
                }
            }
        }

        impl Settings {
            /// Every setting with its current value.
            pub fn all(&self) -> Vec<Setting> {
                vec![$(Setting::$variant(self.$field),)*]
            }

            pub fn set(&mut self, setting: Setting) {
                match setting {
                    $(Setting::$variant(value) => self.$field = value,)*
                }
            }
        }
    };
}

settings! {
    /// Torrent streams are shown and playable. When off, nothing
    /// torrent-related runs.
    p2p_enabled: bool = true => P2pEnabled("p2p_enabled"),
    /// The user has seen and accepted the P2P notice. Until then, playing a
    /// torrent asks first.
    p2p_acknowledged: bool = false => P2pAcknowledged("p2p_acknowledged"),
    /// Subtitles in this language are selected when a playback starts.
    subtitle_language: Option<Language> = None => SubtitleLanguage("subtitle_language"),
}

impl Settings {
    /// The defaults, keeping what is not a preference (the P2P notice).
    #[must_use]
    pub fn reset(self) -> Self {
        Self {
            p2p_acknowledged: self.p2p_acknowledged,
            ..Self::default()
        }
    }
}
