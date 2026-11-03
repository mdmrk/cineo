use std::sync::mpsc::{self, Receiver, Sender};

use tracing::warn;
#[cfg(not(windows))]
use zbus::Connection;

#[cfg(not(windows))]
const SERVICE: &str = "org.freedesktop.ScreenSaver";
#[cfg(not(windows))]
const PATH: &str = "/org/freedesktop/ScreenSaver";

pub(crate) struct KeepAwake {
    tx: Option<Sender<bool>>,
    on: bool,
}

impl KeepAwake {
    pub(crate) fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("keep-awake".into())
            .spawn(move || run(&rx));
        if let Err(err) = &spawned {
            warn!(%err, "could not start the screen-saver thread");
        }
        Self {
            tx: spawned.ok().map(|_| tx),
            on: false,
        }
    }

    pub(crate) fn set(&mut self, on: bool) {
        if on == self.on {
            return;
        }
        self.on = on;
        if let Some(tx) = &self.tx {
            let _ = tx.send(on);
        }
    }
}

#[cfg(not(windows))]
fn run(rx: &Receiver<bool>) {
    let mut bus: Option<Connection> = None;
    let mut cookie: Option<u32> = None;
    for on in rx {
        if on == cookie.is_some() {
            continue;
        }
        let conn = match &bus {
            Some(conn) => conn,
            None => match zbus::block_on(Connection::session()) {
                Ok(conn) => bus.insert(conn),
                Err(err) => {
                    warn!(%err, "no session bus; the screen may sleep during playback");
                    continue;
                }
            },
        };
        if on {
            match inhibit(conn) {
                Ok(id) => cookie = Some(id),
                Err(err) => warn!(%err, "could not keep the screen awake"),
            }
        } else if let Some(id) = cookie.take()
            && let Err(err) = zbus::block_on(conn.call_method(
                Some(SERVICE),
                PATH,
                Some(SERVICE),
                "UnInhibit",
                &id,
            ))
        {
            warn!(%err, "could not let the screen sleep again");
        }
    }
}

#[cfg(not(windows))]
fn inhibit(conn: &Connection) -> zbus::Result<u32> {
    zbus::block_on(conn.call_method(
        Some(SERVICE),
        PATH,
        Some(SERVICE),
        "Inhibit",
        &("Cineo", "Playing a video"),
    ))?
    .body()
    .deserialize()
}

#[cfg(windows)]
fn run(rx: &Receiver<bool>) {
    let mut held: Option<keepawake::KeepAwake> = None;
    for on in rx {
        if !on {
            held = None;
            continue;
        }
        if held.is_some() {
            continue;
        }
        match keepawake::Builder::default()
            .display(true)
            .idle(true)
            .reason("Playing a video")
            .app_name("Cineo")
            .app_reverse_domain("io.github.mdmrk.cineo")
            .create()
        {
            Ok(awake) => held = Some(awake),
            Err(err) => warn!(%err, "could not keep the screen awake"),
        }
    }
}
