//! `cineo-stream`: the local streaming engine (ADR-0010, ADR-0012).
//!
//! It turns a torrent into an `http://127.0.0.1:<port>/<token>/…` URL the
//! player can read with range requests. `librqbit` does the BitTorrent work;
//! this crate decides where data is stored (the bounded cache, with
//! engine-chosen file names), which addresses peers and trackers may have,
//! and how files are served. Which file to play is the core's decision
//! ([`cineo_core::app::choose_file`]).

mod blocklist;
mod cache;
mod engine;
mod range;
mod server;
mod socks;
mod storage;

pub use engine::{Engine, EngineOptions, StreamError};
