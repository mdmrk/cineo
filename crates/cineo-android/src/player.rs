use std::sync::Arc;

use cineo_core::app::PlayRequest;
use cineo_player::embedded::{Player, PlayerCommand, Status};
use cineo_player::{PlayerError, PlayerEvent};
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::error;

pub trait Playback: Send + Sync + std::fmt::Debug {
    fn attach_surface(&self, wid: i64) -> Result<(), PlayerError>;
    fn detach_surface(&self) -> Result<(), PlayerError>;
    fn resize_surface(&self, width: u32, height: u32) -> Result<(), PlayerError>;
    fn send(&self, command: PlayerCommand);
    fn status(&self) -> Status;
}

pub type Started = (Box<dyn Playback>, UnboundedReceiver<PlayerEvent>);

pub trait Players: Send + Sync + std::fmt::Debug {
    fn start(&self, request: &PlayRequest) -> Result<Started, PlayerError>;
}

#[derive(Debug)]
pub(crate) struct Mpv;

impl Players for Mpv {
    fn start(&self, request: &PlayRequest) -> Result<Started, PlayerError> {
        let (player, events) = Player::start_on_surface(request, Arc::new(|| {}))?;
        Ok((Box::new(OwnThreadDrop(Some(player))), events))
    }
}

#[derive(Debug)]
pub(crate) struct OwnThreadDrop<T: Send + 'static>(Option<T>);

impl<T: Send + 'static> OwnThreadDrop<T> {
    fn get(&self) -> &T {
        self.0.as_ref().unwrap_or_else(|| unreachable!())
    }
}

impl<T: Send + 'static> Drop for OwnThreadDrop<T> {
    fn drop(&mut self) {
        let inner = self.0.take();
        let spawned = std::thread::Builder::new()
            .name("mpv-release".into())
            .spawn(move || drop(inner));
        match spawned {
            Ok(thread) => {
                if thread.join().is_err() {
                    error!("releasing the player panicked");
                }
            }
            Err(err) => error!(%err, "cannot start a thread to release the player"),
        }
    }
}

impl Playback for OwnThreadDrop<Player> {
    fn attach_surface(&self, wid: i64) -> Result<(), PlayerError> {
        self.get().attach_surface(wid)
    }

    fn detach_surface(&self) -> Result<(), PlayerError> {
        self.get().detach_surface()
    }

    fn resize_surface(&self, width: u32, height: u32) -> Result<(), PlayerError> {
        self.get().resize_surface(width, height)
    }

    fn send(&self, command: PlayerCommand) {
        self.get().send(command);
    }

    fn status(&self) -> Status {
        self.get().status()
    }
}

#[cfg(test)]
mod tests {
    use std::thread::ThreadId;

    use super::*;

    struct Probe(std::sync::mpsc::Sender<ThreadId>);

    impl Drop for Probe {
        fn drop(&mut self) {
            let _ = self.0.send(std::thread::current().id());
        }
    }

    #[test]
    fn the_player_is_released_on_its_own_thread_before_drop_returns() {
        let (tx, rx) = std::sync::mpsc::channel();
        drop(OwnThreadDrop(Some(Probe(tx))));
        let released_on = rx.try_recv().unwrap();
        assert_ne!(released_on, std::thread::current().id());
    }
}
