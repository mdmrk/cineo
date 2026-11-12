use cineo_core::app::{
    DownloadLimit, HideControls, Language, NextVideoNotice, PeerLimit, Ranged, SeekStep, Setting,
    Settings, SubtitleBackground, SubtitleColor, SubtitleFont, SubtitleOutline, TorrentStorage,
    UploadLimit, WatchedAt,
};

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SettingsView {
    pub hardware_decoding: bool,
    pub seek_step_s: u32,
    pub seek_steps_s: Vec<u32>,
    pub audio_language: Option<String>,
    pub subtitle_language: Option<String>,
    pub languages: Vec<String>,
    pub secondary_audio_language: Option<String>,
    pub secondary_subtitle_language: Option<String>,
    pub watched_at_percent: u32,
    pub watched_at_choices: Vec<u32>,
    pub allow_private_network: bool,
    pub p2p_enabled: bool,
    pub hide_controls: ChoiceView,
    pub binge_watching: ToggleView,
    pub pause_on_minimize: ToggleView,
    pub next_video_notice: ChoiceView,
    pub subtitle_size: RangeView,
    pub subtitle_font: ChoiceView,
    pub subtitle_bold: ToggleView,
    pub subtitle_color: ChoiceView,
    pub subtitle_opacity: RangeView,
    pub subtitle_outline: ChoiceView,
    pub subtitle_background: ChoiceView,
    pub subtitle_position: RangeView,
    pub keep_subtitle_styles: ToggleView,
    pub torrent_upload: ToggleView,
    pub torrent_storage: ChoiceView,
    pub download_limit: ChoiceView,
    pub upload_limit: ChoiceView,
    pub peer_limit: ChoiceView,
    pub torrent_dht: ToggleView,
    pub torrent_port: RangeView,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ChoiceView {
    pub key: String,
    pub value: String,
    pub options: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ToggleView {
    pub key: String,
    pub on: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RangeView {
    pub key: String,
    pub value: u32,
    pub min: u32,
    pub max: u32,
}

pub(super) fn settings(settings: &Settings) -> SettingsView {
    let code = |language: Option<Language>| language.map(|l| l.code().to_owned());
    SettingsView {
        hardware_decoding: settings.hardware_decoding,
        seek_step_s: settings.seek_step.whole_seconds(),
        seek_steps_s: SeekStep::ALL.iter().map(|s| s.whole_seconds()).collect(),
        audio_language: code(settings.audio_language),
        subtitle_language: code(settings.subtitle_language),
        languages: Language::all().map(|l| l.code().to_owned()).collect(),
        secondary_audio_language: code(settings.secondary_audio_language),
        secondary_subtitle_language: code(settings.secondary_subtitle_language),
        watched_at_percent: settings.watched_at.percent(),
        watched_at_choices: WatchedAt::ALL.iter().map(|w| w.percent()).collect(),
        allow_private_network: settings.allow_private_network,
        p2p_enabled: settings.p2p_enabled,
        hide_controls: choice(
            settings.hide_controls,
            HideControls::ALL,
            Setting::HideControls,
        ),
        binge_watching: toggle(settings.binge_watching, Setting::BingeWatching),
        pause_on_minimize: toggle(settings.pause_on_minimize, Setting::PauseOnMinimize),
        next_video_notice: choice(
            settings.next_video_notice,
            NextVideoNotice::ALL,
            Setting::NextVideoNotice,
        ),
        subtitle_size: range(settings.subtitle_size, Setting::SubtitleSize),
        subtitle_font: choice(
            settings.subtitle_font,
            SubtitleFont::ALL,
            Setting::SubtitleFont,
        ),
        subtitle_bold: toggle(settings.subtitle_bold, Setting::SubtitleBold),
        subtitle_color: choice(
            settings.subtitle_color,
            SubtitleColor::ALL,
            Setting::SubtitleColor,
        ),
        subtitle_opacity: range(settings.subtitle_opacity, Setting::SubtitleOpacity),
        subtitle_outline: choice(
            settings.subtitle_outline,
            SubtitleOutline::ALL,
            Setting::SubtitleOutline,
        ),
        subtitle_background: choice(
            settings.subtitle_background,
            SubtitleBackground::ALL,
            Setting::SubtitleBackground,
        ),
        subtitle_position: range(settings.subtitle_position, Setting::SubtitlePosition),
        keep_subtitle_styles: toggle(settings.keep_subtitle_styles, Setting::KeepSubtitleStyles),
        torrent_upload: toggle(settings.torrent_upload, Setting::TorrentUpload),
        torrent_storage: choice(
            settings.torrent_storage,
            TorrentStorage::ALL,
            Setting::TorrentStorage,
        ),
        download_limit: choice(
            settings.download_limit,
            DownloadLimit::ALL,
            Setting::DownloadLimit,
        ),
        upload_limit: choice(
            settings.upload_limit,
            UploadLimit::ALL,
            Setting::UploadLimit,
        ),
        peer_limit: choice(settings.peer_limit, PeerLimit::ALL, Setting::PeerLimit),
        torrent_dht: toggle(settings.torrent_dht, Setting::TorrentDht),
        torrent_port: range(settings.torrent_port, Setting::TorrentPort),
    }
}

fn choice<T: Copy>(current: T, all: &[T], wrap: fn(T) -> Setting) -> ChoiceView {
    ChoiceView {
        key: wrap(current).key().to_owned(),
        value: wrap(current).value(),
        options: all.iter().map(|&option| wrap(option).value()).collect(),
    }
}

fn toggle(on: bool, wrap: fn(bool) -> Setting) -> ToggleView {
    ToggleView {
        key: wrap(on).key().to_owned(),
        on,
    }
}

fn range<const MIN: u32, const MAX: u32>(
    current: Ranged<MIN, MAX>,
    wrap: fn(Ranged<MIN, MAX>) -> Setting,
) -> RangeView {
    RangeView {
        key: wrap(current).key().to_owned(),
        value: current.get(),
        min: MIN,
        max: MAX,
    }
}
