use std::collections::{BTreeMap, BTreeSet};

use data_encoding::BASE64URL_NOPAD;
use serde_json::{Value as Json, json};

pub const PROTOCOL: u64 = 1;
pub const PUBLIC_KEY_LEN: usize = 32;
pub const SIGNATURE_LEN: usize = 64;
pub const MAX_CLOCK_SKEW_MS: u64 = 10 * 60 * 1000;
pub const WORK_BITS: u32 = 20;
pub const MAX_WORK_BITS: u32 = 24;
pub const WORK_HEADER: &str = "cineo-work";
const AUTH_SCHEME: &str = "Cineo-Ed25519";
const REQUEST_CONTEXT: &[u8] = b"cineo-sync-v1 request";
const WORK_CONTEXT: &[u8] = b"cineo-sync-v1 work";
pub const RECORD_KEY_LEN: usize = 16;
pub const MAX_BLOB_BYTES: usize = 32 * 1024;
pub const MAX_BATCH_RECORDS: usize = 64;
pub const MAX_BATCH_BYTES: usize = 256 * 1024;
pub const PULL_PAGE_RECORDS: usize = 500;
pub const MAX_PULL_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_SPACE_RECORDS: u64 = 10_000;
pub const MAX_SPACE_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ProtocolError {
    #[error("the body is not the expected JSON")]
    Malformed,
    #[error("more than {MAX_BATCH_RECORDS} writes")]
    TooManyWrites,
    #[error("a record key is not {RECORD_KEY_LEN} bytes")]
    BadKey,
    #[error("a blob is empty or larger than {MAX_BLOB_BYTES} bytes")]
    BadBlob,
    #[error("the same record key is written twice")]
    DuplicateKey,
}

pub fn encode(bytes: &[u8]) -> String {
    BASE64URL_NOPAD.encode(bytes)
}

