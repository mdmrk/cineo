use crate::addon::{SourceKind, TransportUrl};

/// Why a value could not load; the shell words it (ADR-0017).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    AlreadyInstalled,
    ConfigurationRequired(TransportUrl),
    InvalidAddonUrl(String),
    NoMetaAddon,
    UnsupportedFilter,
    Request(String),
}

/// A message for the notice bar; the shell words it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    AddonUnavailable(String),
    UnsupportedScheme(String),
    TorrentsOff,
    UnsupportedSource(SourceKind),
    PlaybackFailed {
        failure: PlaybackFailure,
        pick_another: bool,
    },
    UnexpectedEngineAddress,
    SubtitleFailed(String),
    SaveFailed(String),
    StoreClosed,
    InvalidLink,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaybackFailure {
    Player(String),
    Torrent(String),
}
