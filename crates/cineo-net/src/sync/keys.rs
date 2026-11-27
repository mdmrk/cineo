use std::fmt;

use aws_lc_rs::aead::{self, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};
use aws_lc_rs::{hkdf, hmac, rand};
use cineo_core::sync::protocol::{RECORD_KEY_LEN, encode};
use cineo_core::sync::{DeviceId, Record, RecordName, SyncKey};
use zeroize::Zeroizing;

use super::SyncError;

pub const BLOB_FORMAT: u8 = 1;
const SALT: &[u8] = b"cineo-sync-v1";
const TAG_LEN: usize = 16;

fn random<const N: usize>() -> Result<[u8; N], SyncError> {
    let mut bytes = [0; N];
    rand::fill(&mut bytes).map_err(|_| SyncError::Crypto)?;
    Ok(bytes)
}

pub fn new_key() -> Result<SyncKey, SyncError> {
    random().map(SyncKey::from_bytes)
}

pub fn new_device() -> Result<DeviceId, SyncError> {
    random().map(|bytes| DeviceId(u64::from_le_bytes(bytes)))
}

struct Len32;

impl hkdf::KeyType for Len32 {
    fn len(&self) -> usize {
        32
    }
}

pub struct SpaceKeys {
    token: Zeroizing<String>,
    names: hmac::Key,
    seal: LessSafeKey,
}

impl fmt::Debug for SpaceKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SpaceKeys(..)")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sealed {
    pub key: [u8; RECORD_KEY_LEN],
    pub blob: Vec<u8>,
}

impl SpaceKeys {
    pub fn derive(key: &SyncKey) -> Result<Self, SyncError> {
        let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, SALT).extract(key.as_bytes());
        let expand = |info: &[u8]| -> Result<Zeroizing<[u8; 32]>, SyncError> {
            let mut out = Zeroizing::new([0; 32]);
            prk.expand(&[info], Len32)
                .and_then(|okm| okm.fill(out.as_mut()))
                .map_err(|_| SyncError::Crypto)?;
            Ok(out)
        };
        let token = expand(b"token")?;
        let names = expand(b"names")?;
        let seal = expand(b"seal")?;
        Ok(Self {
            token: Zeroizing::new(encode(token.as_ref())),
            names: hmac::Key::new(hmac::HMAC_SHA256, names.as_ref()),
            seal: LessSafeKey::new(
                UnboundKey::new(&aead::CHACHA20_POLY1305, seal.as_ref())
                    .map_err(|_| SyncError::Crypto)?,
            ),
        })
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn record_key(&self, name: &RecordName) -> [u8; RECORD_KEY_LEN] {
        let tag = hmac::sign(&self.names, name.to_string().as_bytes());
        let mut key = [0; RECORD_KEY_LEN];
        key.copy_from_slice(&tag.as_ref()[..RECORD_KEY_LEN]);
        key
    }

    pub fn seal(&self, record: &Record) -> Result<Sealed, SyncError> {
        let key = self.record_key(&record.name);
        let nonce: [u8; NONCE_LEN] = random()?;
        let mut sealed = record.to_json().into_bytes();
        self.seal
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(aad(&key)),
                &mut sealed,
            )
            .map_err(|_| SyncError::Crypto)?;
        let blob = [[BLOB_FORMAT].as_slice(), &nonce, &sealed].concat();
        Ok(Sealed { key, blob })
    }

    pub fn open(&self, key: &[u8], blob: &[u8]) -> Option<Record> {
        let key: [u8; RECORD_KEY_LEN] = key.try_into().ok()?;
        let (&format, rest) = blob.split_first()?;
        if format != BLOB_FORMAT || rest.len() < NONCE_LEN + TAG_LEN {
            return None;
        }
        let (nonce, sealed) = rest.split_at(NONCE_LEN);
        let mut buffer = sealed.to_vec();
        let plain = self
            .seal
            .open_in_place(
                Nonce::try_assume_unique_for_key(nonce).ok()?,
                Aad::from(aad(&key)),
                &mut buffer,
            )
            .ok()?;
        let record = Record::parse(std::str::from_utf8(plain).ok()?)?;
        (self.record_key(&record.name) == key).then_some(record)
    }
}

fn aad(key: &[u8; RECORD_KEY_LEN]) -> [u8; RECORD_KEY_LEN + 1] {
    let mut aad = [0; RECORD_KEY_LEN + 1];
    aad[0] = BLOB_FORMAT;
    aad[1..].copy_from_slice(key);
    aad
}
