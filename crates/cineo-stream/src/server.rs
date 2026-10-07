//! The loopback HTTP server that hands torrent files to the player
//! (SECURITY.md §Streaming engine): `127.0.0.1` only, a random per-session
//! path token, a `Host` check against DNS rebinding, single byte ranges, no
//! CORS headers.

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use bytes::Bytes;
use futures_util::TryStreamExt as _;
use http_body_util::{BodyExt as _, Full, StreamBody, combinators::BoxBody};
use hyper::body::{Frame, Incoming};
use hyper::header::{
    ACCEPT_RANGES, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, HOST, HeaderValue,
    RANGE,
};
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use librqbit::ManagedTorrent;
use tokio::io::{AsyncReadExt as _, AsyncSeekExt as _};
use tokio::net::TcpListener;
use tokio_util::io::ReaderStream;
use tracing::{debug, warn};

use crate::range::{Span, span};

type Body = BoxBody<Bytes, std::io::Error>;

pub(crate) struct Served {
    pub(crate) info_hash: String,
    pub(crate) file: usize,
    pub(crate) len: u64,
    pub(crate) name: String,
    pub(crate) torrent: Arc<ManagedTorrent>,
}

pub(crate) type Current = Arc<RwLock<Option<Arc<Served>>>>;

pub(crate) struct Ctx {
    pub(crate) token: String,
    pub(crate) addr: SocketAddr,
    pub(crate) current: Current,
}

pub(crate) async fn run(listener: TcpListener, ctx: Arc<Ctx>) {
    loop {
        let (stream, _) = match listener.accept().await {
            Ok(conn) => conn,
            Err(err) => {
                warn!(%err, "accepting a player connection failed");
                continue;
            }
        };
        let ctx = Arc::clone(&ctx);
        tokio::spawn(async move {
            let service = hyper::service::service_fn(move |req| {
                let ctx = Arc::clone(&ctx);
                async move { Ok::<_, Infallible>(handle(&ctx, req).await) }
            });
            if let Err(err) = hyper::server::conn::http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .await
            {
                debug!(%err, "player connection ended");
            }
        });
    }
}

fn empty(status: StatusCode) -> Response<Body> {
    let mut response = Response::new(Full::new(Bytes::new()).map_err(|n| match n {}).boxed());
    *response.status_mut() = status;
    response
}

fn host_ok(req: &Request<Incoming>, addr: SocketAddr) -> bool {
    let Some(host) = req.headers().get(HOST).and_then(|h| h.to_str().ok()) else {
        return false;
    };
    let port = addr.port();
    host == format!("{}:{port}", addr.ip()) || host == format!("localhost:{port}")
}

fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0, |acc, (x, y)| acc | (x ^ y))
            == 0
}

fn content_type(name: &str) -> &'static str {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("mkv") => "video/x-matroska",
        Some("mp4" | "m4v") => "video/mp4",
        Some("webm") => "video/webm",
        Some("avi") => "video/x-msvideo",
        Some("mov") => "video/quicktime",
        Some("ts" | "m2ts") => "video/mp2t",
        Some("mpg" | "mpeg") => "video/mpeg",
        Some("ogv") => "video/ogg",
        _ => "application/octet-stream",
    }
}

async fn handle(ctx: &Ctx, req: Request<Incoming>) -> Response<Body> {
    if !matches!(*req.method(), Method::GET | Method::HEAD) {
        return empty(StatusCode::METHOD_NOT_ALLOWED);
    }
    if !host_ok(&req, ctx.addr) {
        return empty(StatusCode::FORBIDDEN);
    }
    let mut parts = req.uri().path().trim_start_matches('/').split('/');
    let (Some(token), Some(hash), Some(file), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return empty(StatusCode::NOT_FOUND);
    };
    if !same(token, &ctx.token) {
        return empty(StatusCode::NOT_FOUND);
    }
    let served = ctx.current.read().ok().and_then(|c| c.clone());
    let Some(served) =
        served.filter(|s| s.info_hash == hash && file.parse::<usize>().ok() == Some(s.file))
    else {
        return empty(StatusCode::NOT_FOUND);
    };

    let range = req.headers().get(RANGE).and_then(|v| v.to_str().ok());
    let (status, start, end) = match span(range, served.len) {
        Span::Full if served.len == 0 => (StatusCode::OK, 0, 0),
        Span::Full => (StatusCode::OK, 0, served.len - 1),
        Span::Partial { start, end } => (StatusCode::PARTIAL_CONTENT, start, end),
        Span::Unsatisfiable => {
            let mut response = empty(StatusCode::RANGE_NOT_SATISFIABLE);
            if let Ok(value) = HeaderValue::from_str(&format!("bytes */{}", served.len)) {
                response.headers_mut().insert(CONTENT_RANGE, value);
            }
            return response;
        }
    };
    let length = if served.len == 0 { 0 } else { end - start + 1 };

    let body = if req.method() == Method::HEAD || length == 0 {
        Full::new(Bytes::new()).map_err(|n| match n {}).boxed()
    } else {
        match Arc::clone(&served.torrent).stream(served.file).await {
            Ok(mut stream) => {
                if let Err(err) = stream.seek(std::io::SeekFrom::Start(start)).await {
                    warn!(%err, "seeking in the torrent failed");
                    return empty(StatusCode::INTERNAL_SERVER_ERROR);
                }
                let reader = stream.take(length);
                StreamBody::new(ReaderStream::new(reader).map_ok(Frame::data)).boxed()
            }
            Err(err) => {
                warn!(%err, "opening the torrent stream failed");
                return empty(StatusCode::SERVICE_UNAVAILABLE);
            }
        }
    };
    let mut response = Response::new(body);
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static(content_type(&served.name)),
    );
    headers.insert(CONTENT_LENGTH, HeaderValue::from(length));
    if status == StatusCode::PARTIAL_CONTENT
        && let Ok(value) = HeaderValue::from_str(&format!("bytes {start}-{end}/{}", served.len))
    {
        headers.insert(CONTENT_RANGE, value);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_comparison_needs_equal_strings() {
        assert!(same("abc", "abc"));
        assert!(!same("abc", "abd"));
        assert!(!same("abc", "ab"));
        assert!(!same("", "a"));
    }

    #[test]
    fn content_types_follow_the_extension() {
        assert_eq!(content_type("Film.MKV"), "video/x-matroska");
        assert_eq!(content_type("a/b.mp4"), "video/mp4");
        assert_eq!(content_type("readme"), "application/octet-stream");
    }
}
