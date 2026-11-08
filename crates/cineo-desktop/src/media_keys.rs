use eframe::egui;
use fastframe_now_playing::{App, Command, Controls, NowPlaying, Playback, State, Track};

const TRACK_ID: &str = "current";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Remote {
    Play,
    Pause,
    PlayPause,
    Stop,
    Raise,
}

pub(crate) struct MediaKeys {
    ctx: egui::Context,
    controls: Option<NowPlaying>,
}

impl MediaKeys {
    pub(crate) fn new(ctx: egui::Context) -> Self {
        Self {
            ctx,
            controls: None,
        }
    }

    pub(crate) fn set(&mut self, playing: Option<(&str, bool)>) {
        if playing.is_none() {
            self.stop();
            return;
        }
        let ctx = &self.ctx;
        self.controls
            .get_or_insert_with(|| {
                let ctx = ctx.clone();
                NowPlaying::start(app(std::process::id()), move || ctx.request_repaint())
            })
            .update(state(playing));
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

fn state(playing: Option<(&str, bool)>) -> State {
    let controls = Controls {
        next: false,
        previous: false,
        seek: false,
        ..Controls::default()
    };
    let Some((title, paused)) = playing else {
        return State {
            controls,
            ..State::default()
        };
    };
    State {
        playback: if paused {
            Playback::Paused
        } else {
            Playback::Playing
        },
        track: Some(Track {
            id: TRACK_ID.to_owned(),
            title: title.to_owned(),
            ..Track::default()
        }),
        controls,
        ..State::default()
    }
}

fn remote_of(command: &Command) -> Option<Remote> {
    match command {
        Command::Play => Some(Remote::Play),
        Command::Pause => Some(Remote::Pause),
        Command::PlayPause => Some(Remote::PlayPause),
        Command::Stop => Some(Remote::Stop),
        Command::Raise => Some(Remote::Raise),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn the_desktop_sees_the_title_and_pause_state_without_seek_or_tracks() {
        let playing = state(Some(("Movie", false)));
        assert_eq!(playing.playback, Playback::Playing);
        assert_eq!(playing.track.map(|t| t.title), Some("Movie".to_owned()));
        assert!(playing.controls.play && playing.controls.pause);
        assert!(!playing.controls.next && !playing.controls.previous && !playing.controls.seek);
        assert_eq!(state(Some(("Movie", true))).playback, Playback::Paused);
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
