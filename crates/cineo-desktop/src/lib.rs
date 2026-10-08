//! Cineo's desktop GUI shell (eframe/egui, ADR-0011).

mod app;
mod brand;
pub mod i18n;
mod images;
pub mod instance;
pub mod player;
mod register;
pub mod settings;
mod subtitles;
mod theme;
pub mod view;

pub use app::{Options, run};
