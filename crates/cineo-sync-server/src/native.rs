use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use cineo_core::sync::protocol::{MAX_BATCH_BYTES, error_json};
use http_body_util::{BodyExt as _, Full, Limited};
use hyper::body::Incoming;
use hyper::header::{AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, HeaderValue, RETRY_AFTER};
use hyper_util::rt::TokioIo;
use rusqlite::types::{ToSqlOutput, ValueRef};
use rusqlite::{Connection, ToSql, params_from_iter};
use tokio::net::TcpListener;
use tracing::{debug, warn};

use crate::db::{Db, DbError, SqlValue, Statement};
use crate::handler::{self, Context, Method, Request, Response};
use crate::limit::RateLimiter;

const SWEEP_EVERY: Duration = Duration::from_secs(3600);

#[derive(Debug)]
pub struct SqliteDb {
    conn: Mutex<Connection>,
}

impl SqliteDb {
    pub fn open(path: &Path) -> Result<Self, DbError> {
        let conn = Connection::open(path).map_err(|err| db_error(&err))?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|err| db_error(&err))?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self, DbError> {
        Self::init(Connection::open_in_memory().map_err(|err| db_error(&err))?)
    }

    fn init(conn: Connection) -> Result<Self, DbError> {
        conn.pragma_update(None, "synchronous", "NORMAL")
            .map_err(|err| db_error(&err))?;
        conn.execute_batch(crate::SCHEMA)
            .map_err(|err| db_error(&err))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn db_error(err: &rusqlite::Error) -> DbError {
    DbError(err.to_string())
}

impl ToSql for SqlValue {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(match self {
            Self::Null => ToSqlOutput::Borrowed(ValueRef::Null),
            Self::Int(value) => ToSqlOutput::Borrowed(ValueRef::Integer(*value)),
            Self::Blob(bytes) => ToSqlOutput::Borrowed(ValueRef::Blob(bytes)),
        })
    }
}

impl Db for SqliteDb {
    async fn rows(&self, statement: Statement) -> Result<Vec<Vec<SqlValue>>, DbError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&statement.sql).map_err(|err| db_error(&err))?;
        let columns = stmt.column_count();
        let rows = stmt
            .query_map(params_from_iter(&statement.params), |row| {
                (0..columns)
                    .map(|index| {
                        Ok(match row.get_ref(index)? {
                            ValueRef::Integer(value) => SqlValue::Int(value),
                            ValueRef::Blob(bytes) | ValueRef::Text(bytes) => {
                                SqlValue::Blob(bytes.to_vec())
                            }
                            ValueRef::Null | ValueRef::Real(_) => SqlValue::Null,
                        })
                    })
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(|err| db_error(&err))?;
        rows.collect::<rusqlite::Result<_>>()
            .map_err(|err| db_error(&err))
    }

    async fn batch(&self, statements: Vec<Statement>) -> Result<Vec<u64>, DbError> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(|err| db_error(&err))?;
        let mut changes = Vec::with_capacity(statements.len());
        for statement in &statements {
            let changed = tx
                .prepare(&statement.sql)
                .and_then(|mut stmt| stmt.execute(params_from_iter(&statement.params)))
                .map_err(|err| db_error(&err))?;
            changes.push(changed as u64);
        }
        tx.commit().map_err(|err| db_error(&err))?;
        Ok(changes)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ServeOptions {
    pub trust_forwarded_for: bool,
    pub clock: fn() -> u64,
}

impl Default for ServeOptions {
    fn default() -> Self {
        Self {
            trust_forwarded_for: false,
            clock: system_ms,
        }
    }
}

struct Shared {
    db: Arc<SqliteDb>,
    limiter: Mutex<RateLimiter>,
    options: ServeOptions,
}

pub async fn serve(listener: TcpListener, db: Arc<SqliteDb>, options: ServeOptions) {
    let sweeper = Arc::clone(&db);
    tokio::spawn(async move {
        let mut every = tokio::time::interval(SWEEP_EVERY);
        loop {
            every.tick().await;
            if let Err(err) = handler::sweep(&*sweeper, (options.clock)()).await {
                warn!(%err, "sweeping idle sync spaces failed");
            }
        }
    });
    let shared = Arc::new(Shared {
        db,
        limiter: Mutex::new(RateLimiter::default()),
        options,
    });
    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(conn) => conn,
            Err(err) => {
                warn!(%err, "accepting a sync connection failed");
                continue;
            }
        };
        let shared = Arc::clone(&shared);
        tokio::spawn(async move {
            let service = hyper::service::service_fn(move |req| {
                let shared = Arc::clone(&shared);
                async move { Ok::<_, Infallible>(respond(&shared, req, peer).await) }
            });
            if let Err(err) = hyper::server::conn::http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .await
            {
                debug!(%err, "sync connection ended");
            }
        });
    }
}

