use super::language::Language;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ranged<const MIN: u32, const MAX: u32>(u32);

impl<const MIN: u32, const MAX: u32> Ranged<MIN, MAX> {
    pub const MIN: u32 = MIN;
    pub const MAX: u32 = MAX;

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
    pub const fn rgb(self) -> Option<[u8; 3]> {
        match self {
            Self::None => None,
            Self::Black => Some([0x00, 0x00, 0x00]),
            Self::Gray => Some([0x30, 0x30, 0x30]),
        }
    }
}

choice! {
    AudioOutput { Auto = "auto", Stereo = "stereo", }
}

choice! {
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
    NextVideoNotice { Off = "0", S10 = "10", S20 = "20", S35 = "35", S60 = "60", }
}

impl NextVideoNotice {
    pub const fn seconds(self) -> f64 {
        match self {
            Self::Off => 0.0,
            Self::S10 => 10.0,
            Self::S20 => 20.0,
            Self::S35 => 35.0,
            Self::S60 => 60.0,
        }
    }
}

choice! {
    StartPage { Home = "home", Discover = "discover", Library = "library", }
}

choice! {
    UiLanguage { System = "system", English = "en", Spanish = "es", }
}

choice! {
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

pub type SubtitleSize = Ranged<50, 200>;
pub type SubtitlePosition = Ranged<0, 20>;
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

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Setting {
            $($(#[$doc])* $variant($ty),)*
        }

        impl Setting {
            pub fn key(self) -> &'static str {
                match self {
                    $(Self::$variant(_) => $key,)*
                }
            }

            pub fn value(self) -> String {
                match self {
                    $(Self::$variant(value) => value.encode(),)*
                }
            }

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
    secondary_subtitle_language: Option<Language> = None
        => SecondarySubtitleLanguage("secondary_subtitle_language"),
    audio_language: Option<Language> = None => AudioLanguage("audio_language"),
    secondary_audio_language: Option<Language> = None
        => SecondaryAudioLanguage("secondary_audio_language"),
    hardware_decoding: bool = true => HardwareDecoding("hardware_decoding"),
    seek_step: SeekStep = SeekStep::S10 => SeekStep("seek_step"),
    short_seek_step: ShortSeekStep = ShortSeekStep::S3 => ShortSeekStep("short_seek_step"),
    escape_leaves_fullscreen: bool = true => EscapeLeavesFullscreen("escape_leaves_fullscreen"),
    pause_on_minimize: bool = false => PauseOnMinimize("pause_on_minimize"),
    hide_controls: HideControls = HideControls::Normal => HideControls("hide_controls"),
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
    keep_subtitle_styles: bool = true => KeepSubtitleStyles("keep_subtitle_styles"),
    audio_output: AudioOutput = AudioOutput::Auto => AudioOutput("audio_output"),
    audio_passthrough: bool = false => AudioPassthrough("audio_passthrough"),
    torrent_upload: bool = true => TorrentUpload("torrent_upload"),
    download_limit: DownloadLimit = DownloadLimit::Unlimited => DownloadLimit("download_limit"),
    upload_limit: UploadLimit = UploadLimit::Unlimited => UploadLimit("upload_limit"),
    peer_limit: PeerLimit = PeerLimit::P128 => PeerLimit("peer_limit"),
    torrent_dht: bool = true => TorrentDht("torrent_dht"),
    allow_private_network: bool = false => AllowPrivateNetwork("allow_private_network"),
    interface_scale: InterfaceScale = InterfaceScale::S100 => InterfaceScale("interface_scale"),
    start_page: StartPage = StartPage::Home => StartPage("start_page"),
    ui_language: UiLanguage = UiLanguage::System => UiLanguage("ui_language"),
    watched_at: WatchedAt = WatchedAt::P92 => WatchedAt("watched_at"),
    binge_watching: bool = true => BingeWatching("binge_watching"),
    next_video_notice: NextVideoNotice = NextVideoNotice::S35 => NextVideoNotice("next_video_notice"),
}

impl Settings {
    pub fn subtitle_languages(&self) -> impl Iterator<Item = Language> {
        self.subtitle_language
            .into_iter()
            .chain(self.secondary_subtitle_language)
    }

    pub fn audio_languages(&self) -> impl Iterator<Item = Language> {
        self.audio_language
            .into_iter()
            .chain(self.secondary_audio_language)
    }

    #[must_use]
    pub fn reset(self) -> Self {
        Self {
            p2p_acknowledged: self.p2p_acknowledged,
            ..Self::default()
        }
    }
}
