use std::convert::Infallible;
use std::net::{Ipv4Addr, SocketAddr, TcpListener as StdListener};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use cineo_core::remote::{
    Command, MAX_REQUEST_BYTES, PATH, Refusal, Replays, Request as RemoteRequest, Status,
};
use http_body_util::{BodyExt as _, Full, Limited};
use hyper::body::Incoming;
use hyper::header::{CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, HeaderValue};
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::net::TcpListener;
use tokio::task::JoinSet;
use tracing::{debug, warn};

use super::{RemoteError, RemoteKeys};

pub const MAX_CONNECTIONS: usize = 16;
const HEADER_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(15);

pub type Handler = Arc<dyn Fn(Option<Command>) -> Status + Send + Sync>;

#[derive(Debug)]
pub struct RemoteServer {
    port: u16,
    task: tokio::task::AbortHandle,
}

impl RemoteServer {
    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for RemoteServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub fn bind(port: u16) -> std::io::Result<StdListener> {
    StdListener::bind((Ipv4Addr::UNSPECIFIED, port))
        .or_else(|_| StdListener::bind((Ipv4Addr::UNSPECIFIED, 0)))
}

struct Ctx {
    keys: RemoteKeys,
    replays: Mutex<Replays>,
    handler: Handler,
}

pub fn serve(
    listener: StdListener,
    key: &cineo_core::remote::RemoteKey,
    handler: Handler,
) -> Result<RemoteServer, RemoteError> {
    listener.set_nonblocking(true)?;
    let port = listener.local_addr()?.port();
    let listener = TcpListener::from_std(listener)?;
    let ctx = Arc::new(Ctx {
        keys: RemoteKeys::derive(key)?,
        replays: Mutex::new(Replays::default()),
        handler,
    });
    let task = tokio::spawn(accept(listener, ctx)).abort_handle();
    Ok(RemoteServer { port, task })
}

async fn accept(listener: TcpListener, ctx: Arc<Ctx>) {
    let mut connections = JoinSet::new();
    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(conn) => conn,
            Err(err) => {
                warn!(%err, "accepting a remote connection failed");
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        while connections.try_join_next().is_some() {}
        if connections.len() >= MAX_CONNECTIONS {
            debug!("too many remote connections; one was closed");
            continue;
        }
        let ctx = Arc::clone(&ctx);
        connections.spawn(async move {
            let service = hyper::service::service_fn(move |req| {
                let ctx = Arc::clone(&ctx);
                async move { Ok::<_, Infallible>(handle(&ctx, req, peer).await) }
            });
            let connection = hyper::server::conn::http1::Builder::new()
                .timer(TokioTimer::new())
                .header_read_timeout(HEADER_TIMEOUT)
                .keep_alive(false)
                .serve_connection(TokioIo::new(stream), service);
            match tokio::time::timeout(CONNECTION_TIMEOUT, connection).await {
                Ok(Err(err)) => debug!(%err, "remote connection ended"),
                Err(_) => debug!("remote connection timed out"),
                Ok(Ok(())) => {}
            }
        });
    }
}

fn answer(status: StatusCode, body: Vec<u8>) -> Response<Full<Bytes>> {
    let mut response = Response::new(Full::new(Bytes::from(body)));
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    response
}

fn refuse(status: StatusCode) -> Response<Full<Bytes>> {
    answer(status, Vec::new())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

async fn handle(ctx: &Ctx, req: Request<Incoming>, peer: SocketAddr) -> Response<Full<Bytes>> {
    if req.uri().path() != PATH {
        return refuse(StatusCode::NOT_FOUND);
    }
    if req.method() != Method::POST {
        return refuse(StatusCode::METHOD_NOT_ALLOWED);
    }
    let too_long = req
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok())
        .is_some_and(|len| len > MAX_REQUEST_BYTES);
    if too_long {
        return refuse(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let Ok(body) = Limited::new(req.into_body(), MAX_REQUEST_BYTES)
        .collect()
        .await
        .map(|b| b.to_bytes())
    else {
        return refuse(StatusCode::PAYLOAD_TOO_LARGE);
    };
    let Some((nonce, plain)) = ctx.keys.open_request(&body) else {
        debug!(%peer, "a remote request with the wrong key");
        return refuse(StatusCode::UNAUTHORIZED);
    };
    let Some(request) = RemoteRequest::parse(&plain) else {
        return refuse(StatusCode::BAD_REQUEST);
    };
    let admitted = ctx
        .replays
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .admit(now_ms(), request.time_ms, nonce);
    match admitted {
        Ok(()) => {}
        Err(Refusal::Clock | Refusal::Replayed) => return refuse(StatusCode::FORBIDDEN),
        Err(Refusal::Busy) => return refuse(StatusCode::SERVICE_UNAVAILABLE),
    }
    let status = (ctx.handler)(request.command);
    match ctx.keys.seal_answer(&nonce, status.to_json().as_bytes()) {
        Ok(sealed) => answer(StatusCode::OK, sealed),
        Err(_) => refuse(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