fn random53() -> i64 {
    match getrandom::u64().map(|n| n >> 11) {
        Ok(0) | Err(_) => 1,
        Ok(n) => i64::try_from(n).unwrap_or(1),
    }
}

pub fn system_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn client_ip(req: &hyper::Request<Incoming>, peer: SocketAddr, trust: bool) -> IpAddr {
    let forwarded = trust
        .then(|| req.headers().get("x-forwarded-for")?.to_str().ok())
        .flatten()
        .and_then(|list| list.rsplit(',').next())
        .and_then(|last| last.trim().parse().ok());
    forwarded.unwrap_or_else(|| peer.ip())
}

async fn respond(
    shared: &Shared,
    req: hyper::Request<Incoming>,
    peer: SocketAddr,
) -> hyper::Response<Full<Bytes>> {
    let now = (shared.options.clock)();
    let ip = client_ip(&req, peer, shared.options.trust_forwarded_for);
    let limited = shared
        .limiter
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .request(ip, now);
    if let Err(after_s) = limited {
        return to_http(&Response {
            status: 429,
            body: error_json("too many requests"),
            retry_after_s: Some(after_s),
        });
    }
    let too_large = Response {
        status: 413,
        body: error_json("request too large"),
        retry_after_s: None,
    };
    let declared = req
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok());
    if declared.is_some_and(|len| len > MAX_BATCH_BYTES) {
        return to_http(&too_large);
    }
    let method = match *req.method() {
        hyper::Method::GET => Method::Get,
        hyper::Method::POST => Method::Post,
        hyper::Method::DELETE => Method::Delete,
        _ => Method::Other,
    };
    let path = req.uri().path().to_owned();
    let query = req.uri().query().map(str::to_owned);
    let bearer = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::to_owned);
    let Ok(body) = Limited::new(req.into_body(), MAX_BATCH_BYTES)
        .collect()
        .await
    else {
        return to_http(&too_large);
    };
    let request = Request {
        method,
        path,
        query,
        bearer,
        body: body.to_bytes().to_vec(),
    };
    let ctx = Context {
        now_ms: now,
        nonce: random53(),
        born: random53(),
        may_create: shared
            .limiter
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .may_create(ip, now),
    };
    let outcome = handler::handle(&*shared.db, &request, ctx).await;
    if outcome.created {
        shared
            .limiter
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .created(ip, now);
    }
    to_http(&outcome.response)
}

fn to_http(response: &Response) -> hyper::Response<Full<Bytes>> {
    let mut http = hyper::Response::new(Full::new(Bytes::from(response.body.clone())));
    *http.status_mut() =
        hyper::StatusCode::from_u16(response.status).unwrap_or(hyper::StatusCode::OK);
    if !response.body.is_empty() {
        http.headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    }
    if let Some(after_s) = response.retry_after_s {
        http.headers_mut()
            .insert(RETRY_AFTER, HeaderValue::from(after_s));
    }
    http
}
