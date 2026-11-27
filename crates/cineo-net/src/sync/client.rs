use cineo_core::sync::protocol::{
    MAX_BATCH_BYTES, MAX_BATCH_RECORDS, MAX_BLOB_BYTES, MAX_PULL_BYTES, PULL_PAGE_RECORDS,
    PullPage, PushRequest, PushResponse, Write, parse_conflicts,
};
use cineo_core::sync::{Outgoing, Pairing, Record, RecordName, Replica, Stamp};
use reqwest::{Method, StatusCode};
use tracing::{Instrument, info_span, warn};
use url::{Host, Url};

use super::keys::SpaceKeys;
use crate::addon_client::{SyncRequest, next_request_id, origin};
use crate::{AddonClient, FetchError};

const MAX_PUSH_RESPONSE_BYTES: usize = 64 * 1024;
const REQUEST_OVERHEAD_BYTES: usize = 32;
const PUSH_ATTEMPTS: usize = 3;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SyncError {
    #[error("the sync server must use https")]
    InsecureServer,
    #[error(transparent)]
    Fetch(#[from] FetchError),
    #[error("another device changed the same records first")]
    Conflict(Vec<(RecordName, u64)>),
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
    pub space_seq: u64,
    pub born: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyncReport {
    pub pulled: usize,
    pub pushed: usize,
    pub unreadable: usize,
    pub oversized: Vec<RecordName>,
    pub started_over: bool,
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
        let body = self
            .call("pull", Method::GET, url, None, MAX_PULL_BYTES)
            .await?;
        let page = PullPage::parse(&body).ok_or(SyncError::Malformed)?;
        let mut pulled = Pulled {
            more: page.more,
            unreadable: page.skipped,
            space_seq: page.space_seq,
            born: page.born,
            ..Pulled::default()
        };
        for entry in page.records {
            match self.keys.open(&entry.key, &entry.blob) {
                Some(record) => pulled.records.push((record, entry.seq)),
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

    pub async fn push(&self, outgoing: &[Outgoing]) -> PushOutcome {
        let mut outcome = PushOutcome::default();
        let mut sealed = Vec::new();
        for out in outgoing {
            match self.keys.seal(&out.record) {
                Ok(s) if s.blob.len() > MAX_BLOB_BYTES => {
                    warn!(bytes = s.blob.len(), "a sync record is too large to push");
                    outcome.oversized.push(out.record.name.clone());
                }
                Ok(s) => sealed.push((
                    out,
                    Write {
                        key: s.key.to_vec(),
                        base_seq: out.base_seq,
                        blob: s.blob,
                    },
                )),
                Err(err) => {
                    outcome.error = Some(err);
                    return outcome;
                }
            }
        }
        for batch in batches(&sealed) {
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

    async fn push_batch(&self, batch: &[(&Outgoing, Write)]) -> Result<Vec<Pushed>, SyncError> {
        let request = PushRequest {
            writes: batch.iter().map(|(_, w)| w.clone()).collect(),
        };
        let url = self.url("v1/records")?;
        let (status, retry_after_s, body) = self
            .exchange(
                "push",
                Method::POST,
                url,
                Some(request.to_json().into_bytes()),
                MAX_PUSH_RESPONSE_BYTES,
            )
            .await?;
        if status == StatusCode::CONFLICT {
            let names: Vec<(RecordName, u64)> = parse_conflicts(&body)
                .into_iter()
                .filter_map(|(key, seq)| {
                    let (out, _) = batch.iter().find(|(_, w)| w.key == key)?;
                    Some((out.record.name.clone(), seq))
                })
                .collect();
            return Err(SyncError::Conflict(names));
        }
        let response = check(status, retry_after_s, body)?;
        let response = PushResponse::parse(&response).ok_or(SyncError::Malformed)?;
        if response.seqs.len() != batch.len() {
            return Err(SyncError::Malformed);
        }
        Ok(batch
            .iter()
            .zip(response.seqs)
            .map(|((out, _), seq)| Pushed {
                name: out.record.name.clone(),
                stamp: out.record.stamp,
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
            Ok(_) | Err(SyncError::SpaceGone) => Ok(()),
            Err(err) => Err(err),
        }
    }

    fn url(&self, path: &str) -> Result<Url, SyncError> {
        self.base.join(path).map_err(|_| SyncError::Malformed)
    }

    pub async fn sync(&self, replica: &mut Replica, now_ms: u64) -> Result<SyncReport, SyncError> {
        let mut report = SyncReport::default();
        if replica.verify_due(now_ms) {
            replica.verify(now_ms);
        }
        for attempt in 1..=PUSH_ATTEMPTS {
            self.pull_all(replica, now_ms, &mut report).await?;
            let outgoing = replica.outgoing();
            if outgoing.is_empty() {
                break;
            }
            let outcome = self.push(&outgoing).await;
            for pushed in &outcome.pushed {
                replica.pushed(&pushed.name, pushed.stamp, pushed.seq);
            }
            report.pushed += outcome.pushed.len();
            report.oversized = outcome.oversized;
            match outcome.error {
                None => break,
                Some(SyncError::Conflict(server)) if attempt < PUSH_ATTEMPTS => {
                    report.started_over |= replica.conflicted(&server);
                }
                Some(err) => return Err(err),
            }
        }
        replica.pulled(now_ms);
        Ok(report)
    }

    async fn pull_all(
        &self,
        replica: &mut Replica,
        now_ms: u64,
        report: &mut SyncReport,
    ) -> Result<(), SyncError> {
        let mut started_over = false;
        loop {
            let pulled = self.pull(replica.since()).await?;
            if replica.observe_space(pulled.born, pulled.space_seq) {
                if started_over {
                    return Err(SyncError::Malformed);
                }
                started_over = true;
                report.started_over = true;
                continue;
            }
            report.pulled += pulled.records.len();
            report.unreadable += pulled.unreadable;
            for (record, seq) in pulled.records {
                replica.merge(record, seq, now_ms);
            }
            if !pulled.more {
                return Ok(());
            }
        }
    }

    async fn exchange(
        &self,
        op: &'static str,
        method: Method,
        url: Url,
        body: Option<Vec<u8>>,
        limit: usize,
    ) -> Result<(StatusCode, Option<u64>, Vec<u8>), SyncError> {
        let span = info_span!("sync_request", req = next_request_id(), server = %origin(&url), op);
        Ok(self
            .http
            .send(SyncRequest {
                method,
                url,
                bearer: self.keys.token(),
                body,
                limit,
            })
            .instrument(span)
            .await?)
    }

    async fn call(
        &self,
        op: &'static str,
        method: Method,
        url: Url,
        body: Option<Vec<u8>>,
        limit: usize,
    ) -> Result<Vec<u8>, SyncError> {
        let (status, retry_after_s, body) = self.exchange(op, method, url, body, limit).await?;
        check(status, retry_after_s, body)
    }
}

fn check(
    status: StatusCode,
    retry_after_s: Option<u64>,
    body: Vec<u8>,
) -> Result<Vec<u8>, SyncError> {
    match status {
        status if status.is_success() => Ok(body),
        StatusCode::CONFLICT => Err(SyncError::Conflict(Vec::new())),
        StatusCode::GONE => Err(SyncError::SpaceGone),
        StatusCode::TOO_MANY_REQUESTS | StatusCode::SERVICE_UNAVAILABLE => {
            Err(SyncError::Unavailable { retry_after_s })
        }
        status => Err(SyncError::Rejected(status)),
    }
}

fn batches<'s, 'o>(sealed: &'s [(&'o Outgoing, Write)]) -> Vec<&'s [(&'o Outgoing, Write)]> {
    let mut batches = Vec::new();
    let mut start = 0;
    let mut bytes = REQUEST_OVERHEAD_BYTES;
    for (index, (_, write)) in sealed.iter().enumerate() {
        let size = write.encoded_len();
        if index > start && (index - start == MAX_BATCH_RECORDS || bytes + size > MAX_BATCH_BYTES) {
            batches.push(&sealed[start..index]);
            start = index;
            bytes = REQUEST_OVERHEAD_BYTES;
        }
        bytes += size;
    }
    if start < sealed.len() {
        batches.push(&sealed[start..]);
    }
    batches
}
