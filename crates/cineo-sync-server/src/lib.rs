#![forbid(unsafe_code)]

mod db;
mod handler;
mod limit;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod worker_host;

pub use db::DbError;
pub use handler::{
    BUSY_RETRY_S, CREATE_RETRY_S, Context, IDLE_MS, Method, Outcome, Request, Response, SEQS_PER_MS,
};
pub use limit::{REQUESTS_PER_MINUTE, RateLimiter, SPACES_PER_HOUR};
#[cfg(not(target_arch = "wasm32"))]
pub use native::{ServeOptions, SqliteDb, serve, system_ms};

pub const SCHEMA: &str = include_str!("../schema.sql");
