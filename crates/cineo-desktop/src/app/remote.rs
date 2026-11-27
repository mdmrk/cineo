use std::sync::{Arc, Mutex, PoisonError};

use cineo_core::app::Action;
use cineo_core::remote::{
    Command, DEFAULT_PORT, Hosting, NowPlaying, RemoteKey, Status, Track, computer_name,
};
use cineo_net::remote::{Handler, RemoteError, RemoteServer, bind, lan_ip, new_remote_key, serve};
use cineo_player::embedded::{PlayerCommand, Status as PlayerStatus, TrackKind};
use eframe::egui;
use tracing::warn;

use super::{CineoApp, Msg, remote_commands};
use crate::media_keys::Remote;
use crate::player::{self, Controls};
use crate::view::{RemoteListener, RemoteRequest};

#[derive(Debug, Default)]
pub(super) struct Host {
    server: Option<RemoteServer>,
    shown: Arc<Mutex<Status>>,
}

impl Host {
    fn publish(&self, status: Status) {
        let mut shown = self.shown.lock().unwrap_or_else(PoisonError::into_inner);
        if *shown != status {
            *shown = status;
        }
    }
}

impl CineoApp {
    pub(super) fn serve_remote(&mut self, key: Option<RemoteKey>) {
        self.remote.server = None;
        let Some(key) = key else {
            self.view.remote_listener = RemoteListener::Off;
            self.announce_address();
            return;
        };
        let post = self.post();
        let shown = Arc::clone(&self.remote.shown);
        let handler: Handler = Arc::new(move |command| {
            if let Some(command) = command {
                post(Msg::Remote(command));
            }
            shown.lock().unwrap_or_else(PoisonError::into_inner).clone()
        });
        let _runtime = self.io.runtime.enter();
        let started = bind(DEFAULT_PORT)
            .map_err(cineo_net::remote::RemoteError::from)
            .and_then(|listener| serve(listener, &key, handler));
        self.view.remote_listener = match started {
            Ok(server) => {
                let port = server.port();
                self.remote.server = Some(server);
                RemoteListener::On { ip: lan_ip(), port }
            }
            Err(err) => {
                warn!(%err, "the remote control did not start");
                RemoteListener::Failed(err.to_string())
            }
        };
        self.announce_address();
    }

    fn announce_address(&mut self) {
        let address = listening_address(&self.view.remote_listener);
        if address != self.state.remote.address || self.state.remote.name.is_empty() {
            let name = computer_name(&gethostname::gethostname().to_string_lossy());
            self.dispatch(Action::RemoteListening { address, name });
        }
    }

    pub(super) fn remote_request(&mut self, request: RemoteRequest) {
        let key = match request {
            RemoteRequest::TurnOff => None,
            RemoteRequest::TurnOn | RemoteRequest::NewCode => match new_remote_key() {
                Ok(key) => Some(key),
                Err(err) => {
                    warn!(%err, "no random remote key");
                    self.view.remote_listener = RemoteListener::Failed(err.to_string());
                    return;
                }
            },
        };
        self.dispatch(Action::SetRemoteHost(key));
    }

    pub(super) fn refresh_remote_address(&mut self) {
        if let RemoteListener::On { ip, .. } = &mut self.view.remote_listener {
            *ip = lan_ip();
        }
        self.announce_address();
    }

    pub(super) fn publish_remote(&self) {
        if self.remote.server.is_none() {
            return;
        }
        let playing = if let Some(embedded) = &self.playback {
            Some(now_playing(
                &embedded.title,
                &embedded.player.status(),
                &embedded.controls,
            ))
        } else {
            self.state
                .torrent
                .as_ref()
                .and_then(|t| t.connecting())
                .map(|request| NowPlaying {
                    title: request.title.clone(),
                    loading: true,
                    ..NowPlaying::default()
                })
        };
        self.remote.publish(Status { playing });
    }

    pub(super) fn remote_command(&mut self, command: Command) {
        if let Command::Play(sent) = command {
            self.end_playback();
            self.connecting_controls = Controls::default();
            self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            self.dispatch(Action::PlaySent(sent));
            return;
        }
        if let Some(embedded) = &mut self.playback {
            let status = embedded.player.status();
            if matches!(command, Command::Subtitle(_)) {
                embedded.auto_subtitle.clear();
            }
            for command in player_commands(&command, &status, &mut embedded.controls) {
                embedded.player.send(command);
            }
        } else if command == Command::Stop
            && self
                .state
                .torrent
                .as_ref()
                .is_some_and(|t| t.connecting().is_some())
        {
            self.connecting_controls = Controls::default();
            player::leave_fullscreen(&self.ctx);
            self.dispatch(Action::PlaybackStopped);
        }
    }
}

