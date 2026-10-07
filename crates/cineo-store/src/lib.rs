//! Persistence for Cineo: installed addons in order, library items and watch
//! progress, and user settings, in one SQLite database.

mod doctor;
mod migrate;
mod store;

pub use doctor::{Report, diagnose};
pub use migrate::SCHEMA_VERSION;
pub use store::{DB_FILE, Store, StoreError, default_cache_dir, default_data_dir};
