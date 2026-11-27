use std::collections::BTreeMap;

use cineo_core::sync::protocol::{
    MAX_PULL_BYTES, MAX_SPACE_BYTES, MAX_SPACE_RECORDS, PULL_PAGE_RECORDS, PullPage, PushRequest,
    PushResponse, WireRecord, conflict_json, decode_token, error_json, info_json,
};
use sha2::{Digest, Sha256};
use tracing::warn;

use crate::db::{Db, DbError, SqlValue, Statement};

pub const IDLE_MS: u64 = 365 * 24 * 60 * 60 * 1000;
pub const BUSY_RETRY_S: u64 = 60;
pub const CREATE_RETRY_S: u64 = 3600;
pub const SEQS_PER_MS: u64 = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Delete,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: Method,
    pub path: String,
    pub query: Option<String>,
    pub bearer: Option<String>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub body: String,
    pub retry_after_s: Option<u64>,
}

impl Response {
    fn json(status: u16, body: String) -> Self {
        Self {
            status,
            body,
            retry_after_s: None,
        }
    }

    fn error(status: u16, reason: &str) -> Self {
        Self::json(status, error_json(reason))
    }

    fn retry(status: u16, reason: &str, after_s: u64) -> Self {
        Self {
            retry_after_s: Some(after_s),
            ..Self::error(status, reason)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Context {
    pub now_ms: u64,
    pub nonce: i64,
    pub born: i64,
    pub may_create: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub response: Response,
    pub created: bool,
}

impl From<Response> for Outcome {
    fn from(response: Response) -> Self {
        Self {
            response,
            created: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Route {
    Info,
    Records,
    Space,
}

#[derive(Debug, Clone, Copy)]
struct Space {
    seq: u64,
    records: u64,
    bytes: u64,
    deleted: bool,
    born: u64,
}

const SPACE: &str = "SELECT seq, records, bytes, deleted, born FROM spaces WHERE id = ?1";
const PULL: &str = "SELECT key, seq, blob FROM records
     WHERE space = ?1 AND seq > ?2 ORDER BY seq LIMIT ?3";
const CLAIM: &str = "INSERT INTO spaces (id, seq, records, bytes, touched_ms, deleted, lock, born)
     VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?8)
     ON CONFLICT (id) DO UPDATE SET
         seq = excluded.seq,
         records = excluded.records,
         bytes = excluded.bytes,
         touched_ms = excluded.touched_ms,
         lock = excluded.lock
     WHERE spaces.seq = ?7 AND spaces.deleted = 0";
const WRITE: &str = "INSERT INTO records (space, key, seq, blob)
     SELECT ?1, ?2, ?3, ?4 WHERE EXISTS (SELECT 1 FROM spaces WHERE id = ?1 AND lock = ?5)
     ON CONFLICT (space, key) DO UPDATE SET seq = excluded.seq, blob = excluded.blob";
const DELETE_RECORDS: &str = "DELETE FROM records WHERE space = ?1";
const MARK_DELETED: &str =
    "INSERT INTO spaces (id, seq, records, bytes, touched_ms, deleted, lock, born)
     VALUES (?1, 0, 0, 0, ?2, 1, 0, 0)
     ON CONFLICT (id) DO UPDATE SET
         records = 0, bytes = 0, touched_ms = excluded.touched_ms, deleted = 1";
const SWEEP_RECORDS: &str =
    "DELETE FROM records WHERE space IN (SELECT id FROM spaces WHERE touched_ms < ?1)";
const SWEEP_SPACES: &str = "DELETE FROM spaces WHERE touched_ms < ?1";

pub(crate) async fn handle(db: &impl Db, request: &Request, ctx: Context) -> Outcome {
    let route = match request.path.rsplit_once("/v1/").map(|(_, tail)| tail) {
        Some("info") => Route::Info,
        Some("records") => Route::Records,
        Some("space") => Route::Space,
        _ => return Response::error(404, "not found").into(),
    };
    let result = match (route, request.method) {
        (Route::Info, Method::Get) => return Response::json(200, info_json()).into(),
        (Route::Records, Method::Get | Method::Post) | (Route::Space, Method::Delete) => {
            let Some(id) = space_id(request.bearer.as_deref()) else {
                return Response::error(401, "missing or invalid token").into();
            };
            match request.method {
                Method::Get => pull(db, &id, request.query.as_deref())
                    .await
                    .map(Outcome::from),
                Method::Post => push(db, &id, &request.body, ctx).await,
                _ => delete(db, &id, ctx.now_ms).await.map(Outcome::from),
            }
        }
        _ => return Response::error(405, "method not allowed").into(),
    };
    result.unwrap_or_else(|err| {
        warn!(%err, "sync storage failed");
        Response::retry(503, "storage unavailable", BUSY_RETRY_S).into()
    })
}

fn space_id(bearer: Option<&str>) -> Option<Vec<u8>> {
    let token = decode_token(bearer?)?;
    Some(Sha256::digest(token).to_vec())
}

async fn space(db: &impl Db, id: &[u8]) -> Result<Option<Space>, DbError> {
    let rows = db
        .rows(Statement::new(SPACE, vec![SqlValue::Blob(id.to_vec())]))
        .await?;
    Ok(rows.into_iter().next().and_then(|row| {
        Some(Space {
            seq: row.first()?.int()?,
            records: row.get(1)?.int()?,
            bytes: row.get(2)?.int()?,
            deleted: row.get(3)?.int()? != 0,
            born: row.get(4).and_then(|v| match v {
                SqlValue::Int(n) => Some(u64::from_le_bytes(n.to_le_bytes())),
                _ => None,
            })?,
        })
    }))
}

fn query_u64(query: Option<&str>, name: &str) -> Option<u64> {
    query?
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(key, _)| *key == name)
        .and_then(|(_, value)| value.parse().ok())
}

async fn pull(db: &impl Db, id: &[u8], query: Option<&str>) -> Result<Response, DbError> {
    let since = query_u64(query, "since").unwrap_or(0);
    let limit = query_u64(query, "limit")
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(PULL_PAGE_RECORDS)
        .clamp(1, PULL_PAGE_RECORDS);
    let space = match space(db, id).await? {
        None => return Ok(Response::json(200, PullPage::default().to_json())),
        Some(space) if space.deleted => return Ok(Response::error(410, "space deleted")),
        Some(space) => space,
    };
    let rows = db
        .rows(Statement::new(
            PULL,
            vec![
                SqlValue::Blob(id.to_vec()),
                SqlValue::from_u64(since),
                SqlValue::from_u64(limit as u64 + 1),
            ],
        ))
        .await?;
    let mut page = PullPage {
        space_seq: space.seq,
        born: space.born,
        ..PullPage::default()
    };
    let mut bytes = 96;
    for (index, row) in rows.into_iter().enumerate() {
        let mut row = row.into_iter();
        let (Some(key), Some(seq), Some(blob)) = (
            row.next().and_then(SqlValue::blob),
            row.next().as_ref().and_then(SqlValue::int),
            row.next().and_then(SqlValue::blob),
        ) else {
            return Err(DbError("unreadable record row".into()));
        };
        let record = WireRecord { key, seq, blob };
        if index == limit
            || !page.records.is_empty() && bytes + record.encoded_len() > MAX_PULL_BYTES
        {
            page.more = true;
            break;
        }
        bytes += record.encoded_len();
        page.records.push(record);
    }
    Ok(Response::json(200, page.to_json()))
}

async fn push(db: &impl Db, id: &[u8], body: &[u8], ctx: Context) -> Result<Outcome, DbError> {
    let request = match PushRequest::parse(body) {
        Ok(request) => request,
        Err(err) => return Ok(Response::error(400, &err.to_string()).into()),
    };
    if request.writes.is_empty() {
        return Ok(Response::json(200, PushResponse { seqs: Vec::new() }.to_json()).into());
    }
    let existing = space(db, id).await?;
    match existing {
        Some(space) if space.deleted => return Ok(Response::error(410, "space deleted").into()),
        None if !ctx.may_create => {
            return Ok(Response::retry(429, "too many new spaces", CREATE_RETRY_S).into());
        }
        _ => {}
    }
    let current = current_versions(db, id, &request).await?;
    let conflicts: Vec<(Vec<u8>, u64)> = request
        .writes
        .iter()
        .map(|w| (w, current.get(&w.key).map_or(0, |(seq, _)| *seq)))
        .filter(|(w, seq)| *seq != w.base_seq)
        .map(|(w, seq)| (w.key.clone(), seq))
        .collect();
    if !conflicts.is_empty() {
        return Ok(Response::json(409, conflict_json(&conflicts)).into());
    }
    let base = existing.unwrap_or(Space {
        seq: 0,
        records: 0,
        bytes: 0,
        deleted: false,
        born: 0,
    });
    let added = request
        .writes
        .iter()
        .filter(|w| !current.contains_key(&w.key))
        .count() as u64;
    let records = base.records + added;
    let bytes = request.writes.iter().fold(base.bytes, |bytes, w| {
        let old = current.get(&w.key).map_or(0, |(_, len)| *len);
        (bytes + w.blob.len() as u64).saturating_sub(old)
    });
    if records > MAX_SPACE_RECORDS || bytes > MAX_SPACE_BYTES {
        return Ok(Response::error(413, "space full").into());
    }
    let first = base.seq.max(ctx.now_ms.saturating_mul(SEQS_PER_MS));
    let seqs: Vec<u64> = (1..=request.writes.len() as u64)
        .map(|n| first + n)
        .collect();
    let last = seqs.last().copied().unwrap_or(base.seq);
    let mut statements = vec![Statement::new(
        CLAIM,
        vec![
            SqlValue::Blob(id.to_vec()),
            SqlValue::from_u64(last),
            SqlValue::from_u64(records),
            SqlValue::from_u64(bytes),
            SqlValue::from_u64(ctx.now_ms),
            SqlValue::Int(ctx.nonce),
            SqlValue::from_u64(base.seq),
            SqlValue::Int(ctx.born),
        ],
    )];
    for (write, seq) in request.writes.iter().zip(&seqs) {
        statements.push(Statement::new(
            WRITE,
            vec![
                SqlValue::Blob(id.to_vec()),
                SqlValue::Blob(write.key.clone()),
                SqlValue::from_u64(*seq),
                SqlValue::Blob(write.blob.clone()),
                SqlValue::Int(ctx.nonce),
            ],
        ));
    }
    let changes = db.batch(statements).await?;
    if changes.first().copied() != Some(1) {
        return Ok(Response::json(409, conflict_json(&[])).into());
    }
    Ok(Outcome {
        response: Response::json(200, PushResponse { seqs }.to_json()),
        created: existing.is_none(),
    })
}

async fn current_versions(
    db: &impl Db,
    id: &[u8],
    request: &PushRequest,
) -> Result<BTreeMap<Vec<u8>, (u64, u64)>, DbError> {
    let placeholders: Vec<String> = (2..request.writes.len() + 2)
        .map(|n| format!("?{n}"))
        .collect();
    let sql = format!(
        "SELECT key, seq, length(blob) FROM records WHERE space = ?1 AND key IN ({})",
        placeholders.join(", ")
    );
    let mut params = vec![SqlValue::Blob(id.to_vec())];
    params.extend(request.writes.iter().map(|w| SqlValue::Blob(w.key.clone())));
    let mut current = BTreeMap::new();
    for row in db.rows(Statement::new(sql, params)).await? {
        let mut row = row.into_iter();
        let (Some(key), Some(seq), Some(len)) = (
            row.next().and_then(SqlValue::blob),
            row.next().as_ref().and_then(SqlValue::int),
            row.next().as_ref().and_then(SqlValue::int),
        ) else {
            return Err(DbError("unreadable version row".into()));
        };
        current.insert(key, (seq, len));
    }
    Ok(current)
}

async fn delete(db: &impl Db, id: &[u8], now_ms: u64) -> Result<Response, DbError> {
    db.batch(vec![
        Statement::new(DELETE_RECORDS, vec![SqlValue::Blob(id.to_vec())]),
        Statement::new(
            MARK_DELETED,
            vec![SqlValue::Blob(id.to_vec()), SqlValue::from_u64(now_ms)],
        ),
    ])
    .await?;
    Ok(Response::json(204, String::new()))
}

pub(crate) async fn sweep(db: &impl Db, now_ms: u64) -> Result<(), DbError> {
    let before = SqlValue::from_u64(now_ms.saturating_sub(IDLE_MS));
    db.batch(vec![
        Statement::new(SWEEP_RECORDS, vec![before.clone()]),
        Statement::new(SWEEP_SPACES, vec![before]),
    ])
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests;
