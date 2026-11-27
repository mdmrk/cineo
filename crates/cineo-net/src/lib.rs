#![forbid(unsafe_code)]

mod addon_client;
mod effects;
mod policy;
pub mod sync;

pub use addon_client::{AddonClient, FetchError, MAX_IMAGE_BYTES};
pub use effects::fetch_task;
pub use policy::{BlockReason, NetPolicy};
