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
        self.map_or_else(String::new, |l| l.code().to_owned())
    }

    fn decode(text: &str) -> Option<Self> {
        if text.is_empty() {
            Some(None)
        } else {
            Language::from_code(text).map(Some)
        }
    }
}

/// A whole number kept within `MIN..=MAX`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ranged<const MIN: u32, const MAX: u32>(u32);

impl<const MIN: u32, const MAX: u32> Ranged<MIN, MAX> {
    pub const MIN: u32 = MIN;
    pub const MAX: u32 = MAX;

    /// Clamps `value` into the range.
    pub const fn new(value: u32) -> Self {
        Self(if value < MIN {
            MIN
        } else if value > MAX {
            MAX
        } else {
            value
        })
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

impl<const MIN: u32, const MAX: u32> SettingValue for Ranged<MIN, MAX> {
    fn encode(self) -> String {
        self.0.to_string()
    }

    fn decode(text: &str) -> Option<Self> {
        text.parse().ok().map(Self::new)
    }
}

/// A percentage, 0–100.
pub type Percent = Ranged<0, 100>;

macro_rules! choice {
    ($(#[$doc:meta])* $name:ident { $($variant:ident = $text:literal,)* }) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name {
            $($variant,)*
        }

        impl $name {
            pub const ALL: &[Self] = &[$(Self::$variant,)*];
        }

        impl SettingValue for $name {
            fn encode(self) -> String {
                match self {
                    $(Self::$variant => $text,)*
                }
                .to_owned()
            }

            fn decode(text: &str) -> Option<Self> {
                match text {
                    $($text => Some(Self::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

choice! {
    /// How far ←/→ seek.
    SeekStep { S5 = "5", S10 = "10", S15 = "15", S30 = "30", }
}

impl SeekStep {
    pub const fn seconds(self) -> f64 {
        match self {
            Self::S5 => 5.0,
            Self::S10 => 10.0,
            Self::S15 => 15.0,
            Self::S30 => 30.0,
        }
    }
}

choice! {
    /// How far Shift+←/→ seek.
    ShortSeekStep { S1 = "1", S3 = "3", S5 = "5", }
}

impl ShortSeekStep {
    pub const fn seconds(self) -> f64 {
        match self {
            Self::S1 => 1.0,
            Self::S3 => 3.0,
            Self::S5 => 5.0,
        }
    }
}

choice! {
    /// How long the player controls stay up without input.
    HideControls { Short = "1.5", Normal = "2.5", Long = "5", }
}

impl HideControls {
    pub const fn seconds(self) -> f64 {
        match self {
            Self::Short => 1.5,
            Self::Normal => 2.5,
            Self::Long => 5.0,
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
    /// Used when nothing is in [`Settings::subtitle_language`].
    secondary_subtitle_language: Option<Language> = None
        => SecondarySubtitleLanguage("secondary_subtitle_language"),
    /// The audio track in this language is played when the file has one.
    audio_language: Option<Language> = None => AudioLanguage("audio_language"),
    secondary_audio_language: Option<Language> = None
        => SecondaryAudioLanguage("secondary_audio_language"),
    hardware_decoding: bool = true => HardwareDecoding("hardware_decoding"),
    seek_step: SeekStep = SeekStep::S10 => SeekStep("seek_step"),
    short_seek_step: ShortSeekStep = ShortSeekStep::S3 => ShortSeekStep("short_seek_step"),
    /// Esc leaves fullscreen first; when off, it leaves the player at once.
    escape_leaves_fullscreen: bool = true => EscapeLeavesFullscreen("escape_leaves_fullscreen"),
    pause_on_minimize: bool = false => PauseOnMinimize("pause_on_minimize"),
    hide_controls: HideControls = HideControls::Normal => HideControls("hide_controls"),
    /// Playback starts at [`Settings::volume`], the volume the last one ended with.
    remember_volume: bool = true => RememberVolume("remember_volume"),
    volume: Percent = Percent::new(100) => Volume("volume"),
}

impl Settings {
    /// The preferred subtitle languages, first choice first.
    pub fn subtitle_languages(&self) -> impl Iterator<Item = Language> {
        self.subtitle_language
            .into_iter()
            .chain(self.secondary_subtitle_language)
    }

    /// The preferred audio languages, first choice first.
    pub fn audio_languages(&self) -> impl Iterator<Item = Language> {
        self.audio_language
            .into_iter()
            .chain(self.secondary_audio_language)
    }

    /// The defaults, keeping what is not a preference (the P2P notice).
    #[must_use]
    pub fn reset(self) -> Self {
        Self {
            p2p_acknowledged: self.p2p_acknowledged,
            ..Self::default()
        }
    }
}
