use std::fmt;

use aws_lc_rs::aead::{self, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};
use aws_lc_rs::{hkdf, rand};
use cineo_core::remote::RemoteKey;
use zeroize::Zeroizing;

use super::RemoteError;

const FORMAT: u8 = 1;
const SALT: &[u8] = b"cineo-remote-v1";
const REQUEST: &[u8] = b"cineo-remote-v1 request";
const ANSWER: &[u8] = b"cineo-remote-v1 answer";
const TAG_LEN: usize = 16;

struct Len32;

impl hkdf::KeyType for Len32 {
    fn len(&self) -> usize {
        32
    }
}

pub struct RemoteKeys {
    seal: LessSafeKey,
}

impl fmt::Debug for RemoteKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RemoteKeys(..)")
    }
}

pub fn new_remote_key() -> Result<RemoteKey, RemoteError> {
    random().map(RemoteKey::from_bytes)
}

fn random<const N: usize>() -> Result<[u8; N], RemoteError> {
    let mut bytes = [0; N];
    rand::fill(&mut bytes).map_err(|_| RemoteError::Crypto)?;
    Ok(bytes)
}

impl RemoteKeys {
    pub fn derive(key: &RemoteKey) -> Result<Self, RemoteError> {
        let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, SALT).extract(key.as_bytes());
        let mut seal = Zeroizing::new([0; 32]);
        prk.expand(&[b"seal"], Len32)
            .and_then(|okm| okm.fill(seal.as_mut()))
            .map_err(|_| RemoteError::Crypto)?;
        Ok(Self {
            seal: LessSafeKey::new(
                UnboundKey::new(&aead::CHACHA20_POLY1305, seal.as_ref())
                    .map_err(|_| RemoteError::Crypto)?,
            ),
        })
    }

    pub fn seal_request(&self, plain: &[u8]) -> Result<([u8; NONCE_LEN], Vec<u8>), RemoteError> {
        let nonce = random()?;
        let blob = self.seal(nonce, REQUEST.to_vec(), plain)?;
        Ok((nonce, blob))
    }

    pub fn open_request(&self, blob: &[u8]) -> Option<([u8; NONCE_LEN], Vec<u8>)> {
        self.open(blob, |_| REQUEST.to_vec())
    }

    pub fn seal_answer(
        &self,
        request: &[u8; NONCE_LEN],
        plain: &[u8],
    ) -> Result<Vec<u8>, RemoteError> {
        self.seal(random()?, answer_aad(request), plain)
    }

    pub fn open_answer(&self, request: &[u8; NONCE_LEN], blob: &[u8]) -> Option<Vec<u8>> {
        self.open(blob, |_| answer_aad(request))
            .map(|(_, plain)| plain)
    }

    fn seal(
        &self,
        nonce: [u8; NONCE_LEN],
        aad: Vec<u8>,
        plain: &[u8],
    ) -> Result<Vec<u8>, RemoteError> {
        let mut sealed = plain.to_vec();
        self.seal
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(aad),
                &mut sealed,
            )
            .map_err(|_| RemoteError::Crypto)?;
        Ok([[FORMAT].as_slice(), &nonce, &sealed].concat())
    }

    fn open(
        &self,
        blob: &[u8],
        aad: impl FnOnce(&[u8; NONCE_LEN]) -> Vec<u8>,
    ) -> Option<([u8; NONCE_LEN], Vec<u8>)> {
        let (&format, rest) = blob.split_first()?;
        if format != FORMAT || rest.len() < NONCE_LEN + TAG_LEN {
            return None;
        }
        let (nonce, sealed) = rest.split_at(NONCE_LEN);
        let nonce: [u8; NONCE_LEN] = nonce.try_into().ok()?;
        let mut buffer = sealed.to_vec();
        let plain = self
            .seal
            .open_in_place(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(aad(&nonce)),
                &mut buffer,
            )
            .ok()?;
        Some((nonce, plain.to_vec()))
    }
}

fn answer_aad(request: &[u8; NONCE_LEN]) -> Vec<u8> {
    [ANSWER, request].concat()
}
