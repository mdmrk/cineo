#![allow(clippy::unwrap_used, reason = "test helpers panic on purpose")]

use cineo_core::app::{
    AudioOutput, DownloadLimit, HideControls, InterfaceScale, Language, PeerLimit, Percent,
    SeekStep, Setting, SettingError, Settings, ShortSeekStep, StartPage, SubtitleBackground,
    SubtitleColor, SubtitleFont, SubtitleOpacity, SubtitleOutline, SubtitlePosition, SubtitleSize,
    UploadLimit, WatchedAt,
};

#[test]
fn every_setting_reads_back_what_it_saves() {
    let por = Language::from_code("por");
    let mut settings = Settings::default();
    for setting in [
        Setting::P2pEnabled(false),
        Setting::P2pAcknowledged(true),
        Setting::SubtitleLanguage(por),
        Setting::SecondarySubtitleLanguage(por),
        Setting::AudioLanguage(por),
        Setting::SecondaryAudioLanguage(por),
        Setting::HardwareDecoding(false),
        Setting::SeekStep(SeekStep::S30),
        Setting::ShortSeekStep(ShortSeekStep::S1),
        Setting::EscapeLeavesFullscreen(false),
        Setting::PauseOnMinimize(true),
        Setting::HideControls(HideControls::Long),
        Setting::RememberVolume(false),
        Setting::Volume(Percent::new(35)),
        Setting::SubtitleSize(SubtitleSize::new(150)),
        Setting::SubtitleFont(SubtitleFont::Mono),
        Setting::SubtitleBold(true),
        Setting::SubtitlePosition(SubtitlePosition::new(10)),
        Setting::SubtitleColor(SubtitleColor::Yellow),
        Setting::SubtitleOutline(SubtitleOutline::None),
        Setting::SubtitleBackground(SubtitleBackground::Black),
        Setting::SubtitleOpacity(SubtitleOpacity::new(60)),
        Setting::KeepSubtitleStyles(false),
        Setting::AudioOutput(AudioOutput::Stereo),
        Setting::AudioPassthrough(true),
        Setting::TorrentUpload(false),
        Setting::DownloadLimit(DownloadLimit::M5),
        Setting::UploadLimit(UploadLimit::K100),
        Setting::PeerLimit(PeerLimit::P50),
        Setting::TorrentDht(false),
        Setting::AllowPrivateNetwork(true),
        Setting::InterfaceScale(InterfaceScale::S125),
        Setting::StartPage(StartPage::Library),
        Setting::WatchedAt(WatchedAt::P80),
    ] {
        settings.set(setting);
    }
    assert!(
        settings
            .all()
            .iter()
            .zip(Settings::default().all())
            .all(|(changed, default)| *changed != default),
        "every setting differs from its default"
    );
    for setting in Settings::default().all().into_iter().chain(settings.all()) {
        assert_eq!(
            Setting::parse(setting.key(), &setting.value()),
            Ok(setting),
            "{}",
            setting.key()
        );
    }
}

#[test]
fn saved_keys_never_change() {
    let keys: Vec<_> = Settings::default()
        .all()
        .into_iter()
        .map(Setting::key)
        .collect();
    assert_eq!(
        keys,
        [
            "p2p_enabled",
            "p2p_acknowledged",
            "subtitle_language",
            "secondary_subtitle_language",
            "audio_language",
            "secondary_audio_language",
            "hardware_decoding",
            "seek_step",
            "short_seek_step",
            "escape_leaves_fullscreen",
            "pause_on_minimize",
            "hide_controls",
            "remember_volume",
            "volume",
            "subtitle_size",
            "subtitle_font",
            "subtitle_bold",
            "subtitle_position",
            "subtitle_color",
            "subtitle_outline",
            "subtitle_background",
            "subtitle_opacity",
            "keep_subtitle_styles",
            "audio_output",
            "audio_passthrough",
            "torrent_upload",
            "download_limit",
            "upload_limit",
            "peer_limit",
            "torrent_dht",
            "allow_private_network",
            "interface_scale",
            "start_page",
            "watched_at",
        ]
    );
    assert_eq!(Setting::SubtitleLanguage(None).value(), "");
}

#[test]
fn unknown_keys_and_unreadable_values_are_told_apart() {
    assert_eq!(
        Setting::parse("from_the_future", "1"),
        Err(SettingError::UnknownKey)
    );
    assert_eq!(
        Setting::parse("p2p_enabled", "maybe"),
        Err(SettingError::Unreadable)
    );
    assert_eq!(
        Setting::parse("subtitle_language", "klingon"),
        Err(SettingError::Unreadable)
    );
}

#[test]
fn numbers_are_kept_in_range() {
    assert_eq!(Percent::new(250).get(), 100);
    assert_eq!(
        Setting::parse("volume", "250"),
        Ok(Setting::Volume(Percent::new(100)))
    );
    assert_eq!(
        Setting::parse("volume", "-3"),
        Err(SettingError::Unreadable)
    );
    assert_eq!(
        Setting::parse("subtitle_opacity", "0"),
        Ok(Setting::SubtitleOpacity(SubtitleOpacity::new(25))),
        "never fully transparent"
    );
    assert_eq!(
        Setting::parse("seek_step", "7"),
        Err(SettingError::Unreadable)
    );
}

#[test]
fn preferred_languages_come_first_choice_first() {
    let (spa, eng) = (Language::from_code("spa"), Language::from_code("eng"));
    let settings = Settings {
        subtitle_language: None,
        secondary_subtitle_language: eng,
        audio_language: spa,
        secondary_audio_language: eng,
        ..Settings::default()
    };
    let codes = |l: Vec<Language>| l.iter().map(|l| l.code()).collect::<Vec<_>>();
    assert_eq!(codes(settings.subtitle_languages().collect()), ["eng"]);
    assert_eq!(codes(settings.audio_languages().collect()), ["spa", "eng"]);
}
