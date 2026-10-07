//! Plays streams with mpv: embedded in the host window through libmpv
//! loaded at runtime ([`embedded`], ADR-0014), or as an external mpv process
//! over JSON IPC when libmpv is unavailable (ADR-0004).
//!
//! Safety rules (docs/SECURITY.md): only `http(s)` URLs; media is loaded with
//! an argument-array `loadfile` (never a command string or a command-line
//! argument); mpv starts without user config, scripts or ytdl; header
//! names/values were validated by the core parser and are re-checked here.
//! `unsafe` is confined to `embedded::ffi` and `embedded::render`.

pub mod embedded;
mod ipc;
mod process;
mod tracker;

pub use ipc::{PlayerEvent, drive_session};
pub use process::{PlayerError, PlayerHandle, launch};
