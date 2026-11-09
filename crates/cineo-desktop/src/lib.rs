mod app;
mod brand;
pub mod i18n;
mod image_disk;
mod images;
pub mod instance;
mod keep_awake;
pub mod libmpv;
pub mod logging;
mod media_keys;
pub mod panic_log;
pub mod player;
mod register;
pub mod settings;
mod theme;
pub mod view;

pub use app::{Options, run};

rust_i18n::i18n!("assets/locales", fallback = "en");
