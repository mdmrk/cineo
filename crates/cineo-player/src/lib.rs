//! Plays streams with libmpv, loaded at runtime and drawing into the host
//! window ([`embedded`], ADR-0014).

pub mod embedded;
mod tracker;

/// What the player reports back to the shell.
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEvent {
    Progress { time_ms: u64, duration_ms: u64 },
    Ended { time_ms: u64, duration_ms: u64 },
    Failed(String),
    Closed { time_ms: u64, duration_ms: u64 },
}

/// Why playback could not start.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PlayerError {
    #[error("only http(s) streams can be played, got `{0}`")]
    UnsupportedScheme(String),
    #[error("playback needs libmpv (mpv 0.35 or newer): {0}")]
    LibmpvUnavailable(String),
    #[error("libmpv failed: {0}")]
    Libmpv(String),
    #[error("cannot show video in the window: {0}")]
    Render(String),
}
