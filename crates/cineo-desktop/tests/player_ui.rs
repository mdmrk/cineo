//! The playback screen's controls, rendered headlessly: which commands
//! clicks and keys produce. Playback itself is tested in
//! `cineo-player`.

// Test helpers panic on purpose: a panic is a failed assertion.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use cineo_core::app::{Action, HideControls, SeekStep, Setting, Settings, ShortSeekStep};
use cineo_desktop::player::{AddonSubtitle, Controls, Playback, show};
use cineo_player::embedded::{PlayerCommand, Status, Track, TrackKind};
use eframe::egui::{Key, Modifiers, ViewportId};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

type Screen = (
    Status,
    Controls,
    Vec<PlayerCommand>,
    Vec<AddonSubtitle>,
    Settings,
);

fn track(id: i64, kind: TrackKind, lang: &str, selected: bool) -> Track {
    Track {
        id,
        kind,
        title: None,
        lang: Some(lang.into()),
        selected,
        external_file: None,
    }
}

fn status() -> Status {
    Status {
        loaded: true,
        position_s: 75.0,
        duration_s: 600.0,
        paused: true,
        volume: 80.0,
        tracks: [
            track(1, TrackKind::Video, "", true),
            track(1, TrackKind::Audio, "eng", true),
            track(2, TrackKind::Audio, "spa", false),
            track(1, TrackKind::Subtitle, "fra", false),
        ]
        .into(),
        ..Status::default()
    }
}

fn harness(status: Status) -> Harness<'static, Screen> {
    harness_with(status, Vec::new())
}

fn harness_with(status: Status, addon: Vec<AddonSubtitle>) -> Harness<'static, Screen> {
    harness_full(status, addon, Settings::default())
}

fn harness_full(
    status: Status,
    addon: Vec<AddonSubtitle>,
    settings: Settings,
) -> Harness<'static, Screen> {
    let mut harness = Harness::builder()
        .with_size([1280.0, 720.0])
        .build_ui_state(
            |ui, (status, controls, out, addon, settings): &mut Screen| {
                let rect = ui.max_rect();
                let playback = Playback {
                    status,
                    title: "A Film",
                    logo: None,
                    background: None,
                    addon_subtitles: addon,
                    settings,
                };
                out.extend(show(ui, rect, &playback, controls));
            },
            (status, Controls::default(), Vec::new(), addon, settings),
        );
    harness.run_steps(2);
    harness
}

#[test]
fn paused_playback_shows_title_time_and_play() {
    let harness = harness(status());
    harness.get_by_label("A Film");
    harness.get_by_label("1:15 / 10:00");
    harness.get_by_label("Play");
}

#[test]
fn buttons_send_typed_commands() {
    let mut harness = harness(status());
    harness.get_by_label("Play").click();
    harness.run();
    harness.get_by_label("Seek forward 10 seconds").click();
    harness.run();
    harness.get_by_label("Mute").click();
    harness.run();
    harness.get_by_label("Back").click();
    harness.run();
    assert_eq!(
        harness.state().2,
        vec![
            PlayerCommand::TogglePause,
            PlayerCommand::SeekBy(10.0),
            PlayerCommand::ToggleMute,
            PlayerCommand::Stop,
        ]
    );
}

#[test]
fn track_menus_list_tracks_and_select_one() {
    let mut harness = harness(status());
    harness.get_by_label("Subtitles").click();
    harness.run();
    harness.get_by_label("Off");
    harness.get_by_label("fra").click();
    harness.run();
    assert!(
        harness.query_by_label("Off").is_none(),
        "the menu closes after a choice"
    );

    harness.get_by_label("Audio").click();
    harness.run();
    harness.get_by_label("eng");
    harness.get_by_label("spa").click();
    harness.run();
    assert_eq!(
        harness.state().2,
        vec![
            PlayerCommand::SetSubtitle(Some(1)),
            PlayerCommand::SetAudio(Some(2))
        ]
    );
}

#[test]
fn the_subtitles_menu_changes_the_style_and_the_delay() {
    let mut harness = harness(status());
    harness.get_by_label("Subtitles").click();
    harness.run();
    harness.get_by_label("Bold").click();
    harness.run();
    for label in [
        "Subtitles 1 s later",
        "Subtitles 0.1 s later",
        "Subtitles 0.1 s earlier",
        "Subtitles 0.1 s earlier",
    ] {
        harness.get_by_label(label).click();
        harness.run();
    }
    harness.get_by_label("Subtitle delay +0.9 s, reset").click();
    harness.run();
    harness.get_by_label("Off");
    assert_eq!(
        harness.state_mut().1.take_settings(),
        [Action::ChangeSetting(Setting::SubtitleBold(true))]
    );
    assert_eq!(
        harness.state().2,
        [
            PlayerCommand::SetSubtitleDelay(1.0),
            PlayerCommand::SetSubtitleDelay(1.1),
            PlayerCommand::SetSubtitleDelay(1.0),
            PlayerCommand::SetSubtitleDelay(0.9),
            PlayerCommand::SetSubtitleDelay(0.0),
        ]
    );
}

