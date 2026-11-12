use super::*;

#[test]
fn resetting_settings_restores_defaults_but_keeps_the_p2p_notice_accepted() {
    let mut state = detail_with_p2p_accepted();
    update(
        &mut state,
        Action::ChangeSetting(Setting::SubtitleLanguage(Language::from_code("spa"))),
    );
    let effects = update(&mut state, Action::ResetSettings);
    let expected = Settings {
        p2p_acknowledged: true,
        ..Settings::default()
    };
    assert_eq!(effects, vec![Effect::SaveSettings(expected)]);
    assert_eq!(state.settings, expected);
}

#[test]
fn resetting_settings_restores_the_shells_defaults() {
    let mut state = State {
        default_settings: Settings {
            torrent_storage: TorrentStorage::Disk4096,
            ..Settings::default()
        },
        ..detail_with_p2p_accepted()
    };
    update(
        &mut state,
        Action::ChangeSetting(Setting::TorrentStorage(TorrentStorage::Ram512)),
    );
    update(&mut state, Action::ResetSettings);
    assert_eq!(state.settings.torrent_storage, TorrentStorage::Disk4096);
    assert!(state.settings.p2p_acknowledged);
}

#[test]
fn restoring_settings_checks_for_an_update_once() {
    let mut state = State::default();
    assert_eq!(
        update(&mut state, Action::RestoreSettings(Settings::default())),
        vec![Effect::CheckForUpdate(update_url())]
    );
    assert!(update(&mut state, Action::RestoreSettings(Settings::default())).is_empty());
    assert!(
        !update(&mut state, Action::ResetSettings).contains(&Effect::CheckForUpdate(update_url()))
    );
}

#[test]
fn a_newer_release_is_offered_and_an_older_one_is_not() {
    let mut state = State::default();
    update(&mut state, Action::RestoreSettings(Settings::default()));
    update(
        &mut state,
        Action::LatestReleaseLoaded(Ok(semver::Version::new(0, 0, 1))),
    );
    assert_eq!(state.available_update(), None);
    update(
        &mut state,
        Action::LatestReleaseLoaded(Err("HTTP 403".into())),
    );
    assert_eq!(state.available_update(), None);
    let newer = semver::Version::new(999, 0, 0);
    update(&mut state, Action::LatestReleaseLoaded(Ok(newer.clone())));
    assert_eq!(state.available_update(), Some(&newer));
}

#[test]
fn with_update_checks_off_nothing_is_fetched_or_shown() {
    let mut state = State::default();
    let off = Settings {
        check_for_updates: false,
        ..Settings::default()
    };
    assert!(update(&mut state, Action::RestoreSettings(off)).is_empty());
    update(
        &mut state,
        Action::LatestReleaseLoaded(Ok(semver::Version::new(999, 0, 0))),
    );
    assert_eq!(state.available_update(), None);

    let effects = update(
        &mut state,
        Action::ChangeSetting(Setting::CheckForUpdates(true)),
    );
    assert!(
        effects.contains(&Effect::CheckForUpdate(update_url())),
        "{effects:?}"
    );
    update(
        &mut state,
        Action::LatestReleaseLoaded(Ok(semver::Version::new(999, 0, 0))),
    );
    assert!(state.available_update().is_some());
    update(
        &mut state,
        Action::ChangeSetting(Setting::CheckForUpdates(false)),
    );
    assert_eq!(
        state.available_update(),
        None,
        "turning checks off hides it"
    );
    update(
        &mut state,
        Action::ChangeSetting(Setting::CheckForUpdates(true)),
    );
    assert!(
        state.available_update().is_some(),
        "turning checks on again shows it without asking again"
    );
}

fn update_url() -> url::Url {
    url::Url::parse(cineo_core::app::LATEST_RELEASE_URL).unwrap()
}
