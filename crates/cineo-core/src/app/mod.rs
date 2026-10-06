//! Application state machine (ADR-0001): pure `update(state, action)`
//! returning effects for the shell to execute. No IO happens here.

mod library;
mod plan;
mod state;

pub use library::{LibraryItem, continue_watching};
pub use plan::{CatalogTarget, board_targets, meta_candidates, search_targets, stream_targets};
pub use state::{
    Action, Detail, Discover, Effect, InstalledAddon, Loadable, PlayRequest, Row, State,
    StreamGroup, update,
};
