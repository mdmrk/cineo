use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};

use eframe::egui;
use tracing::warn;
use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;
use zbus::interface;
use zbus::zvariant::{ObjectPath, OwnedValue, Str};

const PATH: &str = "/org/mpris/MediaPlayer2";
const TRACK: ObjectPath<'static> =
    ObjectPath::from_static_str_unchecked("/io/github/mdmrk/cineo/track");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Remote {
    Play,
    Pause,
    PlayPause,
    Stop,
    Raise,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Playing {
    title: String,
    paused: bool,
}

pub(crate) struct Mpris {
    tx: Option<Sender<Option<Playing>>>,
    remote: Receiver<Remote>,
    last: Option<Playing>,
}

impl Mpris {
    pub(crate) fn new(ctx: egui::Context) -> Self {
        let (tx, rx) = mpsc::channel();
        let (remote_tx, remote) = mpsc::channel();
        let sink = Sink { tx: remote_tx, ctx };
        let spawned = std::thread::Builder::new()
            .name("mpris".into())
            .spawn(move || run(&rx, &sink));
        if let Err(err) = &spawned {
            warn!(%err, "could not start the media-keys thread");
        }
        Self {
            tx: spawned.ok().map(|_| tx),
            remote,
            last: None,
        }
    }

    pub(crate) fn set(&mut self, playing: Option<(&str, bool)>) {
        let same = match (&self.last, playing) {
            (None, None) => true,
            (Some(last), Some((title, paused))) => last.title == title && last.paused == paused,
            _ => false,
        };
        if same {
            return;
        }
        self.last = playing.map(|(title, paused)| Playing {
            title: title.to_owned(),
            paused,
        });
        if let Some(tx) = &self.tx {
            let _ = tx.send(self.last.clone());
        }
    }

    pub(crate) fn take(&self) -> Vec<Remote> {
        self.remote.try_iter().collect()
    }
}

#[derive(Clone)]
struct Sink {
    tx: Sender<Remote>,
    ctx: egui::Context,
}

impl Sink {
    fn send(&self, remote: Remote) {
        let _ = self.tx.send(remote);
        self.ctx.request_repaint();
    }
}

fn run(rx: &Receiver<Option<Playing>>, sink: &Sink) {
    let mut bus: Option<Connection> = None;
    for update in rx {
        let Some(playing) = update else {
            bus = None;
            continue;
        };
        let result = match bus.take() {
            Some(conn) => {
                let changed = change(&conn, playing);
                bus = Some(conn);
                changed
            }
            None => serve(playing, sink).map(|conn| bus = Some(conn)),
        };
        if let Err(err) = result {
            warn!(%err, "media keys and the desktop's media controls may not work");
        }
    }
}

fn serve(playing: Playing, sink: &Sink) -> zbus::Result<Connection> {
    Builder::session()?
        .serve_at(PATH, Root { sink: sink.clone() })?
        .serve_at(
            PATH,
            Player {
                playing,
                sink: sink.clone(),
            },
        )?
        .name(format!(
            "org.mpris.MediaPlayer2.cineo.instance{}",
            std::process::id()
        ))?
        .build()
}

fn change(conn: &Connection, playing: Playing) -> zbus::Result<()> {
    let iface = conn.object_server().interface::<_, Player>(PATH)?;
    let mut player = iface.get_mut();
    let status = player.playing.paused != playing.paused;
    let title = player.playing.title != playing.title;
    player.playing = playing;
    let emitter = iface.signal_emitter();
    if status {
        zbus::block_on(player.playback_status_changed(emitter))?;
    }
    if title {
        zbus::block_on(player.metadata_changed(emitter))?;
    }
    Ok(())
}

struct Root {
    sink: Sink,
}

#[interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    fn raise(&self) {
        self.sink.send(Remote::Raise);
    }

    fn quit(&self) {}

    #[zbus(property)]
    fn can_quit(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn can_raise(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn identity(&self) -> String {
        "Cineo".into()
    }

    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<String> {
        Vec::new()
    }

    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<String> {
        Vec::new()
    }
}

struct Player {
    playing: Playing,
    sink: Sink,
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    fn play_pause(&self) {
        self.sink.send(Remote::PlayPause);
    }

    fn play(&self) {
        self.sink.send(Remote::Play);
    }

    fn pause(&self) {
        self.sink.send(Remote::Pause);
    }

    fn stop(&self) {
        self.sink.send(Remote::Stop);
    }

    fn next(&self) {}

    fn previous(&self) {}

    #[zbus(property)]
    fn playback_status(&self) -> String {
        if self.playing.paused {
            "Paused".into()
        } else {
            "Playing".into()
        }
    }

    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, OwnedValue> {
        HashMap::from([
            ("mpris:trackid".to_owned(), OwnedValue::from(TRACK)),
            (
                "xesam:title".to_owned(),
                OwnedValue::from(Str::from(self.playing.title.clone())),
            ),
        ])
    }

    #[zbus(property)]
    fn rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn minimum_rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn maximum_rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn can_play(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_pause(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_seek(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }
}
