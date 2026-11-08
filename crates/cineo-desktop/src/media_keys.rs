use std::time::Duration;

use eframe::egui;
use fastframe_now_playing::{App, Command, Controls, NowPlaying, Playback, State, Track};

const TRACK_ID: &str = "current";
const SEEK_JUMP_S: f64 = 2.0;
const FULL_VOLUME: f64 = 100.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Remote {
    Play,
    Pause,
    PlayPause,
    Stop,
    Raise,
    SeekBy(f64),
    SeekTo(f64),
    SetVolume(f64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Playing<'a> {
    pub(crate) title: &'a str,
    pub(crate) paused: bool,
    pub(crate) position_s: f64,
    pub(crate) duration_s: f64,
    pub(crate) volume: f64,
}

pub(crate) struct MediaKeys {
    ctx: egui::Context,
    controls: Option<NowPlaying>,
    last_position: Option<f64>,
}

impl MediaKeys {
    pub(crate) fn new(ctx: egui::Context) -> Self {
        Self {
            ctx,
            controls: None,
            last_position: None,
        }
    }

    pub(crate) fn set(&mut self, playing: Option<Playing<'_>>) {
        let Some(now) = playing else {
            self.last_position = None;
            self.stop();
            return;
        };
        let ctx = &self.ctx;
        let controls = self.controls.get_or_insert_with(|| {
            let ctx = ctx.clone();
            NowPlaying::start(app(std::process::id()), move || ctx.request_repaint())
        });
        controls.update(state(Some(now)));
        if jumped(self.last_position, now.position_s) {
            controls.seeked(duration(now.position_s));
        }
        self.last_position = Some(now.position_s);
    }

    #[cfg(not(windows))]
    fn stop(&mut self) {
        self.controls = None;
    }

    #[cfg(windows)]
    fn stop(&mut self) {
        if let Some(controls) = &mut self.controls {
            controls.update(state(None));
        }
    }

    pub(crate) fn take(&self) -> Vec<Remote> {
        self.controls.as_ref().map_or_else(Vec::new, |controls| {
            controls.commands().iter().filter_map(remote_of).collect()
        })
    }
}

fn app(pid: u32) -> App {
    App {
        desktop_entry: "cineo".to_owned(),
        can_quit: false,
        ..App::new(&format!("cineo.instance{pid}"), "Cineo")
    }
}

fn duration(seconds: f64) -> Duration {
    Duration::try_from_secs_f64(seconds.max(0.0)).unwrap_or_default()
}

fn jumped(last: Option<f64>, now: f64) -> bool {
    last.is_some_and(|last| (now - last).abs() > SEEK_JUMP_S)
}

fn state(playing: Option<Playing<'_>>) -> State {
    let controls = Controls {
        next: false,
        previous: false,
        seek: false,
        ..Controls::default()
    };
    let Some(now) = playing else {
        return State {
            controls,
            ..State::default()
        };
    };
    let known_length = now.duration_s > 0.0;
    State {
        playback: if now.paused {
            Playback::Paused
        } else {
            Playback::Playing
        },
        track: Some(Track {
            id: TRACK_ID.to_owned(),
            title: now.title.to_owned(),
            duration: known_length.then(|| duration(now.duration_s)),
            ..Track::default()
        }),
        position: duration(now.position_s),
        volume: Some((now.volume / FULL_VOLUME).clamp(0.0, 1.0)),
        controls: Controls {
            seek: known_length,
            ..controls
        },
        ..State::default()
    }
}

fn seconds(millis: i64) -> f64 {
    let amount = Duration::from_millis(millis.unsigned_abs()).as_secs_f64();
    if millis < 0 { -amount } else { amount }
}

fn remote_of(command: &Command) -> Option<Remote> {
    match command {
        Command::Play => Some(Remote::Play),
        Command::Pause => Some(Remote::Pause),
        Command::PlayPause => Some(Remote::PlayPause),
        Command::Stop => Some(Remote::Stop),
        Command::Raise => Some(Remote::Raise),
        Command::SeekBy(millis) => Some(Remote::SeekBy(seconds(*millis))),
        Command::SetPosition { track_id, position } if track_id == TRACK_ID => {
            Some(Remote::SeekTo(position.as_secs_f64()))
        }
        Command::SetVolume(volume) => Some(Remote::SetVolume(volume.clamp(0.0, 1.0) * FULL_VOLUME)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playing(title: &str, paused: bool) -> Playing<'_> {
        Playing {
            title,
            paused,
            position_s: 30.0,
            duration_s: 600.0,
            volume: 40.0,
        }
    }

    #[test]
    fn seek_and_volume_commands_map_to_remotes() {
        assert_eq!(
            remote_of(&Command::SeekBy(-10_000)),
            Some(Remote::SeekBy(-10.0))
        );
        assert_eq!(
            remote_of(&Command::SetPosition {
                track_id: TRACK_ID.to_owned(),
                position: Duration::from_secs(90),
            }),
            Some(Remote::SeekTo(90.0))
        );
        assert_eq!(
            remote_of(&Command::SetPosition {
                track_id: "another".to_owned(),
                position: Duration::from_secs(90),
            }),
            None,
            "a request for another track is ignored"
        );
        assert_eq!(
            remote_of(&Command::SetVolume(0.5)),
            Some(Remote::SetVolume(50.0))
        );
    }

    #[test]
    fn only_a_jump_counts_as_a_seek() {
        assert!(!jumped(None, 500.0), "the first report");
        assert!(!jumped(Some(10.0), 10.5), "playing on");
        assert!(jumped(Some(10.0), 200.0), "forward");
        assert!(jumped(Some(200.0), 10.0), "back");
    }

    #[test]
    fn desktop_commands_map_to_remotes() {
        assert_eq!(remote_of(&Command::PlayPause), Some(Remote::PlayPause));
        assert_eq!(remote_of(&Command::Play), Some(Remote::Play));
        assert_eq!(remote_of(&Command::Pause), Some(Remote::Pause));
        assert_eq!(remote_of(&Command::Stop), Some(Remote::Stop));
        assert_eq!(remote_of(&Command::Raise), Some(Remote::Raise));
        assert_eq!(remote_of(&Command::Next), None);
        assert_eq!(remote_of(&Command::Quit), None);
    }

    #[test]
    fn the_desktop_sees_the_title_position_length_and_volume() {
        let shown = state(Some(playing("Movie", false)));
        assert_eq!(shown.playback, Playback::Playing);
        assert_eq!(shown.position, Duration::from_secs(30));
        assert_eq!(shown.volume, Some(0.4));
        assert!(shown.controls.play && shown.controls.pause && shown.controls.seek);
        assert!(!shown.controls.next && !shown.controls.previous);
        let track = shown.track.as_ref();
        assert_eq!(track.map(|t| t.title.as_str()), Some("Movie"));
        assert_eq!(
            track.and_then(|t| t.duration),
            Some(Duration::from_secs(600))
        );
        assert_eq!(
            state(Some(playing("Movie", true))).playback,
            Playback::Paused
        );
    }

    #[test]
    fn a_stream_of_unknown_length_cannot_be_sought() {
        let live = Playing {
            duration_s: 0.0,
            ..playing("Live", false)
        };
        let shown = state(Some(live));
        assert!(!shown.controls.seek);
        assert_eq!(shown.track.and_then(|t| t.duration), None);
    }

    #[test]
    fn nothing_playing_shows_as_stopped_with_no_track() {
        let stopped = state(None);
        assert_eq!(stopped.playback, Playback::Stopped);
        assert!(stopped.track.is_none());
    }

    #[test]
    fn each_process_keeps_its_own_bus_name_and_cannot_be_quit() {
        let app = app(42);
        assert_eq!(app.bus_name, "cineo.instance42");
        assert_eq!(app.identity, "Cineo");
        assert_eq!(app.desktop_entry, "cineo");
        assert!(app.can_raise);
        assert!(!app.can_quit);
    }
}
