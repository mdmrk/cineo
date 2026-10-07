//! Cineo's desktop GUI shell (eframe/egui, ADR-0011).

mod app;
mod images;
pub mod player;
pub mod settings;
mod subtitles;
mod theme;
pub mod view;

pub use app::{Options, run};
