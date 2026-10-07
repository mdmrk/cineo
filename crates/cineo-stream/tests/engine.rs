//! The engine against a local seeder: no internet, no DHT. The seeder is a
//! second `librqbit` session on loopback that already has the files.

// Test helpers panic on purpose: a panic is a failed assertion.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::{Ipv4Addr, SocketAddr};
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use cineo_core::app::{TorrentRequest, TorrentStatus};
use cineo_stream::{Engine, EngineOptions, StreamError};
use librqbit::spawn_utils::BlockingSpawner;
use librqbit::{
    AddTorrent, AddTorrentOptions, CreateTorrentOptions, ListenerOptions, Session, SessionOptions,
    create_torrent,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

fn temp_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("stream")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn film_bytes() -> Vec<u8> {
    (0..1_500_000u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect()
}

struct Seeder {
    _session: Arc<Session>,
    addr: SocketAddr,
    info_hash: String,
    film: Vec<u8>,
    film_index: usize,
}

async fn seeder(name: &str) -> Seeder {
    let content = temp_dir(&format!("{name}-seed")).join("content");
    std::fs::create_dir_all(&content).unwrap();
    let film = film_bytes();
    std::fs::write(content.join("film.mkv"), &film).unwrap();
    std::fs::write(content.join("readme.txt"), vec![b'x'; 5_000_000]).unwrap();
    let torrent = create_torrent(
        &content,
        CreateTorrentOptions {
            piece_length: Some(32 * 1024),
            ..CreateTorrentOptions::default()
        },
        &BlockingSpawner::new(1),
    )
    .await
    .unwrap();
    let session = Session::new_with_opts(
        content.clone(),
        SessionOptions {
            dht: None,
            persistence: None,
            listen: Some(ListenerOptions {
                listen_addr: (Ipv4Addr::LOCALHOST, 0).into(),
                ..ListenerOptions::default()
            }),
            disable_local_service_discovery: true,
            ..SessionOptions::default()
        },
    )
    .await
    .unwrap();
    let handle = session
        .add_torrent(
            AddTorrent::from_bytes(torrent.as_bytes().unwrap()),
            Some(AddTorrentOptions {
                output_folder: Some(content.to_str().unwrap().to_owned()),
                overwrite: true,
                ..AddTorrentOptions::default()
            }),
        )
        .await
        .unwrap()
        .into_handle()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(20), handle.wait_until_completed())
        .await
        .unwrap()
        .unwrap();
    let film_index = handle
        .with_metadata(|m| {
            m.file_infos
                .iter()
                .position(|f| f.relative_filename.ends_with("film.mkv"))
        })
        .unwrap()
        .unwrap();
    Seeder {
        film_index,
        addr: session.listen_addr().unwrap(),
        _session: session,
        info_hash: torrent.info_hash().as_string(),
        film,
    }
}

fn options(name: &str, seeder: &Seeder) -> EngineOptions {
    EngineOptions {
        allow_private_network: true,
        dht: false,
        extra_peers: vec![seeder.addr],
        metadata_timeout: Duration::from_secs(20),
        ..EngineOptions::new(temp_dir(&format!("{name}-cache")))
    }
}

fn request(seeder: &Seeder) -> TorrentRequest {
    TorrentRequest {
        info_hash: seeder.info_hash.clone(),
        file_idx: None,
        filename: None,
        trackers: Vec::new(),
    }
}

struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Reply {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

async fn http(addr: SocketAddr, method: &str, path: &str, extra: &[(&str, &str)]) -> Reply {
    let host = format!("{addr}");
    let mut request = format!("{method} {path} HTTP/1.1\r\nConnection: close\r\n");
    if !extra.iter().any(|(n, _)| n.eq_ignore_ascii_case("host")) {
        request.push_str(&format!("Host: {host}\r\n"));
    }
    for (name, value) in extra {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("\r\n");
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut raw = Vec::new();
    tokio::time::timeout(Duration::from_secs(30), stream.read_to_end(&mut raw))
        .await
        .unwrap()
        .unwrap();
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8(raw[..split].to_vec()).unwrap();
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .unwrap()
        .split(' ')
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers = lines
        .map(|l| {
            let (n, v) = l.split_once(':').unwrap();
            (n.trim().to_owned(), v.trim().to_owned())
        })
        .collect();
    Reply {
        status,
        headers,
        body: raw[split + 4..].to_vec(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn serves_the_chosen_file_with_ranges_from_a_local_peer() {
    let seeder = seeder("ranges").await;
    let engine = Engine::start(options("ranges", &seeder)).await.unwrap();
    assert!(engine.local_addr().ip().is_loopback());

    let url = engine.open(&request(&seeder)).await.unwrap();
    assert_eq!(url.host_str(), Some("127.0.0.1"));
    let path = url.path().to_owned();
    assert!(
        path.ends_with(&format!("/{}/{}", seeder.info_hash, seeder.film_index)),
        "{path}"
    );

    let addr = engine.local_addr();
    let full = http(addr, "GET", &path, &[]).await;
    assert_eq!(full.status, 200);
    assert_eq!(full.header("accept-ranges"), Some("bytes"));
    assert_eq!(full.header("content-type"), Some("video/x-matroska"));
    assert_eq!(full.header("access-control-allow-origin"), None);
    assert!(full.body == seeder.film, "the whole film arrives intact");

    let middle = http(addr, "GET", &path, &[("Range", "bytes=700000-700099")]).await;
    assert_eq!(middle.status, 206);
    assert_eq!(
        middle.header("content-range"),
        Some("bytes 700000-700099/1500000")
    );
    assert_eq!(middle.body, seeder.film[700_000..700_100]);

    let tail = http(addr, "GET", &path, &[("Range", "bytes=-10")]).await;
    assert_eq!(tail.body, seeder.film[seeder.film.len() - 10..]);

    let head = http(addr, "HEAD", &path, &[]).await;
    assert_eq!(head.status, 200);
    assert_eq!(head.header("content-length"), Some("1500000"));
    assert!(head.body.is_empty());

    let past = http(addr, "GET", &path, &[("Range", "bytes=1500000-")]).await;
    assert_eq!(past.status, 416);
    assert_eq!(past.header("content-range"), Some("bytes */1500000"));

    match engine.status() {
        Some((hash, TorrentStatus::Streaming { size, .. })) => {
            assert_eq!(hash, seeder.info_hash);
            assert_eq!(size, 1_500_000);
        }
        other => panic!("{other:?}"),
    }

    engine.stop().await;
    assert_eq!(http(addr, "GET", &path, &[]).await.status, 404);
    assert!(engine.status().is_none());
    engine.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn requests_without_the_token_or_from_a_foreign_host_are_refused() {
    let seeder = seeder("refuse").await;
    let engine = Engine::start(options("refuse", &seeder)).await.unwrap();
    let url = engine.open(&request(&seeder)).await.unwrap();
    let addr = engine.local_addr();
    let path = url.path().to_owned();
    let hash = &seeder.info_hash;

    let (film, other) = (seeder.film_index, 1 - seeder.film_index);
    let wrong_token = format!("/{}/{hash}/{film}", "0".repeat(32));
    assert_eq!(http(addr, "GET", &wrong_token, &[]).await.status, 404);
    let wrong_file = path.replace(&format!("/{hash}/{film}"), &format!("/{hash}/{other}"));
    assert_eq!(http(addr, "GET", &wrong_file, &[]).await.status, 404);
    assert_eq!(http(addr, "GET", "/", &[]).await.status, 404);
    assert_eq!(
        http(addr, "GET", &path, &[("Host", "evil.example")])
            .await
            .status,
        403,
        "DNS rebinding"
    );
    assert_eq!(http(addr, "POST", &path, &[]).await.status, 405);
    let localhost = format!("localhost:{}", addr.port());
    assert_eq!(
        http(addr, "HEAD", &path, &[("Host", &localhost)])
            .await
            .status,
        200
    );
    engine.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn loopback_peers_are_blocked_unless_private_networks_are_allowed() {
    let seeder = seeder("blocked").await;
    let engine = Engine::start(EngineOptions {
        allow_private_network: false,
        metadata_timeout: Duration::from_secs(3),
        ..options("blocked", &seeder)
    })
    .await
    .unwrap();
    let err = engine.open(&request(&seeder)).await.unwrap_err();
    assert!(
        matches!(err, StreamError::MetadataTimeout | StreamError::Add(_)),
        "{err:?}"
    );
    engine.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn downloaded_data_is_stored_by_hash_and_file_index_only() {
    let seeder = seeder("resume").await;
    let opts = options("resume", &seeder);
    let cache = opts.cache_dir.clone();
    let engine = Engine::start(opts.clone()).await.unwrap();
    let url = engine.open(&request(&seeder)).await.unwrap();
    let full = http(engine.local_addr(), "GET", url.path(), &[]).await;
    assert!(full.body == seeder.film);

    let stored: Vec<String> = std::fs::read_dir(cache.join(&seeder.info_hash))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(!stored.is_empty());
    assert!(
        stored.iter().all(|n| n.parse::<usize>().is_ok()),
        "{stored:?}"
    );
    engine.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn stopping_deletes_the_torrent_data() {
    let seeder = seeder("stop-deletes").await;
    let opts = options("stop-deletes", &seeder);
    let dir = opts.cache_dir.join(&seeder.info_hash);
    let engine = Engine::start(opts).await.unwrap();
    let url = engine.open(&request(&seeder)).await.unwrap();
    let full = http(engine.local_addr(), "GET", url.path(), &[]).await;
    assert!(full.body == seeder.film);
    assert!(dir.exists());

    engine.stop().await;
    assert!(!dir.exists(), "no torrent data is kept after it stops");
    engine.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn leftover_torrent_data_is_deleted_at_start() {
    let cache = temp_dir("leftover-cache");
    let leftover = cache.join("c".repeat(40));
    std::fs::create_dir_all(&leftover).unwrap();
    std::fs::write(leftover.join("0"), b"data").unwrap();
    let engine = Engine::start(EngineOptions {
        dht: false,
        ..EngineOptions::new(cache)
    })
    .await
    .unwrap();
    assert!(!leftover.exists());
    engine.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn shutdown_while_waiting_for_metadata_is_quick() {
    let options = EngineOptions {
        dht: false,
        metadata_timeout: Duration::from_secs(30),
        ..EngineOptions::new(temp_dir("quick-shutdown-cache"))
    };
    let engine = std::sync::Arc::new(Engine::start(options).await.unwrap());
    let waiting = {
        let engine = std::sync::Arc::clone(&engine);
        tokio::spawn(async move {
            let request = TorrentRequest {
                info_hash: "0123456789abcdef0123456789abcdef01234567".into(),
                file_idx: None,
                filename: None,
                trackers: Vec::new(),
            };
            let _ = engine.open(&request).await;
        })
    };
    tokio::task::yield_now().await;
    waiting.abort();
    let _ = waiting.await;
    let engine = std::sync::Arc::into_inner(engine).unwrap();

    let started = std::time::Instant::now();
    engine.shutdown().await;
    let took = started.elapsed();
    assert!(took < Duration::from_millis(500), "shutdown took {took:?}");
}

async fn seed_limited(
    content: &Path,
    torrent: &[u8],
    upload_bps: Option<u32>,
) -> (Arc<Session>, SocketAddr) {
    let session = Session::new_with_opts(
        content.to_path_buf(),
        SessionOptions {
            dht: None,
            persistence: None,
            listen: Some(ListenerOptions {
                listen_addr: (Ipv4Addr::LOCALHOST, 0).into(),
                ..ListenerOptions::default()
            }),
            disable_local_service_discovery: true,
            ratelimits: librqbit::limits::LimitsConfig {
                upload_bps: upload_bps.and_then(std::num::NonZeroU32::new),
                download_bps: None,
            },
            ..SessionOptions::default()
        },
    )
    .await
    .unwrap();
    let handle = session
        .add_torrent(
            AddTorrent::from_bytes(torrent.to_vec()),
            Some(AddTorrentOptions {
                output_folder: Some(content.to_str().unwrap().to_owned()),
                overwrite: true,
                ..AddTorrentOptions::default()
            }),
        )
        .await
        .unwrap()
        .into_handle()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(20), handle.wait_until_completed())
        .await
        .unwrap()
        .unwrap();
    let addr = session.listen_addr().unwrap();
    (session, addr)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_slow_peer_does_not_hold_back_the_start() {
    let content = temp_dir("slow-peer-seed").join("content");
    std::fs::create_dir_all(&content).unwrap();
    let film: Vec<u8> = (0..8_000_000u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    std::fs::write(content.join("film.mkv"), &film).unwrap();
    let torrent = create_torrent(
        &content,
        CreateTorrentOptions {
            piece_length: Some(512 * 1024),
            ..CreateTorrentOptions::default()
        },
        &BlockingSpawner::new(1),
    )
    .await
    .unwrap();
    let bytes = torrent.as_bytes().unwrap().to_vec();
    let (_slow_session, slow) = seed_limited(&content, &bytes, Some(128 * 1024)).await;
    let (_fast_session, fast) = seed_limited(&content, &bytes, None).await;

    let engine = Engine::start(EngineOptions {
        allow_private_network: true,
        dht: false,
        extra_peers: vec![slow, fast],
        metadata_timeout: Duration::from_secs(20),
        ..EngineOptions::new(temp_dir("slow-peer-cache"))
    })
    .await
    .unwrap();
    let started = std::time::Instant::now();
    let url = engine
        .open(&TorrentRequest {
            info_hash: torrent.info_hash().as_string(),
            file_idx: None,
            filename: None,
            trackers: Vec::new(),
        })
        .await
        .unwrap();
    let start = http(
        engine.local_addr(),
        "GET",
        url.path(),
        &[("Range", "bytes=0-2097151")],
    )
    .await;
    let took = started.elapsed();
    assert!(start.body == film[..2_097_152]);
    assert!(took < Duration::from_secs(2), "the start took {took:?}");
    engine.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_download_limit_slows_the_torrent_down() {
    let seeder = seeder("limit").await;
    let mut elapsed = Vec::new();
    for (name, limit) in [("limit-off", None), ("limit-on", NonZeroU32::new(400_000))] {
        let engine = Engine::start(EngineOptions {
            download_limit_bps: limit,
            ..options(name, &seeder)
        })
        .await
        .unwrap();
        assert_eq!(engine.options().download_limit_bps, limit);
        let url = engine.open(&request(&seeder)).await.unwrap();
        let start = std::time::Instant::now();
        let full = http(engine.local_addr(), "GET", url.path(), &[]).await;
        elapsed.push(start.elapsed());
        assert!(full.body == seeder.film);
        engine.shutdown().await;
    }
    let [off, on] = elapsed[..] else {
        unreachable!()
    };
    assert!(
        on >= Duration::from_millis(1500) && on > off * 2,
        "1.5 MB at 400 kB/s after a one-second burst: off {off:?}, on {on:?}"
    );
}