#[test]
fn keys_control_playback_and_escape_leaves() {
    let mut harness = harness(status());
    for key in [
        Key::Space,
        Key::ArrowLeft,
        Key::ArrowUp,
        Key::M,
        Key::Escape,
    ] {
        harness.key_press(key);
        harness.run();
    }
    assert_eq!(
        harness.state().2,
        vec![
            PlayerCommand::TogglePause,
            PlayerCommand::SeekBy(-10.0),
            PlayerCommand::SetVolume(85.0),
            PlayerCommand::ToggleMute,
            PlayerCommand::Stop,
        ]
    );
}

#[test]
fn controls_hide_while_playing_and_stay_while_paused() {
    let mut playing = status();
    playing.paused = false;
    let mut harness = harness(playing);
    harness.get_by_label("Pause");
    harness.run_steps(16);
    assert!(
        harness.query_by_label("Pause").is_none(),
        "hidden while playing"
    );

    let mut harness = self::harness(status());
    harness.run_steps(16);
    harness.get_by_label("Play");
}

#[test]
fn before_the_file_loads_back_is_available_and_stops() {
    let mut harness = harness(Status::default());
    harness.get_by_label("A Film");
    assert!(
        harness.query_by_label("Pause").is_none(),
        "no playback controls yet"
    );
    harness.get_by_label("Back").click();
    harness.run_steps(1);
    assert_eq!(harness.state().2, vec![PlayerCommand::Stop]);
}

#[test]
fn addon_subtitles_are_listed_and_requested() {
    let mut status = status();
    status.tracks = status
        .tracks
        .iter()
        .cloned()
        .chain([Track {
            external_file: Some("/cache/subtitles/1".into()),
            ..track(2, TrackKind::Subtitle, "spa", false)
        }])
        .collect();
    let addon = vec![
        AddonSubtitle {
            url: "https://subs.example/en.srt".parse().unwrap(),
            lang: "eng".into(),
            label: None,
            addon_name: "Subs Fixture".into(),
            selected: false,
        },
        AddonSubtitle {
            url: "https://subs.example/es.srt".parse().unwrap(),
            lang: "spa".into(),
            label: Some("Latino".into()),
            addon_name: "Subs Fixture".into(),
            selected: true,
        },
    ];
    let mut harness = harness_with(status, addon);
    harness.get_by_label("Subtitles").click();
    harness.run();
    harness.get_by_label("fra");
    harness.get_by_label("spa · Latino");
    assert!(
        harness.query_by_label("spa").is_none(),
        "loaded addon files are listed once, as addon entries"
    );
    harness.get_by_label("eng · Subs Fixture").click();
    harness.run();
    assert_eq!(
        harness
            .state_mut()
            .1
            .take_addon_subtitle()
            .map(String::from),
        Some("https://subs.example/en.srt".to_owned())
    );
    assert!(
        harness.state().2.is_empty(),
        "no mpv command until the file is fetched"
    );
    assert!(harness.query_by_label("Off").is_none(), "the menu closes");
}

#[test]
fn seek_steps_come_from_settings() {
    let settings = Settings {
        seek_step: SeekStep::S30,
        short_seek_step: ShortSeekStep::S1,
        ..Settings::default()
    };
    let mut harness = harness_full(status(), Vec::new(), settings);
    harness.key_press(Key::ArrowRight);
    harness.run();
    harness.key_press_modifiers(Modifiers::SHIFT, Key::ArrowLeft);
    harness.run();
    harness.get_by_label("Seek back 30 seconds").click();
    harness.run();
    assert_eq!(
        harness.state().2,
        vec![
            PlayerCommand::SeekBy(30.0),
            PlayerCommand::SeekBy(-1.0),
            PlayerCommand::SeekBy(-30.0),
        ]
    );
}

#[test]
fn escape_in_fullscreen_can_leave_the_player_at_once() {
    for (leaves_fullscreen, expected) in [(true, vec![]), (false, vec![PlayerCommand::Stop])] {
        let settings = Settings {
            escape_leaves_fullscreen: leaves_fullscreen,
            ..Settings::default()
        };
        let mut harness = harness_full(status(), Vec::new(), settings);
        harness
            .input_mut()
            .viewports
            .entry(ViewportId::ROOT)
            .or_default()
            .fullscreen = Some(true);
        harness.key_press(Key::Escape);
        harness.run();
        assert_eq!(harness.state().2, expected, "{leaves_fullscreen}");
    }
}

#[test]
fn controls_stay_longer_with_a_longer_hide_delay() {
    let mut playing = status();
    playing.paused = false;
    let settings = Settings {
        hide_controls: HideControls::Long,
        ..Settings::default()
    };
    let mut harness = harness_full(playing, Vec::new(), settings);
    harness.run_steps(16);
    harness.get_by_label("Pause");
}
