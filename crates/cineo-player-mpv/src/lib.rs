//! Plays streams in an external mpv process (ADR-0004).
//!
//! Safety rules (docs/SECURITY.md): only `http(s)` URLs; media is loaded with
//! the IPC `loadfile` command (a JSON array, never a command string or a
//! command-line argument); mpv starts with `--no-config --ytdl=no`; header
//! names/values were validated by the core parser and are re-checked here.

mod ipc;
mod process;
mod tracker;

pub use ipc::{PlayerEvent, drive_session};
pub use process::{PlayerError, PlayerHandle, launch};
