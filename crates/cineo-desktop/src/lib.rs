//! Cineo's desktop GUI shell (eframe/egui, ADR-0011).

mod app;
mod images;
pub mod player;
mod theme;
pub mod view;

pub use app::{Options, run};
