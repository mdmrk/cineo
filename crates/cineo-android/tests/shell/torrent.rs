use super::*;

struct Seeder {
    _session: Arc<Session>,
    addr: SocketAddr,
    info_hash: String,
    film: Vec<u8>,
}

fn first_bytes(url: &url::Url, count: usize) -> Vec<u8> {
    use std::io::{Read, Write};
    let addr = (url.host_str().unwrap(), url.port().unwrap());
    let mut stream = std::net::TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .unwrap();
    write!(
        stream,
        "GET {} HTTP/1.1\r\nHost: {}:{}\r\nRange: bytes=0-{}\r\nConnection: close\r\n\r\n",
        &url[url::Position::BeforePath..],
        addr.0,
        addr.1,
        count - 1
    )
    .unwrap();
    let mut reply = Vec::new();
    stream.read_to_end(&mut reply).unwrap();
    let body = reply.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
    assert!(
        reply.starts_with(b"HTTP/1.1 206"),
        "{:?}",
        String::from_utf8_lossy(&reply[..body])
    );
    reply[body..].to_vec()
}

async fn seeder(dir: &Path) -> Seeder {
    let content = dir.join("seed");
    std::fs::create_dir_all(&content).unwrap();
    let film: Vec<u8> = (0..1_500_000u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    std::fs::write(content.join("film.mkv"), &film).unwrap();
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
    Seeder {
        addr: session.listen_addr().unwrap(),
        _session: session,
        info_hash: torrent.info_hash().as_string(),
        film,
    }
}

async fn torrent_addon(server: &MockServer, info_hash: &str) -> String {
    serve_movie(server).await;
    let streams =
        format!(r#"{{"streams":[{{"name":"Torrent","infoHash":"{info_hash}","fileIdx":0}}]}}"#);
    serve(
        server,
        "/cfg/stream/movie/tt0000001.json",
        streams.into_bytes(),
    )
    .await;
    format!("{}/cfg/manifest.json", server.uri())
}

#[tokio::test(flavor = "multi_thread")]
async fn a_torrent_stream_asks_once_then_plays_through_the_engine() {
    let dir = local_data_dir();
    let seeder = seeder(dir.path()).await;
    let server = MockServer::start().await;
    let manifest = torrent_addon(&server, &seeder.info_hash).await;

    tokio::task::block_in_place(|| {
        let players = Arc::new(FakePlayers::default());
        let (cineo, views) = start_seeded(dir.path(), &players, vec![seeder.addr]);
        open_movie(&cineo, &views, manifest);
        cineo.play(0, 0);
        wait_for(&views, |v| v.p2p_prompt);
        assert!(players.requests.lock().unwrap().is_empty());
        cineo.accept_p2p();
        let playing = wait_for(&views, |v| v.playback.is_some());
        assert!(!playing.p2p_prompt && playing.connecting.is_none());
        let request = players.requests.lock().unwrap()[0].clone();
        assert_eq!(request.url.host_str(), Some("127.0.0.1"));
        assert_eq!(first_bytes(&request.url, 4096), seeder.film[..4096]);
        let streaming = wait_for(
            &views,
            |v| matches!(v.torrent, Some(TorrentView::Streaming { downloaded, .. }) if downloaded > 0),
        );
        assert!(streaming.playback.is_some());
        players
            .events
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .send(PlayerEvent::Closed {
                time_ms: 1_000,
                duration_ms: 120_000,
            })
            .unwrap();
        wait_for(&views, |v| v.playback.is_none() && v.torrent.is_none());

        cineo.play(0, 0);
        wait_for(&views, |v| v.playback.is_some());
        assert_eq!(players.requests.lock().unwrap().len(), 2, "asked only once");
        players
            .events
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .send(PlayerEvent::Closed {
                time_ms: 2_000,
                duration_ms: 120_000,
            })
            .unwrap();
        wait_for(&views, |v| v.playback.is_none() && v.torrent.is_none());

        cineo.resume("tt0000001".into());
        wait_for(&views, |v| v.playback.is_some());
        let resumed = players.requests.lock().unwrap()[2].clone();
        assert_eq!(resumed.start_ms, 2_000);
        assert_eq!(first_bytes(&resumed.url, 4096), seeder.film[..4096]);
        drop(cineo);
    });

    let settings = Store::open_in(dir.path()).unwrap().settings().unwrap();
    assert!(settings.p2p_acknowledged);
}

#[tokio::test(flavor = "multi_thread")]
async fn going_back_while_a_torrent_connects_stops_it() {
    let dir = local_data_dir();
    let server = MockServer::start().await;
    let manifest = torrent_addon(&server, &"ab".repeat(20)).await;
    let mut store = Store::open_in(dir.path()).unwrap();
    let mut settings = store.settings().unwrap();
    settings.p2p_acknowledged = true;
    store.save_settings(&settings).unwrap();
    drop(store);

    tokio::task::block_in_place(|| {
        let players = Arc::new(FakePlayers::default());
        let (cineo, views) = start_with(dir.path(), &players);
        open_movie(&cineo, &views, manifest);
        cineo.play(0, 0);
        let connecting = wait_for(&views, |v| v.connecting.is_some());
        let art = connecting.connecting.clone().unwrap();
        assert_eq!(art.title, "First Example Film");
        assert_eq!(
            art.logo.as_deref(),
            Some("https://img.example/logo/tt0000001.png")
        );
        assert_eq!(
            art.background.as_deref(),
            Some("https://img.example/background/tt0000001.jpg")
        );
        assert_eq!(connecting.torrent, Some(TorrentView::Starting));
        cineo.stop_playback();
        let stopped = wait_for(&views, |v| v.connecting.is_none());
        assert_eq!(stopped.torrent, None);
        assert!(stopped.detail.is_some());
        assert!(players.requests.lock().unwrap().is_empty());
        drop(cineo);
    });
}
