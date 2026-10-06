//! Persistence for Cineo: installed addons in order, library items and watch
//! progress, and user settings, in one SQLite database.
//!
//! The core decides *what* is stored (the `Save*`/`Delete*` variants of
//! [`cineo_core::app::Effect`]); this crate decides *how*. Rules
//! (docs/SECURITY.md): file paths come from platform directories only, never
//! from addon data; SQL uses bound parameters only; a database this version
//! cannot read (corrupt, or a newer schema) is reported and left untouched.

mod doctor;
mod migrate;
mod store;

pub use doctor::{Report, diagnose};
pub use migrate::SCHEMA_VERSION;
pub use store::{DB_FILE, Store, StoreError, default_data_dir};
