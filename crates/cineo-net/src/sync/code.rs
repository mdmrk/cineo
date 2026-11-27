use aws_lc_rs::digest::{SHA256, digest};
use cineo_core::sync::{DEFAULT_SERVER, Pairing, SyncKey};
use data_encoding::BASE32_NOPAD;

const CHECK_LEN: usize = 4;
const CODE_LEN: usize = 32;
const GROUP: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CodeError {
    #[error("a sync code has {CODE_LEN} letters and digits")]
    Length,
    #[error("the sync code has a character that sync codes never use")]
    Character,
    #[error("the sync code has a typo")]
    Typo,
}

fn check(key: &[u8; 16]) -> [u8; CHECK_LEN] {
    let sum = digest(&SHA256, &[b"cineo-sync-check".as_slice(), key].concat());
    let mut check = [0; CHECK_LEN];
    check.copy_from_slice(&sum.as_ref()[..CHECK_LEN]);
    check
}

pub fn format_code(key: &SyncKey) -> String {
    let bytes = [key.as_bytes().as_slice(), &check(key.as_bytes())].concat();
    let mut code = String::with_capacity(CODE_LEN + CODE_LEN / GROUP);
    for (index, letter) in BASE32_NOPAD.encode(&bytes).chars().enumerate() {
        if index > 0 && index % GROUP == 0 {
            code.push('-');
        }
        code.push(letter);
    }
    code
}

pub fn parse_code(text: &str) -> Result<SyncKey, CodeError> {
    let cleaned: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| match c.to_ascii_uppercase() {
            '0' => 'O',
            '1' => 'I',
            other => other,
        })
        .collect();
    if cleaned.chars().count() != CODE_LEN {
        return Err(CodeError::Length);
    }
    let bytes = BASE32_NOPAD
        .decode(cleaned.as_bytes())
        .map_err(|_| CodeError::Character)?;
    let (key, sum) = bytes.split_at(16);
    let key: [u8; 16] = key.try_into().map_err(|_| CodeError::Length)?;
    if check(&key) != sum {
        return Err(CodeError::Typo);
    }
    Ok(SyncKey::from_bytes(key))
}

pub fn pairing_link(pairing: &Pairing) -> String {
    let mut link = format!("cineo:///sync/{}", format_code(&pairing.key));
    if pairing.server.as_str() != DEFAULT_SERVER {
        link.push_str("?server=");
        link.extend(url::form_urlencoded::byte_serialize(
            pairing.server.as_str().as_bytes(),
        ));
    }
    link
}
