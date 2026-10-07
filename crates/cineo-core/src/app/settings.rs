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

choice! {
    /// The subtitle typeface, by generic family.
    SubtitleFont { Sans = "sans", Serif = "serif", Mono = "mono", }
}

choice! {
    SubtitleColor { White = "white", Yellow = "yellow", Cyan = "cyan", Green = "green", }
}

choice! {
    SubtitleOutline { Black = "black", Gray = "gray", None = "none", }
}

choice! {
    SubtitleBackground { None = "none", Black = "black", Gray = "gray", }
}

impl SubtitleColor {
    pub const fn rgb(self) -> [u8; 3] {
        match self {
            Self::White => [0xFF, 0xFF, 0xFF],
            Self::Yellow => [0xFF, 0xE4, 0x5C],
            Self::Cyan => [0x5C, 0xE1, 0xFF],
            Self::Green => [0x7C, 0xFF, 0x7C],
        }
    }
}

impl SubtitleOutline {
    pub const fn rgb(self) -> Option<[u8; 3]> {
        match self {
            Self::Black => Some([0x00, 0x00, 0x00]),
            Self::Gray => Some([0x40, 0x40, 0x40]),
            Self::None => None,
        }
    }
}

impl SubtitleBackground {
    /// Drawn at 80 % opacity.
    pub const fn rgb(self) -> Option<[u8; 3]> {
        match self {
            Self::None => None,
            Self::Black => Some([0x00, 0x00, 0x00]),
            Self::Gray => Some([0x30, 0x30, 0x30]),
        }
    }
}

choice! {
    /// Speaker layout mpv decodes to.
    AudioOutput { Auto = "auto", Stereo = "stereo", }
}

choice! {
    /// Most a torrent may download per second, in MB/s.
    DownloadLimit { Unlimited = "none", M1 = "1", M2 = "2", M5 = "5", M10 = "10", M20 = "20", }
}

impl DownloadLimit {
    pub const fn bytes_per_second(self) -> Option<u32> {
        match self {
            Self::Unlimited => None,
            Self::M1 => Some(1_000_000),
            Self::M2 => Some(2_000_000),
            Self::M5 => Some(5_000_000),
            Self::M10 => Some(10_000_000),
            Self::M20 => Some(20_000_000),
        }
    }
}

choice! {
    /// Most a torrent may upload per second.
    UploadLimit { Unlimited = "none", K100 = "100k", K500 = "500k", M1 = "1", M5 = "5", }
}

impl UploadLimit {
    pub const fn bytes_per_second(self) -> Option<u32> {
        match self {
            Self::Unlimited => None,
            Self::K100 => Some(100_000),
            Self::K500 => Some(500_000),
            Self::M1 => Some(1_000_000),
            Self::M5 => Some(5_000_000),
        }
    }
}

choice! {
    /// Most peers one torrent connects to.
    PeerLimit { P50 = "50", P128 = "128", P200 = "200", }
}

impl PeerLimit {
    pub const fn peers(self) -> usize {
        match self {
            Self::P50 => 50,
            Self::P128 => 128,
            Self::P200 => 200,
        }
    }
}

choice! {
    /// The UI size, in percent.
    InterfaceScale {
        S75 = "75", S90 = "90", S100 = "100", S110 = "110", S125 = "125", S150 = "150",
        S175 = "175", S200 = "200",
    }
}

impl InterfaceScale {
    pub const fn percent(self) -> u32 {
        match self {
            Self::S75 => 75,
            Self::S90 => 90,
            Self::S100 => 100,
            Self::S110 => 110,
            Self::S125 => 125,
            Self::S150 => 150,
            Self::S175 => 175,
            Self::S200 => 200,
        }
    }
}

choice! {
    /// The page shown when Cineo opens.
    StartPage { Home = "home", Discover = "discover", Library = "library", }
}

choice! {
    /// How much of a video must have played for it to count as watched.
    WatchedAt { P80 = "80", P85 = "85", P90 = "90", P92 = "92", P95 = "95", }
}

impl WatchedAt {
    pub const fn fraction(self) -> f64 {
        match self {
            Self::P80 => 0.80,
            Self::P85 => 0.85,
            Self::P90 => 0.90,
            Self::P92 => 0.92,
            Self::P95 => 0.95,
        }
    }
}

/// Subtitle size in percent of the player's default.
pub type SubtitleSize = Ranged<50, 200>;
/// How far above the bottom edge subtitles sit, in percent of the height.
pub type SubtitlePosition = Ranged<0, 20>;
/// Subtitle text opacity in percent; never fully transparent.
pub type SubtitleOpacity = Ranged<25, 100>;

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
    subtitle_size: SubtitleSize = SubtitleSize::new(100) => SubtitleSize("subtitle_size"),
    subtitle_font: SubtitleFont = SubtitleFont::Sans => SubtitleFont("subtitle_font"),
    subtitle_bold: bool = false => SubtitleBold("subtitle_bold"),
    subtitle_position: SubtitlePosition = SubtitlePosition::new(0)
        => SubtitlePosition("subtitle_position"),
    subtitle_color: SubtitleColor = SubtitleColor::White => SubtitleColor("subtitle_color"),
    subtitle_outline: SubtitleOutline = SubtitleOutline::Black
        => SubtitleOutline("subtitle_outline"),
    subtitle_background: SubtitleBackground = SubtitleBackground::None
        => SubtitleBackground("subtitle_background"),
    subtitle_opacity: SubtitleOpacity = SubtitleOpacity::new(100)
        => SubtitleOpacity("subtitle_opacity"),
    /// Styled (ASS) subtitles keep their own look; when off, the subtitle
    /// settings above apply to them too.
    keep_subtitle_styles: bool = true => KeepSubtitleStyles("keep_subtitle_styles"),
    audio_output: AudioOutput = AudioOutput::Auto => AudioOutput("audio_output"),
    /// Compressed surround audio goes to the receiver undecoded.
    audio_passthrough: bool = false => AudioPassthrough("audio_passthrough"),
    /// Pieces already downloaded are shared with other peers.
    torrent_upload: bool = true => TorrentUpload("torrent_upload"),
    download_limit: DownloadLimit = DownloadLimit::Unlimited => DownloadLimit("download_limit"),
    upload_limit: UploadLimit = UploadLimit::Unlimited => UploadLimit("upload_limit"),
    peer_limit: PeerLimit = PeerLimit::P128 => PeerLimit("peer_limit"),
    /// Find peers through the DHT, not only through trackers.
    torrent_dht: bool = true => TorrentDht("torrent_dht"),
    /// Addons, images and torrent peers on loopback/LAN addresses are
    /// allowed (docs/SECURITY.md). Read at startup.
    allow_private_network: bool = false => AllowPrivateNetwork("allow_private_network"),
    interface_scale: InterfaceScale = InterfaceScale::S100 => InterfaceScale("interface_scale"),
    start_page: StartPage = StartPage::Home => StartPage("start_page"),
    /// Watched videos restart from the beginning and leave Continue Watching.
    watched_at: WatchedAt = WatchedAt::P92 => WatchedAt("watched_at"),
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