pub(super) fn at_start(
    hosting: Hosting,
    new_key: impl FnOnce() -> Result<RemoteKey, RemoteError>,
) -> Vec<Action> {
    let restore = |host| Action::RestoreRemote {
        host,
        peer: None,
        choice: None,
    };
    match hosting {
        Hosting::On(key) => vec![restore(Some(key))],
        Hosting::Off => vec![restore(None)],
        Hosting::Unchosen => match new_key() {
            Ok(key) => vec![restore(None), Action::SetRemoteHost(Some(key))],
            Err(err) => {
                warn!(%err, "no random remote key; the remote control stays off");
                vec![restore(None)]
            }
        },
    }
}

fn listening_address(listener: &RemoteListener) -> Option<std::net::SocketAddr> {
    match *listener {
        RemoteListener::On { ip: Some(ip), port } => Some(std::net::SocketAddr::new(ip, port)),
        _ => None,
    }
}

fn tracks(status: &PlayerStatus, kind: TrackKind) -> Vec<Track> {
    status
        .tracks
        .iter()
        .filter(|t| t.kind == kind)
        .map(|t| Track {
            id: t.id,
            label: t.label(),
            selected: t.selected,
        })
        .collect()
}

fn now_playing(title: &str, status: &PlayerStatus, controls: &Controls) -> NowPlaying {
    NowPlaying {
        title: title.to_owned(),
        loading: !status.loaded,
        paused: status.paused,
        position_s: status.position_s,
        duration_s: status.duration_s,
        volume: status.volume,
        muted: status.muted,
        speed: controls.speed(),
        subtitle_delay_s: controls.subtitle_delay_s(),
        audio: tracks(status, TrackKind::Audio),
        subtitles: tracks(status, TrackKind::Subtitle),
    }
}

