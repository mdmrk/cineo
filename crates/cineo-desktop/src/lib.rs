mod app;
mod brand;
pub mod i18n;
mod images;
pub mod instance;
mod keep_awake;
pub mod libmpv;
mod media_keys;
#[cfg(not(windows))]
mod mpris;
pub mod player;
mod register;
pub mod settings;
#[cfg(windows)]
mod smtc;
mod theme;
pub mod view;

pub use app::{Options, run};

rust_i18n::i18n!("assets/locales", fallback = "en");
