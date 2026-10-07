//! Settings keys and the text they are saved as.

// Test helpers panic on purpose: a panic is a failed assertion.
#![allow(clippy::unwrap_used)]

use cineo_core::app::{Language, Setting, SettingError, Settings};

#[test]
fn every_setting_reads_back_what_it_saves() {
    let mut settings = Settings::default();
    settings.set(Setting::P2pEnabled(false));
    settings.set(Setting::SubtitleLanguage(Language::from_code("por")));
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
        ["p2p_enabled", "p2p_acknowledged", "subtitle_language"]
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
