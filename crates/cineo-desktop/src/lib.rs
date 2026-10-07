//! Cineo's desktop GUI shell (eframe/egui, ADR-0011).
//!
//! [`view::show`] is pure rendering over a core `State` and is what the UI
//! tests drive; the binary wires it to IO (`app`), images (`images`) and the
//! theme.

mod app;
mod images;
pub mod player;
mod theme;
pub mod view;

pub use app::{Options, run};
