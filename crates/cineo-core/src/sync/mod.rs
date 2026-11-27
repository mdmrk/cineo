mod clock;
mod pairing;
pub mod protocol;
mod record;
mod replica;

pub const DEFAULT_SERVER: &str = "https://cineo-sync.cineo-sync-server.workers.dev/";

pub use clock::{Clock, DeviceId, Stamp};
pub use pairing::{Pairing, SyncKey};
pub use record::{FORMAT, Record, RecordName, Value};
pub use replica::{
    Changes, Entry, Merged, Outgoing, PULL_EVERY_MS, PUSH_AFTER_QUIET_MS, PUSH_AT_MOST_MS, Replica,
    VERIFY_EVERY_MS,
};
