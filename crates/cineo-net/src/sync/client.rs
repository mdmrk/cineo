use cineo_core::sync::{Outgoing, Pairing, Record, RecordName, Stamp};
use data_encoding::BASE64URL_NOPAD;
use reqwest::{Method, StatusCode};
use serde_json::{Value, json};
use tracing::{Instrument, info_span, warn};
use url::{Host, Url};

use super::keys::{Sealed, SpaceKeys};
use crate::addon_client::{SyncRequest, next_request_id, origin};
use crate::{AddonClient, FetchError};

pub const MAX_BLOB_BYTES: usize = 32 * 1024;
pub const MAX_BATCH_BYTES: usize = 256 * 1024;
pub const MAX_BATCH_RECORDS: usize = 100;
pub const PULL_PAGE_RECORDS: usize = 500;
pub const MAX_PULL_BYTES: usize = 2 * 1024 * 1024;
const MAX_PUSH_RESPONSE_BYTES: usize = 64 * 1024;
const WRITE_OVERHEAD_BYTES: usize = 64;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SyncError {
    #[error("the sync server must use https")]
    InsecureServer,
    #[error(transparent)]
    Fetch(#[from] FetchError),
    #[error("another device changed the same records first")]
    Conflict,
    #[error("the sync space was deleted")]
    SpaceGone,
    #[error("the sync server is busy")]
    Unavailable { retry_after_s: Option<u64> },
    #[error("the sync server refused the request with HTTP {0}")]
    Rejected(StatusCode),
    #[error("the sync server sent an unreadable response")]
    Malformed,
    #[error("a cryptographic operation failed")]
    Crypto,
}

#[derive(Debug)]
pub struct SyncClient {
    http: AddonClient,
    base: Url,
    keys: SpaceKeys,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Pulled {
    pub records: Vec<(Record, u64)>,
    pub unreadable: usize,
    pub more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pushed {
    pub name: RecordName,
    pub stamp: Stamp,
    pub seq: u64,
}

#[derive(Debug, Default)]
pub struct PushOutcome {
    pub pushed: Vec<Pushed>,
    pub oversized: Vec<RecordName>,
    pub error: Option<SyncError>,
}

struct Write<'a> {
    out: &'a Outgoing,
    sealed: Sealed,
}

impl SyncClient {
    pub fn new(http: AddonClient, pairing: &Pairing) -> Result<Self, SyncError> {
        let server = &pairing.server;
        let loopback = match server.host() {
            Some(Host::Ipv4(ip)) => ip.is_loopback(),
            Some(Host::Ipv6(ip)) => ip.is_loopback(),
            Some(Host::Domain(domain)) => domain == "localhost",
            None => false,
        };
        if !(server.scheme() == "https" || server.scheme() == "http" && loopback) {
            return Err(SyncError::InsecureServer);
        }
        let mut base = server.clone();
        if !base.path().ends_with('/') {
            let path = format!("{}/", base.path());
            base.set_path(&path);
        }
        base.set_query(None);
        base.set_fragment(None);
        Ok(Self {
            http,
            base,
            keys: SpaceKeys::derive(&pairing.key)?,
        })
    }

    pub async fn pull(&self, since: u64) -> Result<Pulled, SyncError> {
        let mut url = self.url("v1/records")?;
        url.query_pairs_mut()
            .append_pair("since", &since.to_string())
            .append_pair("limit", &PULL_PAGE_RECORDS.to_string());
        let body = match self
            .call("pull", Method::GET, url, None, MAX_PULL_BYTES)
            .await
        {
            Err(SyncError::Rejected(StatusCode::NOT_FOUND)) => return Ok(Pulled::default()),
            other => other?,
        };
        let page: Value = serde_json::from_slice(&body).map_err(|_| SyncError::Malformed)?;
        let entries = page
            .get("records")
            .and_then(Value::as_array)
            .ok_or(SyncError::Malformed)?;
        let mut pulled = Pulled {
            more: page.get("more").and_then(Value::as_bool).unwrap_or(false),
            ..Pulled::default()
        };
        for entry in entries {
            match self.open_entry(entry) {
                Some(opened) => pulled.records.push(opened),
                None => pulled.unreadable += 1,
            }
        }
        if pulled.unreadable > 0 {
            warn!(
                count = pulled.unreadable,
                "skipping unreadable sync records"
            );
        }
        Ok(pulled)
    }

    fn open_entry(&self, entry: &Value) -> Option<(Record, u64)> {
        let decode = |key| {
            BASE64URL_NOPAD
                .decode(entry.get(key)?.as_str()?.as_bytes())
                .ok()
        };
        let seq = entry.get("seq")?.as_u64()?;
        let record = self.keys.open(&decode("key")?, &decode("blob")?)?;
        Some((record, seq))
    }

    pub async fn push(&self, outgoing: &[Outgoing]) -> PushOutcome {
        let mut outcome = PushOutcome::default();
        let mut writes = Vec::new();
        for out in outgoing {
            match self.keys.seal(&out.record) {
                Ok(sealed) if sealed.blob.len() > MAX_BLOB_BYTES => {
                    warn!(
                        bytes = sealed.blob.len(),
                        "a sync record is too large to push"
                    );
                    outcome.oversized.push(out.record.name.clone());
                }
                Ok(sealed) => writes.push(Write { out, sealed }),
                Err(err) => {
                    outcome.error = Some(err);
                    return outcome;
                }
            }
        }
        for batch in batches(&writes) {
            match self.push_batch(batch).await {
                Ok(pushed) => outcome.pushed.extend(pushed),
                Err(err) => {
                    outcome.error = Some(err);
                    break;
                }
            }
        }
        outcome
    }

    async fn push_batch(&self, batch: &[Write<'_>]) -> Result<Vec<Pushed>, SyncError> {
        let body = json!({
            "writes": batch.iter().map(|w| json!({
                "key": BASE64URL_NOPAD.encode(&w.sealed.key),
                "base_seq": w.out.base_seq,
                "blob": BASE64URL_NOPAD.encode(&w.sealed.blob),
            })).collect::<Vec<_>>(),
        });
        let url = self.url("v1/records")?;
        let response = self
            .call(
                "push",
                Method::POST,
                url,
                Some(body.to_string().into_bytes()),
                MAX_PUSH_RESPONSE_BYTES,
            )
            .await?;
        let response: Value =
            serde_json::from_slice(&response).map_err(|_| SyncError::Malformed)?;
        let seqs: Vec<u64> = response
            .get("seqs")
            .and_then(Value::as_array)
            .ok_or(SyncError::Malformed)?
            .iter()
            .map(Value::as_u64)
            .collect::<Option<_>>()
            .ok_or(SyncError::Malformed)?;
        if seqs.len() != batch.len() {
            return Err(SyncError::Malformed);
        }
        Ok(batch
            .iter()
            .zip(seqs)
            .map(|(w, seq)| Pushed {
                name: w.out.record.name.clone(),
                stamp: w.out.record.stamp,
                seq,
            })
            .collect())
    }

    pub async fn delete_space(&self) -> Result<(), SyncError> {
        let url = self.url("v1/space")?;
        match self
            .call("delete", Method::DELETE, url, None, MAX_PUSH_RESPONSE_BYTES)
            .await
        {
            Ok(_) | Err(SyncError::SpaceGone | SyncError::Rejected(StatusCode::NOT_FOUND)) => {
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    fn url(&self, path: &str) -> Result<Url, SyncError> {
        self.base.join(path).map_err(|_| SyncError::Malformed)
    }

    async fn call(
        &self,
        op: &'static str,
        method: Method,
        url: Url,
        body: Option<Vec<u8>>,
        limit: usize,
    ) -> Result<Vec<u8>, SyncError> {
        let span = info_span!("sync_request", req = next_request_id(), server = %origin(&url), op);
        let (status, retry_after_s, body) = self
            .http
            .send(SyncRequest {
                method,
                url,
                bearer: self.keys.token(),
                body,
                limit,
            })
            .instrument(span)
            .await?;
        match status {
            status if status.is_success() => Ok(body),
            StatusCode::CONFLICT => Err(SyncError::Conflict),
            StatusCode::GONE => Err(SyncError::SpaceGone),
            StatusCode::TOO_MANY_REQUESTS | StatusCode::SERVICE_UNAVAILABLE => {
                Err(SyncError::Unavailable { retry_after_s })
            }
            status => Err(SyncError::Rejected(status)),
        }
    }
}

fn batches<'w, 'a>(writes: &'w [Write<'a>]) -> Vec<&'w [Write<'a>]> {
    let mut batches = Vec::new();
    let mut start = 0;
    let mut bytes = 0;
    for (index, write) in writes.iter().enumerate() {
        let size =
            (write.sealed.blob.len() + write.sealed.key.len()) * 4 / 3 + WRITE_OVERHEAD_BYTES;
        if index > start && (index - start == MAX_BATCH_RECORDS || bytes + size > MAX_BATCH_BYTES) {
            batches.push(&writes[start..index]);
            start = index;
            bytes = 0;
        }
        bytes += size;
    }
    if start < writes.len() {
        batches.push(&writes[start..]);
    }
    batches
}
