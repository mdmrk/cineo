use std::sync::mpsc::{self, Receiver, Sender};

use tracing::warn;
#[cfg(target_os = "linux")]
use zbus::Connection;

#[cfg(target_os = "linux")]
const SERVICE: &str = "org.freedesktop.ScreenSaver";
#[cfg(target_os = "linux")]
const PATH: &str = "/org/freedesktop/ScreenSaver";

pub(crate) struct KeepAwake {
    tx: Option<Sender<bool>>,
    on: bool,
    system: Option<fastframe_power::KeepAwake>,
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
            system: None,
        }
    }

    pub(crate) fn set(&mut self, on: bool) {
        if on == self.on {
            return;
        }
        self.on = on;
        self.system = on.then(|| fastframe_power::KeepAwake::begin("Playing a video"));
        if let Some(tx) = &self.tx {
            let _ = tx.send(on);
        }
    }
}

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
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

#[cfg(any(windows, target_os = "macos"))]
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

#[cfg(test)]
mod tests {
    use super::KeepAwake;

    #[test]
    fn the_system_stays_awake_only_while_on() {
        let mut awake = KeepAwake::new();
        assert!(awake.system.is_none());
        awake.set(true);
        assert!(
            awake
                .system
                .as_ref()
                .is_some_and(fastframe_power::KeepAwake::is_held)
        );
        awake.set(false);
        assert!(awake.system.is_none());
    }
}