pub fn decode(text: &str) -> Option<Vec<u8>> {
    BASE64URL_NOPAD.decode(text.as_bytes()).ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Authorization {
    pub key: [u8; PUBLIC_KEY_LEN],
    pub time_ms: u64,
    pub signature: [u8; SIGNATURE_LEN],
}

impl Authorization {
    pub fn to_header(&self) -> String {
        format!(
            "{AUTH_SCHEME} key={}, time={}, sig={}",
            encode(&self.key),
            self.time_ms,
            encode(&self.signature)
        )
    }

    pub fn parse(header: &str) -> Option<Self> {
        let params = header.strip_prefix(AUTH_SCHEME)?.strip_prefix(' ')?;
        let mut fields = BTreeMap::new();
        for param in params.split(',') {
            let (name, value) = param.trim().split_once('=')?;
            if fields.insert(name, value).is_some() {
                return None;
            }
        }
        if fields.len() != 3 {
            return None;
        }
        Some(Self {
            key: decode(fields.get("key")?)?.try_into().ok()?,
            time_ms: fields.get("time")?.parse().ok()?,
            signature: decode(fields.get("sig")?)?.try_into().ok()?,
        })
    }
}

pub fn request_target(path: &str, query: Option<&str>) -> Option<String> {
    let (_, tail) = path.rsplit_once("/v1/")?;
    Some(match query.filter(|q| !q.is_empty()) {
        Some(query) => format!("v1/{tail}?{query}"),
        None => format!("v1/{tail}"),
    })
}

pub fn signed_message(method: &str, target: &str, time_ms: u64, body_sha256: &[u8]) -> Vec<u8> {
    [
        REQUEST_CONTEXT,
        b"\n",
        method.as_bytes(),
        b"\n",
        target.as_bytes(),
        b"\n",
        time_ms.to_string().as_bytes(),
        b"\n",
        encode(body_sha256).as_bytes(),
    ]
    .concat()
}

pub fn work_message(key: &[u8; PUBLIC_KEY_LEN], nonce: u64) -> Vec<u8> {
    [WORK_CONTEXT, key, &nonce.to_le_bytes()].concat()
}

pub fn leading_zero_bits(digest: &[u8]) -> u32 {
    let mut bits = 0;
    for byte in digest {
        bits += byte.leading_zeros();
        if *byte != 0 {
            break;
        }
    }
    bits
}

pub fn work_required_json(bits: u32) -> String {
    json!({ "error": "proof of work required", "work_bits": bits }).to_string()
}

pub fn parse_work_required(bytes: &[u8]) -> Option<u32> {
    let json: Json = serde_json::from_slice(bytes).ok()?;
    u32::try_from(json.get("work_bits")?.as_u64()?).ok()
}

pub fn encoded_len(bytes: usize) -> usize {
    bytes.div_ceil(3) * 4
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireRecord {
    pub key: Vec<u8>,
    pub seq: u64,
    pub blob: Vec<u8>,
}

impl WireRecord {
    pub fn encoded_len(&self) -> usize {
        encoded_len(self.key.len()) + encoded_len(self.blob.len()) + 48
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PullPage {
    pub records: Vec<WireRecord>,
    pub skipped: usize,
    pub more: bool,
    pub space_seq: u64,
    pub born: u64,
}

impl PullPage {
    pub fn to_json(&self) -> String {
        json!({
            "records": self.records.iter().map(|r| json!({
                "key": encode(&r.key),
                "seq": r.seq,
                "blob": encode(&r.blob),
            })).collect::<Vec<_>>(),
            "more": self.more,
            "seq": self.space_seq,
            "born": self.born,
        })
        .to_string()
    }

    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let page: Json = serde_json::from_slice(bytes).ok()?;
        let mut parsed = Self {
            more: page.get("more").and_then(Json::as_bool).unwrap_or(false),
            space_seq: page.get("seq")?.as_u64()?,
            born: page.get("born")?.as_u64()?,
            ..Self::default()
        };
        for entry in page.get("records")?.as_array()? {
            let record = (|| {
                Some(WireRecord {
                    key: decode(entry.get("key")?.as_str()?)?,
                    seq: entry.get("seq")?.as_u64()?,
                    blob: decode(entry.get("blob")?.as_str()?)?,
                })
            })();
            match record {
                Some(record) => parsed.records.push(record),
                None => parsed.skipped += 1,
            }
        }
        Some(parsed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Write {
    pub key: Vec<u8>,
    pub base_seq: u64,
    pub blob: Vec<u8>,
}

impl Write {
    pub fn encoded_len(&self) -> usize {
        encoded_len(self.key.len()) + encoded_len(self.blob.len()) + 64
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushRequest {
    pub writes: Vec<Write>,
}

impl PushRequest {
    pub fn to_json(&self) -> String {
        json!({
            "writes": self.writes.iter().map(|w| json!({
                "key": encode(&w.key),
                "base_seq": w.base_seq,
                "blob": encode(&w.blob),
            })).collect::<Vec<_>>(),
        })
        .to_string()
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let body: Json = serde_json::from_slice(bytes).map_err(|_| ProtocolError::Malformed)?;
        let entries = body
            .get("writes")
            .and_then(Json::as_array)
            .ok_or(ProtocolError::Malformed)?;
        if entries.len() > MAX_BATCH_RECORDS {
            return Err(ProtocolError::TooManyWrites);
        }
        let mut seen = BTreeSet::new();
        let mut writes = Vec::with_capacity(entries.len());
        for entry in entries {
            let field = |name| {
                entry
                    .get(name)
                    .and_then(Json::as_str)
                    .and_then(decode)
                    .ok_or(ProtocolError::Malformed)
            };
            let key = field("key")?;
            let blob = field("blob")?;
            let base_seq = entry
                .get("base_seq")
                .and_then(Json::as_u64)
                .ok_or(ProtocolError::Malformed)?;
            if key.len() != RECORD_KEY_LEN {
                return Err(ProtocolError::BadKey);
            }
            if blob.is_empty() || blob.len() > MAX_BLOB_BYTES {
                return Err(ProtocolError::BadBlob);
            }
            if !seen.insert(key.clone()) {
                return Err(ProtocolError::DuplicateKey);
            }
            writes.push(Write {
                key,
                base_seq,
                blob,
            });
        }
        Ok(Self { writes })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushResponse {
    pub seqs: Vec<u64>,
}

impl PushResponse {
    pub fn to_json(&self) -> String {
        json!({ "seqs": self.seqs }).to_string()
    }

    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let body: Json = serde_json::from_slice(bytes).ok()?;
        let seqs = body
            .get("seqs")?
            .as_array()?
            .iter()
            .map(Json::as_u64)
            .collect::<Option<_>>()?;
        Some(Self { seqs })
    }
}

pub fn conflict_json(conflicts: &[(Vec<u8>, u64)]) -> String {
    json!({
        "conflicts": conflicts
            .iter()
            .map(|(key, seq)| json!({ "key": encode(key), "seq": seq }))
            .collect::<Vec<_>>(),
    })
    .to_string()
}

pub fn parse_conflicts(bytes: &[u8]) -> Vec<(Vec<u8>, u64)> {
    let Ok(body) = serde_json::from_slice::<Json>(bytes) else {
        return Vec::new();
    };
    body.get("conflicts")
        .and_then(Json::as_array)
        .map_or_else(Vec::new, |entries| {
            entries
                .iter()
                .filter_map(|e| Some((decode(e.get("key")?.as_str()?)?, e.get("seq")?.as_u64()?)))
                .collect()
        })
}

pub fn error_json(reason: &str) -> String {
    json!({ "error": reason }).to_string()
}

pub fn info_json() -> String {
    json!({
        "protocol": PROTOCOL,
        "max_record_bytes": MAX_BLOB_BYTES,
        "max_batch_bytes": MAX_BATCH_BYTES,
        "max_batch_records": MAX_BATCH_RECORDS,
        "max_space_records": MAX_SPACE_RECORDS,
        "max_space_bytes": MAX_SPACE_BYTES,
    })
    .to_string()
}
