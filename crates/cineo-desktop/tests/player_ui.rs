//! The playback screen's controls, rendered headlessly: which commands
//! clicks and keys produce. Playback itself is tested in
//! `cineo-player`.

// Test helpers panic on purpose: a panic is a failed assertion.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use cineo_desktop::player::{Controls, show};
use cineo_player::embedded::{PlayerCommand, Status, Track, TrackKind};
use eframe::egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

type Screen = (Status, Controls, Vec<PlayerCommand>);

fn track(id: i64, kind: TrackKind, lang: &str, selected: bool) -> Track {
    Track {
        id,
        kind,
        title: None,
        lang: Some(lang.into()),
        selected,
    }
}

fn status() -> Status {
    Status {
        loaded: true,
        position_s: 75.0,
        duration_s: 600.0,
        paused: true,
        volume: 80.0,
        tracks: vec![
            track(1, TrackKind::Video, "", true),
            track(1, TrackKind::Audio, "eng", true),
            track(2, TrackKind::Audio, "spa", false),
            track(1, TrackKind::Subtitle, "fra", false),
        ],
        ..Status::default()
    }
}

fn harness(status: Status) -> Harness<'static, Screen> {
    let mut harness = Harness::builder()
        .with_size([1280.0, 720.0])
        .build_ui_state(
            |ui, (status, controls, out): &mut Screen| {
                let rect = ui.max_rect();
                out.extend(show(ui, rect, status, "A Film", None, controls));
            },
            (status, Controls::default(), Vec::new()),
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
