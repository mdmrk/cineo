//! Application state machine (ADR-0001): pure `update(state, action)`
//! returning effects for the shell to execute. No IO happens here.

mod language;
mod library;
mod message;
mod plan;
mod settings;
mod state;
mod torrent;

pub use language::Language;
pub use library::{LibraryItem, SavedStream, continue_watching};
pub use message::{Notice, PlaybackFailure, Problem};
pub use plan::{
    CatalogTarget, board_targets, meta_candidates, search_targets, stream_targets, subtitle_targets,
};
pub use settings::{
    AudioOutput, DownloadLimit, HideControls, InterfaceScale, PeerLimit, Percent, Ranged, SeekStep,
    Setting, SettingError, Settings, ShortSeekStep, StartPage, SubtitleBackground, SubtitleColor,
    SubtitleFont, SubtitleOpacity, SubtitleOutline, SubtitlePosition, SubtitleSize, UiLanguage,
    UploadLimit, WatchedAt,
};
pub use state::{
    Action, Detail, Discover, Effect, InstalledAddon, Loadable, PlayRequest, Playing, Row, State,
    StreamGroup, SubtitleGroup, update,
};
pub use torrent::{TorrentFile, TorrentPlayback, TorrentRequest, TorrentStatus, choose_file};
