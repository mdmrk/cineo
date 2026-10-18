use crate::addon::{SourceKind, TransportUrl};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    AlreadyInstalled,
    ConfigurationRequired(TransportUrl),
    InvalidAddonUrl(String),
    NoMetaAddon,
    UnsupportedFilter,
    Request(String),
}

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
