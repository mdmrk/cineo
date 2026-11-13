use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn changed_settings_reach_the_player_and_survive_a_restart_until_reset() {
    let server = MockServer::start().await;
    let manifest = movie_addon(&server).await;
    let dir = local_data_dir();

    tokio::task::block_in_place(|| {
        let players = Arc::new(FakePlayers::default());
        let (cineo, views) = start_with(dir.path(), &players);
        let first = wait_for(&views, |_| true);
        assert!(first.settings.hardware_decoding);
        assert_eq!(first.settings.seek_steps_s, [5, 10, 15, 30]);
        assert_eq!(first.settings.watched_at_choices, [80, 85, 90, 92, 95]);
        assert!(first.settings.languages.iter().any(|c| c == "spa"));
        cineo.install_addon(manifest);
        wait_for(&views, |v| v.addons.len() == 1);
        for change in [
            SettingChange::HardwareDecoding { on: false },
            SettingChange::SeekStep { seconds: 30 },
            SettingChange::AudioLanguage {
                code: Some("spa".into()),
            },
            SettingChange::SubtitleLanguage {
                code: Some("eng".into()),
            },
            SettingChange::WatchedAt { percent: 80 },
            SettingChange::SeekStep { seconds: 7 },
            SettingChange::AudioLanguage {
                code: Some("klingon".into()),
            },
        ] {
            cineo.change_setting(change);
        }
        wait_for(&views, |v| v.settings.watched_at_percent == 80);
        cineo.open_detail("movie".into(), "tt0000001".into());
        wait_for(&views, streams_ready);
        cineo.play(0, 0);
        let playing = wait_for(&views, |v| v.playback.is_some());
        assert_eq!(playing.playback.unwrap().seek_step_s, 30);
        let request = players.requests.lock().unwrap()[0].clone();
        assert!(!request.settings.hardware_decoding);
        assert_eq!(
            request.settings.audio_language.map(|l| l.code()),
            Some("spa")
        );
        assert_eq!(
            request.settings.subtitle_language.map(|l| l.code()),
            Some("eng")
        );
        drop(cineo);

        let (cineo, views) = start(dir.path());
        let restored = wait_for(&views, |_| true).settings;
        assert!(!restored.hardware_decoding);
        assert_eq!(restored.seek_step_s, 30);
        assert_eq!(restored.audio_language.as_deref(), Some("spa"));
        assert_eq!(restored.subtitle_language.as_deref(), Some("eng"));
        assert_eq!(restored.watched_at_percent, 80);
        assert!(restored.allow_private_network);
        cineo.change_setting(SettingChange::AudioLanguage { code: None });
        wait_for(&views, |v| v.settings.audio_language.is_none());
        cineo.reset_settings();
        let reset = wait_for(&views, |v| v.settings.hardware_decoding).settings;
        assert_eq!(reset.seek_step_s, 10);
        assert_eq!(reset.watched_at_percent, 92);
        assert_eq!(reset.subtitle_language, None);
        assert!(!reset.allow_private_network);
        assert_eq!(reset.torrent_storage.value, "disk");
        drop(cineo);
    });

    let saved = Store::open_in(dir.path()).unwrap().settings().unwrap();
    assert_eq!(
        saved,
        Settings {
            torrent_storage: TorrentStorage::Disk4096,
            pause_on_minimize: true,
            ..Settings::default()
        }
    );
}

#[test]
fn a_new_install_keeps_torrent_data_on_disk_until_changed_and_again_after_a_reset() {
    let dir = scratch_dir();
    Store::open_in(dir.path())
        .unwrap()
        .save_addons(&[])
        .unwrap();

    let (cineo, views) = start(dir.path());
    let first = wait_for(&views, |_| true).settings;
    assert_eq!(first.torrent_storage.value, "disk");
    assert!(first.pause_on_minimize.on);
    cineo.change_setting(SettingChange::Value {
        key: "torrent_storage".into(),
        value: "ram-512".into(),
    });
    wait_for(&views, |v| v.settings.torrent_storage.value == "ram-512");
    cineo.reset_settings();
    wait_for(&views, |v| v.settings.torrent_storage.value == "disk");
    drop(cineo);

    let saved = Store::open_in(dir.path()).unwrap().settings().unwrap();
    assert_eq!(saved.torrent_storage, TorrentStorage::Disk4096);
}

