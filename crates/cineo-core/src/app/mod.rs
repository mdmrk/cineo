//! Application state machine (ADR-0001): pure `update(state, action)`
//! returning effects for the shell to execute. No IO happens here.

mod language;
mod library;
mod plan;
mod state;
mod torrent;

pub use language::{LANGUAGES, Language};
pub use library::{LibraryItem, continue_watching};
pub use plan::{
    CatalogTarget, board_targets, meta_candidates, search_targets, stream_targets, subtitle_targets,
};
pub use state::{
    Action, Detail, Discover, Effect, InstalledAddon, Loadable, PlayRequest, Row, State,
    StreamGroup, SubtitleGroup, update,
};
pub use torrent::{
    Settings, TorrentFile, TorrentPlayback, TorrentRequest, TorrentStatus, choose_file,
};
