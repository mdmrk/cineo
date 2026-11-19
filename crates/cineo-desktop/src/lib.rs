#![forbid(unsafe_code)]

mod app;
mod brand;
pub mod dirs;
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
#[cfg(any(windows, test))]
#[cfg_attr(
    not(windows),
    expect(dead_code, reason = "only the Windows shell installs updates")
)]
pub mod self_update;
pub mod settings;
mod theme;
pub mod view;

pub use app::{Options, run};