#[tokio::test(flavor = "multi_thread")]
async fn subtitle_torrent_and_player_settings_change_by_key_and_reach_the_player() {
    let server = MockServer::start().await;
    let manifest = movie_addon(&server).await;
    let dir = local_data_dir();

    tokio::task::block_in_place(|| {
        let players = Arc::new(FakePlayers::default());
        let (cineo, views) = start_with(dir.path(), &players);
        let first = wait_for(&views, |_| true).settings;
        assert_eq!(first.subtitle_font.key, "subtitle_font");
        assert_eq!(first.subtitle_font.value, "sans");
        assert_eq!(first.subtitle_font.options, ["sans", "serif", "mono"]);
        assert_eq!(first.hide_controls.options, ["1.5", "2.5", "5"]);
        assert_eq!(
            (first.subtitle_size.min, first.subtitle_size.max),
            (50, 200)
        );
        assert_eq!(first.torrent_port.value, 42713);
        assert_eq!(first.check_for_updates.key, "check_for_updates");
        assert!(!first.check_for_updates.on);
        assert!(first.torrent_upload.on);
        cineo.install_addon(manifest);
        wait_for(&views, |v| v.addons.len() == 1);
        let value = |key: &str, value: &str| SettingChange::Value {
            key: key.into(),
            value: value.into(),
        };
        for change in [
            value("subtitle_font", "serif"),
            value("subtitle_size", "150"),
            value("subtitle_bold", "true"),
            value("torrent_upload", "false"),
            value("download_limit", "5"),
            value("torrent_port", "50000"),
            value("hide_controls", "5"),
            value("subtitle_font", "comic"),
            value("no_such_setting", "1"),
            SettingChange::SecondaryAudioLanguage {
                code: Some("eng".into()),
            },
            SettingChange::SecondarySubtitleLanguage {
                code: Some("fre".into()),
            },
        ] {
            cineo.change_setting(change);
        }
        let changed =
            wait_for(&views, |v| v.settings.secondary_subtitle_language.is_some()).settings;
        assert_eq!(changed.subtitle_font.value, "serif");
        assert_eq!(changed.subtitle_size.value, 150);
        assert!(changed.subtitle_bold.on);
        assert!(!changed.torrent_upload.on);
        assert_eq!(changed.download_limit.value, "5");
        assert_eq!(changed.torrent_port.value, 50000);
        assert_eq!(changed.secondary_audio_language.as_deref(), Some("eng"));
        cineo.open_detail("movie".into(), "tt0000001".into());
        wait_for(&views, streams_ready);
        cineo.play(0, 0);
        let playing = wait_for(&views, |v| v.playback.is_some());
        assert_eq!(playing.playback.unwrap().hide_controls_ms, 5000);
        let request = players.requests.lock().unwrap()[0].clone();
        assert_eq!(request.settings.subtitle_font, SubtitleFont::Serif);
        assert_eq!(request.settings.subtitle_size.get(), 150);
        assert!(!request.settings.torrent_upload);
        assert_eq!(
            request
                .settings
                .audio_languages()
                .map(|l| l.code())
                .collect::<Vec<_>>(),
            ["eng"]
        );
        drop(cineo);

        let (_cineo, views) = start(dir.path());
        let restored = wait_for(&views, |_| true).settings;
        assert_eq!(restored.subtitle_font.value, "serif");
        assert_eq!(restored.hide_controls.value, "5");
        assert_eq!(restored.secondary_subtitle_language.as_deref(), Some("fre"));
    });
}
