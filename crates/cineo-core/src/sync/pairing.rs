use std::fmt;

use url::Url;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SyncKey([u8; 16]);

impl SyncKey {
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl fmt::Debug for SyncKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SyncKey(..)")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pairing {
    pub key: SyncKey,
    pub server: Url,
}
