use std::sync::Arc;

use cineo_core::app::PlayRequest;
use cineo_player::embedded::{Player, PlayerCommand, Status};
use cineo_player::{PlayerError, PlayerEvent};
use tokio::sync::mpsc::UnboundedReceiver;

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
        Ok((Box::new(player), events))
    }
}

impl Playback for Player {
    fn attach_surface(&self, wid: i64) -> Result<(), PlayerError> {
        Player::attach_surface(self, wid)
    }

    fn detach_surface(&self) -> Result<(), PlayerError> {
        Player::detach_surface(self)
    }

    fn resize_surface(&self, width: u32, height: u32) -> Result<(), PlayerError> {
        Player::resize_surface(self, width, height)
    }

    fn send(&self, command: PlayerCommand) {
        Player::send(self, command);
    }

    fn status(&self) -> Status {
        Player::status(self)
    }
}
