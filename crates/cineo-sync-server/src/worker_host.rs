use cineo_core::sync::protocol::MAX_BATCH_BYTES;
use worker::js_sys::{Array, Math, Uint8Array};
use worker::wasm_bindgen::JsValue;
use worker::{
    D1Database, D1PreparedStatement, Date, Env, Headers, ScheduleContext, ScheduledEvent, event,
};

use crate::db::{Db, DbError, SqlValue, Statement};
use crate::handler::{self, Context, Gate, Method, Request, Response};

const DATABASE: &str = "DB";
const REQUESTS: &str = "REQUESTS";
const NEW_SPACES: &str = "NEW_SPACES";
const LIMIT_RETRY_S: u64 = 60;

async fn allowed(env: &Env, binding: &str, key: String) -> bool {
    let outcome = match env.rate_limiter(binding) {
        Ok(limiter) => limiter.limit(key).await,
        Err(err) => Err(err),
    };
    outcome.map_or_else(
        |err| {
            worker::console_warn!("rate limiter {binding} failed, allowing: {err}");
            true
        },
        |outcome| outcome.success,
    )
}

struct AddressGate<'a> {
    env: &'a Env,
    ip: String,
}

impl Gate for AddressGate<'_> {
    async fn may_create(&self) -> bool {
        allowed(self.env, NEW_SPACES, self.ip.clone()).await
    }
}

struct D1Db(D1Database);

fn db_error(err: &worker::Error) -> DbError {
    DbError(err.to_string())
}

impl D1Db {
    fn bound(&self, statement: &Statement) -> Result<D1PreparedStatement, DbError> {
        let values: Vec<JsValue> = statement
            .params
            .iter()
            .map(|value| match value {
                SqlValue::Null => JsValue::NULL,
                SqlValue::Int(n) => JsValue::from_f64(*n as f64),
                SqlValue::Blob(bytes) => Uint8Array::from(bytes.as_slice()).into(),
            })
            .collect();
        self.0
            .prepare(statement.sql.as_str())
            .bind(&values)
            .map_err(|err| db_error(&err))
    }
}

fn from_js(value: &JsValue) -> SqlValue {
    if let Some(n) = value.as_f64() {
        SqlValue::Int(n as i64)
    } else if Array::is_array(value) {
        SqlValue::Blob(Uint8Array::new(value).to_vec())
    } else if let Some(text) = value.as_string() {
        SqlValue::Blob(text.into_bytes())
    } else {
        SqlValue::Null
    }
}

impl Db for D1Db {
    async fn rows(&self, statement: Statement) -> Result<Vec<Vec<SqlValue>>, DbError> {
        let rows = self
            .bound(&statement)?
            .raw_js_value()
            .await
            .map_err(|err| db_error(&err))?;
        Ok(rows
            .iter()
            .map(|row| Array::from(row).iter().map(|v| from_js(&v)).collect())
            .collect())
    }

    async fn batch(&self, statements: Vec<Statement>) -> Result<Vec<u64>, DbError> {
        let prepared = statements
            .iter()
            .map(|s| self.bound(s))
            .collect::<Result<Vec<_>, _>>()?;
        let results = self.0.batch(prepared).await.map_err(|err| db_error(&err))?;
        results
            .iter()
            .map(|result| {
                let meta = result.meta().map_err(|err| db_error(&err))?;
                Ok(meta.and_then(|m| m.changes).unwrap_or(0) as u64)
            })
            .collect()
    }
}

fn random53() -> i64 {
    let n = (Math::random() * 9_007_199_254_740_992.0) as i64;
    if n == 0 { 1 } else { n }
}

fn to_worker(response: &Response) -> worker::Result<worker::Response> {
    let headers = Headers::new();
    if !response.body.is_empty() {
        headers.set("content-type", "application/json")?;
    }
    if let Some(after_s) = response.retry_after_s {
        headers.set("retry-after", &after_s.to_string())?;
    }
    Ok(worker::Response::builder()
        .with_status(response.status)
        .with_headers(headers)
        .fixed(response.body.clone().into_bytes()))
}

#[event(fetch)]
async fn fetch(
    mut req: worker::Request,
    env: Env,
    _ctx: worker::Context,
) -> worker::Result<worker::Response> {
    let too_large = Response::error(413, "request too large");
    let declared = req
        .headers()
        .get("content-length")?
        .and_then(|v| v.parse::<usize>().ok());
    if declared.is_some_and(|len| len > MAX_BATCH_BYTES) {
        return to_worker(&too_large);
    }
    let method = match req.method() {
        worker::Method::Get => Method::Get,
        worker::Method::Post => Method::Post,
        worker::Method::Delete => Method::Delete,
        _ => Method::Other,
    };
    let url = req.url()?;
    let bearer = req
        .headers()
        .get("authorization")?
        .and_then(|v| v.strip_prefix("Bearer ").map(str::to_owned));
    if let Some(id) = handler::space_id(bearer.as_deref()) {
        let key: String = id.iter().take(16).map(|b| format!("{b:02x}")).collect();
        if !allowed(&env, REQUESTS, key).await {
            return to_worker(&Response::retry(429, "too many requests", LIMIT_RETRY_S));
        }
    }
    let ip = req.headers().get("cf-connecting-ip")?.unwrap_or_default();
    let body = req.bytes().await?;
    if body.len() > MAX_BATCH_BYTES {
        return to_worker(&too_large);
    }
    let request = Request {
        method,
        path: url.path().to_owned(),
        query: url.query().map(str::to_owned),
        bearer,
        body,
    };
    let ctx = Context {
        now_ms: Date::now().as_millis(),
        nonce: random53(),
        born: random53(),
    };
    let db = D1Db(env.d1(DATABASE)?);
    let gate = AddressGate { env: &env, ip };
    let outcome = handler::handle(&db, &gate, &request, ctx).await;
    to_worker(&outcome.response)
}

#[event(scheduled)]
async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    let Ok(database) = env.d1(DATABASE) else {
        return;
    };
    if let Err(err) = handler::sweep(&D1Db(database), Date::now().as_millis()).await {
        worker::console_error!("sweeping idle sync spaces failed: {err}");
    }
}