fn player_commands(
    command: &Command,
    status: &PlayerStatus,
    controls: &mut Controls,
) -> Vec<PlayerCommand> {
    match *command {
        Command::Pause(true) => remote_commands(Remote::Pause, status),
        Command::Pause(false) => remote_commands(Remote::Play, status),
        Command::SeekTo(seconds) => remote_commands(Remote::SeekTo(seconds), status),
        Command::SeekBy(seconds) => remote_commands(Remote::SeekBy(seconds), status),
        Command::Volume(percent) => remote_commands(Remote::SetVolume(percent), status),
        Command::Stop => remote_commands(Remote::Stop, status),
        Command::Mute(muted) => vec![PlayerCommand::SetMute(muted)],
        Command::Audio(id) => vec![PlayerCommand::SetAudio(id)],
        Command::Subtitle(id) => vec![PlayerCommand::SetSubtitle(id)],
        Command::SubtitleDelay(seconds) => vec![controls.set_subtitle_delay(seconds)],
        Command::Speed(speed) => controls.set_speed(speed).into_iter().collect(),
        Command::Play(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use cineo_player::embedded::Track as PlayerTrack;

    use super::*;

    fn video() -> PlayerStatus {
        PlayerStatus {
            loaded: true,
            duration_s: 600.0,
            position_s: 30.0,
            volume: 70.0,
            tracks: Arc::from([
                PlayerTrack {
                    id: 1,
                    kind: TrackKind::Audio,
                    title: Some("Stereo".into()),
                    lang: Some("en".into()),
                    selected: true,
                    external_file: None,
                    codec: None,
                },
                PlayerTrack {
                    id: 2,
                    kind: TrackKind::Subtitle,
                    title: None,
                    lang: Some("es".into()),
                    selected: false,
                    external_file: None,
                    codec: None,
                },
                PlayerTrack {
                    id: 3,
                    kind: TrackKind::Video,
                    title: None,
                    lang: None,
                    selected: true,
                    external_file: None,
                    codec: None,
                },
            ]),
            ..PlayerStatus::default()
        }
    }

    #[test]
    fn phone_commands_become_player_commands() {
        let status = video();
        let mut controls = Controls::default();
        let mut one = |command| player_commands(&command, &status, &mut controls);
        assert_eq!(one(Command::Pause(true)), [PlayerCommand::SetPause(true)]);
        assert_eq!(one(Command::Pause(false)), [PlayerCommand::SetPause(false)]);
        assert_eq!(one(Command::SeekTo(90.0)), [PlayerCommand::SeekTo(90.0)]);
        assert_eq!(one(Command::SeekTo(900.0)), [], "past the end");
        assert_eq!(one(Command::SeekBy(-10.0)), [PlayerCommand::SeekBy(-10.0)]);
        assert_eq!(one(Command::Volume(40.0)), [PlayerCommand::SetVolume(40.0)]);
        assert_eq!(one(Command::Mute(true)), [PlayerCommand::SetMute(true)]);
        assert_eq!(
            one(Command::Audio(Some(1))),
            [PlayerCommand::SetAudio(Some(1))]
        );
        assert_eq!(
            one(Command::Subtitle(None)),
            [PlayerCommand::SetSubtitle(None)]
        );
        assert_eq!(one(Command::Stop), [PlayerCommand::Stop]);
    }

    #[test]
    fn speed_and_subtitle_delay_from_the_phone_show_on_the_desktop_too() {
        let status = video();
        let mut controls = Controls::default();
        assert_eq!(
            player_commands(&Command::Speed(1.5), &status, &mut controls),
            [PlayerCommand::SetSpeed(1.5)]
        );
        assert_eq!(controls.speed(), 1.5);
        assert_eq!(
            player_commands(&Command::Speed(1.5), &status, &mut controls),
            [],
            "already at that speed"
        );
        assert_eq!(
            player_commands(&Command::SubtitleDelay(-0.25), &status, &mut controls),
            [PlayerCommand::SetSubtitleDelay(-0.25)]
        );
        assert_eq!(controls.subtitle_delay_s(), -0.25);
    }

    #[test]
    fn the_phone_sees_what_plays_with_its_tracks() {
        let mut controls = Controls::default();
        let _ = controls.set_speed(2.0);
        let shown = now_playing("Dune", &video(), &controls);
        assert_eq!(shown.title, "Dune");
        assert!(!shown.loading && !shown.paused);
        assert_eq!(shown.position_s, 30.0);
        assert_eq!(shown.duration_s, 600.0);
        assert_eq!(shown.volume, 70.0);
        assert_eq!(shown.speed, 2.0);
        assert_eq!(
            shown.audio,
            [Track {
                id: 1,
                label: Some("en · Stereo".into()),
                selected: true,
            }]
        );
        assert_eq!(
            shown.subtitles,
            [Track {
                id: 2,
                label: Some("es".into()),
                selected: false,
            }]
        );
        assert!(now_playing("Dune", &PlayerStatus::default(), &controls).loading);
    }

    #[test]
    fn the_remote_control_is_on_unless_it_was_turned_off() {
        let key = RemoteKey::from_bytes([1; 16]);
        let restore = |host| Action::RestoreRemote {
            host,
            peer: None,
            choice: None,
        };
        assert_eq!(
            at_start(Hosting::Unchosen, || Ok(key)),
            [restore(None), Action::SetRemoteHost(Some(key))]
        );
        assert_eq!(
            at_start(Hosting::Off, || panic!("no key is made")),
            [restore(None)]
        );
        assert_eq!(
            at_start(Hosting::On(key), || panic!("no key is made")),
            [restore(Some(key))]
        );
        assert_eq!(
            at_start(Hosting::Unchosen, || Err(RemoteError::Crypto)),
            [restore(None)]
        );
    }

    #[test]
    fn only_a_listener_with_a_local_address_is_announced() {
        let ip = std::net::IpAddr::from([192, 168, 1, 20]);
        assert_eq!(
            listening_address(&RemoteListener::On {
                ip: Some(ip),
                port: 18_765
            }),
            Some(std::net::SocketAddr::new(ip, 18_765))
        );
        assert_eq!(
            listening_address(&RemoteListener::On { ip: None, port: 1 }),
            None
        );
        assert_eq!(listening_address(&RemoteListener::Off), None);
        assert_eq!(
            listening_address(&RemoteListener::Failed("busy".into())),
            None
        );
    }

    #[test]
    fn the_phone_gets_the_latest_published_status() {
        let host = Host::default();
        let playing = Status {
            playing: Some(NowPlaying {
                title: "Alien".into(),
                ..NowPlaying::default()
            }),
        };
        host.publish(playing.clone());
        assert_eq!(*host.shown.lock().unwrap(), playing);
        host.publish(Status::default());
        assert_eq!(*host.shown.lock().unwrap(), Status::default());
    }
}
