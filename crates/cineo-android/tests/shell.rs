#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers panic on purpose"
)]

use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cineo_android::{
    AddonListItems, AddonSubtitleView, AppView, Cineo, DecoderView, ImageError, InstallView,
    MetaStatus, Playback, PlayerStatsView, Players, ProblemView, RowItems, SettingChange, Started,
    StateListener, StreamItems, TorrentView, TrackView, VideoSizeView, version,
};
use cineo_core::app::{PlayRequest, Resolution, StreamSort, SubtitleFont, Tier};
use cineo_core::app::{Settings, TorrentStorage};
use cineo_player::embedded::{PlayerCommand, Stats, Status, Track, TrackKind};
use cineo_player::{PlayerError, PlayerEvent};
use cineo_store::Store;
use cineo_stream::EngineOptions;
use librqbit::spawn_utils::BlockingSpawner;
use librqbit::{
    AddTorrent, AddTorrentOptions, CreateTorrentOptions, ListenerOptions, Session, SessionOptions,
    create_torrent,
};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[derive(Debug)]
struct Views(Mutex<Sender<AppView>>);

impl StateListener for Views {
    fn changed(&self, view: AppView) {
        let _ = self.0.lock().unwrap().send(view);
    }
}

fn start(dir: &Path) -> (Arc<Cineo>, Receiver<AppView>) {
    let (tx, rx) = channel();
    let cineo = Cineo::new(
        dir.to_str().unwrap(),
        dir.join("cache").to_str().unwrap(),
        Arc::new(Views(Mutex::new(tx))),
    )
    .unwrap();
    (cineo, rx)
}

fn local_engine(dir: &Path, peers: Vec<SocketAddr>) -> EngineOptions {
    EngineOptions {
        dht: false,
        extra_peers: peers,
        metadata_timeout: Duration::from_secs(20),
        trackers: Vec::new(),
        ..EngineOptions::new(dir.join("torrents"))
    }
}

#[derive(Debug, Default)]
struct FakePlayers {
    requests: Mutex<Vec<PlayRequest>>,
    events: Mutex<Option<UnboundedSender<PlayerEvent>>>,
    calls: Arc<Mutex<Vec<String>>>,
    slow_release: Mutex<Option<SlowRelease>>,
}

#[derive(Debug)]
struct SlowRelease {
    releasing: Sender<()>,
    release: Receiver<()>,
}

#[derive(Debug)]
struct FakePlayback(Arc<Mutex<Vec<String>>>, Mutex<Option<SlowRelease>>);

impl Drop for FakePlayback {
    fn drop(&mut self) {
        if let Some(slow) = self.1.get_mut().unwrap().take() {
            let _ = slow.releasing.send(());
            let _ = slow.release.recv_timeout(Duration::from_secs(10));
        }
    }
}

impl Playback for FakePlayback {
    fn attach_surface(&self, wid: i64) -> Result<(), PlayerError> {
        self.0.lock().unwrap().push(format!("attach {wid}"));
        Ok(())
    }

    fn detach_surface(&self) -> Result<(), PlayerError> {
        self.0.lock().unwrap().push("detach".into());
        Ok(())
    }

    fn resize_surface(&self, width: u32, height: u32) -> Result<(), PlayerError> {
        self.0
            .lock()
            .unwrap()
            .push(format!("resize {width}x{height}"));
        Ok(())
    }

    fn send(&self, command: PlayerCommand) {
        self.0.lock().unwrap().push(format!("{command:?}"));
    }

    fn add_subtitle(&self, path: &Path, title: &str, lang: &str) -> Result<(), PlayerError> {
        let content = std::fs::read_to_string(path).unwrap();
        self.0
            .lock()
            .unwrap()
            .push(format!("subtitle {title} {lang} {content}"));
        Ok(())
    }

    fn status(&self) -> Status {
        Status {
            loaded: true,
            position_s: 12.0,
            duration_s: 120.0,
            tracks: Arc::from([
                Track {
                    id: 1,
                    kind: TrackKind::Video,
                    title: None,
                    lang: None,
                    selected: true,
                    external_file: None,
                    codec: Some("h264\nHigh".into()),
                },
                track(1, TrackKind::Audio, Some("eng"), Some("Stereo"), true),
                track(2, TrackKind::Audio, None, None, false),
                track(1, TrackKind::Subtitle, Some("eng"), None, false),
            ]),
            stats: Stats {
                cache_speed: Some(2_048.0),
                cache_s: Some(31.4),
                width: Some(1920.0),
                height: Some(1080.0),
                video_bitrate: Some(4_500_000.0),
                hwdec: Some("mediacodec".into()),
                dropped_frames: Some(3.0),
            },
            ..Status::default()
        }
    }
}

fn track(
    id: i64,
    kind: TrackKind,
    lang: Option<&str>,
    title: Option<&str>,
    selected: bool,
) -> Track {
    Track {
        id,
        kind,
        title: title.map(str::to_owned),
        lang: lang.map(str::to_owned),
        selected,
        external_file: None,
        codec: None,
    }
}

impl Players for FakePlayers {
    fn start(&self, request: &PlayRequest) -> Result<Started, PlayerError> {
        self.requests.lock().unwrap().push(request.clone());
        let (tx, rx) = unbounded_channel();
        *self.events.lock().unwrap() = Some(tx);
        let slow = self.slow_release.lock().unwrap().take();
        Ok((
            Box::new(FakePlayback(Arc::clone(&self.calls), Mutex::new(slow))),
            rx,
        ))
    }
}

fn start_with(dir: &Path, players: &Arc<FakePlayers>) -> (Arc<Cineo>, Receiver<AppView>) {
    start_seeded(dir, players, Vec::new())
}

fn start_seeded(
    dir: &Path,
    players: &Arc<FakePlayers>,
    peers: Vec<SocketAddr>,
) -> (Arc<Cineo>, Receiver<AppView>) {
    let (tx, views) = channel();
    let cineo = Cineo::start(
        dir.to_str().unwrap(),
        dir.join("subtitles"),
        local_engine(dir, peers),
        Arc::new(Views(Mutex::new(tx))),
        Arc::clone(players) as Arc<dyn Players>,
    )
    .unwrap();
    (cineo, views)
}

async fn movie_addon(server: &MockServer) -> String {
    serve(
        server,
        "/cfg/manifest.json",
        fixture("addons/basic/manifest.json"),
    )
    .await;
    serve(
        server,
        "/cfg/meta/movie/tt0000001.json",
        fixture("addons/basic/meta-movie.json"),
    )
    .await;
    serve(
        server,
        "/cfg/stream/movie/tt0000001.json",
        fixture("addons/basic/streams-movie.json"),
    )
    .await;
    format!("{}/cfg/manifest.json", server.uri())
}

fn streams_ready(view: &AppView) -> bool {
    view.detail.as_ref().is_some_and(|d| {
        d.meta == MetaStatus::Ready
            && !d.streams.is_empty()
            && d.streams
                .iter()
                .all(|g| !matches!(g.items, StreamItems::Loading))
    })
}

fn wait_for(views: &Receiver<AppView>, done: impl Fn(&AppView) -> bool) -> AppView {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut last = None;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match views.recv_timeout(left) {
            Ok(view) if done(&view) => return view,
            Ok(view) => last = Some(view),
            Err(_) => break,
        }
    }
    panic!("the view never got there; last: {last:#?}");
}

fn fixture(rel: &str) -> Vec<u8> {
    let full = format!("{}/../../tests/{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&full).unwrap_or_else(|err| panic!("reading {full}: {err}"))
}

async fn serve(server: &MockServer, at: &str, body: Vec<u8>) {
    Mock::given(method("GET"))
        .and(path(at))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(server)
        .await;
}

fn local_data_dir() -> tempdir::Dir {
    let dir = tempdir::Dir::new();
    let mut store = Store::open_in(dir.path()).unwrap();
    store.save_addons(&[]).unwrap();
    store
        .save_settings(&Settings {
            allow_private_network: true,
            check_for_updates: false,
            ..Settings::default()
        })
        .unwrap();
    dir
}

#[tokio::test(flavor = "multi_thread")]
async fn an_installed_addon_fills_the_board_and_is_back_after_a_restart() {
    let server = MockServer::start().await;
    serve(
        &server,
        "/cfg/manifest.json",
        fixture("addons/basic/manifest.json"),
    )
    .await;
    serve(
        &server,
        "/cfg/catalog/movie/top.json",
        fixture("addons/basic/catalog-movie-top.json"),
    )
    .await;
    let manifest = format!("{}/cfg/manifest.json", server.uri());
    let dir = local_data_dir();

    tokio::task::block_in_place(|| {
        let (cineo, views) = start(dir.path());
        let empty = wait_for(&views, |_| true);
        assert!(empty.addons.is_empty() && empty.board.is_empty());

        cineo.install_addon(manifest.clone());
        let board = wait_for(&views, |v| {
            !v.board.is_empty() && v.board.iter().all(|r| r.items != RowItems::Loading)
        });
        assert_eq!(
            board.install,
            Some(InstallView::Installed {
                name: "Basic Fixture".into()
            })
        );
        assert_eq!(board.addons.len(), 1);
        assert_eq!(board.addons[0].transport, manifest);
        assert_eq!(board.addons[0].version, "1.2.0");
        let names: Vec<_> = board.board.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["Top Movies", "Multi-genre"]);
        let RowItems::Ready { items } = &board.board[0].items else {
            panic!("the movie row did not load: {:?}", board.board[0].items);
        };
        assert_eq!(items[0].id, "tt0000001");
        assert_eq!(items[0].name, "First Example Film");
        assert!(matches!(
            board.board[1].items,
            RowItems::Failed {
                problem: ProblemView::Request { .. }
            }
        ));
        drop(cineo);

        let (_cineo, views) = start(dir.path());
        let restored = wait_for(&views, |v| v.addons.len() == 1 && v.addons_loading == 0);
        assert_eq!(restored.addons[0].name, "Basic Fixture");
        assert_eq!(restored.install, None);
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_install_of_the_same_addon_is_refused() {
    let server = MockServer::start().await;
    serve(
        &server,
        "/cfg/manifest.json",
        fixture("addons/basic/manifest.json"),
    )
    .await;
    let manifest = format!("{}/cfg/manifest.json", server.uri());
    let dir = local_data_dir();

    tokio::task::block_in_place(|| {
        let (cineo, views) = start(dir.path());
        cineo.install_addon(manifest.clone());
        wait_for(&views, |v| v.addons.len() == 1);
        cineo.install_addon(manifest);
        wait_for(&views, |v| {
            v.install
                == Some(InstallView::Failed {
                    problem: ProblemView::AlreadyInstalled,
                })
        });
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn images_come_through_the_addon_client_and_its_limits() {
    let server = MockServer::start().await;
    let png = vec![0x89, b'P', b'N', b'G', 1, 2, 3];
    Mock::given(method("GET"))
        .and(path("/poster.png"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(png.clone(), "image/png"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/huge.png"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(vec![0; 5 * 1024 * 1024], "image/png"),
        )
        .mount(&server)
        .await;
    let dir = local_data_dir();
    let (cineo, _views) = tokio::task::block_in_place(|| start(dir.path()));

    let bytes = cineo
        .fetch_image(format!("{}/poster.png", server.uri()))
        .await
        .unwrap();
    assert_eq!(bytes, png);
    let huge = cineo
        .fetch_image(format!("{}/huge.png", server.uri()))
        .await;
    assert!(matches!(huge, Err(ImageError::Fetch { .. })), "{huge:?}");
    let bad = cineo.fetch_image("not a url".into()).await;
    assert!(matches!(bad, Err(ImageError::InvalidUrl { .. })), "{bad:?}");
    tokio::task::block_in_place(|| drop(cineo));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stream_plays_on_the_surface_and_its_progress_reaches_the_library() {
    let server = MockServer::start().await;
    serve(
        &server,
        "/cfg/manifest.json",
        fixture("addons/basic/manifest.json"),
    )
    .await;
    serve(
        &server,
        "/cfg/meta/movie/tt0000001.json",
        fixture("addons/basic/meta-movie.json"),
    )
    .await;
    serve(
        &server,
        "/cfg/stream/movie/tt0000001.json",
        fixture("addons/basic/streams-movie.json"),
    )
    .await;
    let manifest = format!("{}/cfg/manifest.json", server.uri());
    let dir = local_data_dir();
    let players = Arc::new(FakePlayers::default());

    tokio::task::block_in_place(|| {
        let (cineo, views) = start_with(dir.path(), &players);
        cineo.install_addon(manifest);
        wait_for(&views, |v| v.addons.len() == 1);
        cineo.attach_surface(7);
        cineo.open_detail("movie".into(), "tt0000001".into());
        let detail = wait_for(&views, |v| {
            v.detail.as_ref().is_some_and(|d| {
                d.meta == MetaStatus::Ready
                    && d.streams
                        .iter()
                        .all(|g| !matches!(g.items, StreamItems::Loading))
            })
        });
        let detail = detail.detail.unwrap();
        assert_eq!(detail.name.as_deref(), Some("First Example Film"));
        assert_eq!(detail.runtime.as_deref(), Some("120"));
        let StreamItems::Ready { streams, .. } = &detail.streams[0].items else {
            panic!("streams did not load: {:?}", detail.streams);
        };
        assert_eq!(streams[0].name.as_deref(), Some("1080p"));

        cineo.play(0, 0);
        let playing = wait_for(&views, |v| v.playback.is_some());
        let art = playing.playback.unwrap().art;
        assert_eq!(art.title, "First Example Film");
        assert_eq!(
            art.background.as_deref(),
            Some("https://img.example/background/tt0000001.jpg")
        );
        let request = players.requests.lock().unwrap()[0].clone();
        assert_eq!(request.url.as_str(), "https://media.example/v/1.mp4");
        assert!(
            request
                .headers
                .iter()
                .any(|(k, v)| k == "User-Agent" && v == "Cineo-Test")
        );
        assert_eq!(players.calls.lock().unwrap().as_slice(), ["attach 7"]);
        let status = cineo.player_status().unwrap();
        assert!(status.loaded);
        assert_eq!(status.position_s, 12.0);
        assert_eq!(
            status.stats,
            PlayerStatsView {
                network_bytes_per_sec: Some(2_048.0),
                buffered_s: Some(31.4),
                resolution: Some(VideoSizeView {
                    width: 1920,
                    height: 1080
                }),
                video_codec: Some("h264 High".into()),
                video_bits_per_sec: Some(4_500_000.0),
                decoder: Some(DecoderView::Hardware {
                    name: "mediacodec".into()
                }),
                dropped_frames: Some(3),
            }
        );

        assert_eq!(
            status.audio,
            [
                TrackView {
                    id: 1,
                    number: 1,
                    label: Some("eng · Stereo".into()),
                    selected: true
                },
                TrackView {
                    id: 2,
                    number: 2,
                    label: None,
                    selected: false
                },
            ]
        );
        assert_eq!(
            status.subtitles,
            [TrackView {
                id: 1,
                number: 1,
                label: Some("eng".into()),
                selected: false
            }]
        );
        assert!(status.subtitles_off);
        cineo.select_audio(Some(2));
        cineo.select_subtitle(None);

        cineo.resize_surface(1080, 2340);
        cineo.detach_surface();
        cineo.toggle_pause();
        cineo.stop_playback();
        assert_eq!(
            players.calls.lock().unwrap().as_slice(),
            [
                "attach 7",
                "SetAudio(Some(2))",
                "SetSubtitle(None)",
                "resize 1080x2340",
                "detach",
                "TogglePause",
                "Stop"
            ]
        );
        let events = players.events.lock().unwrap().take().unwrap();
        events
            .send(PlayerEvent::Progress {
                time_ms: 60_000,
                duration_ms: 120_000,
            })
            .unwrap();
        events
            .send(PlayerEvent::Closed {
                time_ms: 61_000,
                duration_ms: 120_000,
            })
            .unwrap();
        wait_for(&views, |v| v.playback.is_none());
        assert!(cineo.player_status().is_none());
        drop(cineo);
    });

    let library = Store::open_in(dir.path()).unwrap().library().unwrap();
    let item = library.iter().find(|i| i.id == "tt0000001").unwrap();
    assert_eq!(item.time_offset_ms, 61_000);
    assert_eq!(item.duration_ms, 120_000);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_screen_answers_while_a_finished_player_is_released() {
    let server = MockServer::start().await;
    let manifest = movie_addon(&server).await;
    let dir = local_data_dir();
    let players = Arc::new(FakePlayers::default());
    let (releasing_tx, releasing) = channel();
    let (release, release_rx) = channel();
    *players.slow_release.lock().unwrap() = Some(SlowRelease {
        releasing: releasing_tx,
        release: release_rx,
    });

    tokio::task::block_in_place(|| {
        let (cineo, views) = start_with(dir.path(), &players);
        cineo.install_addon(manifest);
        wait_for(&views, |v| v.addons.len() == 1);
        cineo.open_detail("movie".into(), "tt0000001".into());
        wait_for(&views, streams_ready);
        cineo.play(0, 0);
        wait_for(&views, |v| v.playback.is_some());

        let events = players.events.lock().unwrap().take().unwrap();
        events
            .send(PlayerEvent::Closed {
                time_ms: 1_000,
                duration_ms: 120_000,
            })
            .unwrap();
        releasing.recv_timeout(Duration::from_secs(10)).unwrap();
        let (answer, answered) = channel();
        let screen = Arc::clone(&cineo);
        std::thread::spawn(move || {
            screen.detach_surface();
            let _ = answer.send(screen.player_status().is_none());
        });
        let status = answered.recv_timeout(Duration::from_secs(2));
        release.send(()).unwrap();
        assert_eq!(status, Ok(true), "the screen waited for the player release");
        wait_for(&views, |v| v.playback.is_none());
        drop(cineo);
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn addon_subtitles_load_by_the_preferred_language_and_when_picked() {
    let server = MockServer::start().await;
    let manifest = movie_addon(&server).await;
    serve(
        &server,
        "/subs/manifest.json",
        fixture("addons/basic/manifest-subtitles.json"),
    )
    .await;
    let subtitles = format!(
        r#"{{"subtitles":[{{"id":"1","url":"{0}/files/spa","lang":"spa"}},{{"id":"2","url":"{0}/files/fre","lang":"fre","label":"French"}}]}}"#,
        server.uri()
    );
    Mock::given(method("GET"))
        .and(path_regex(r"^/subs/subtitles/movie/tt0000001/.*\.json$"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(subtitles, "application/json"))
        .mount(&server)
        .await;
    for (at, body) in [("/files/spa", "hola"), ("/files/fre", "salut")] {
        Mock::given(method("GET"))
            .and(path(at))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
    }
    let dir = local_data_dir();
    let players = Arc::new(FakePlayers::default());

    tokio::task::block_in_place(|| {
        let (cineo, views) = start_with(dir.path(), &players);
        cineo.change_setting(SettingChange::SubtitleLanguage {
            code: Some("spa".into()),
        });
        cineo.install_addon(manifest);
        wait_for(&views, |v| v.addons.len() == 1);
        cineo.install_addon(format!("{}/subs/manifest.json", server.uri()));
        wait_for(&views, |v| v.addons.len() == 2);
        cineo.open_detail("movie".into(), "tt0000001".into());
        wait_for(&views, streams_ready);
        cineo.play(0, 0);
        wait_for(&views, |v| v.playback.is_some());

        let calls = || players.calls.lock().unwrap().clone();
        let until = |done: &dyn Fn(&[String]) -> bool| {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                let status = cineo.player_status().unwrap();
                if done(&calls()) {
                    return status;
                }
                assert!(Instant::now() < deadline, "calls: {:?}", calls());
                std::thread::sleep(Duration::from_millis(20));
            }
        };
        let status = until(&|c| c.iter().any(|c| c.starts_with("subtitle ")));
        assert_eq!(
            calls()
                .iter()
                .filter(|c| c.starts_with("subtitle "))
                .collect::<Vec<_>>(),
            ["subtitle Subtitles Fixture spa hola"],
            "the preferred language loads once"
        );
        let fre = format!("{}/files/fre", server.uri());
        assert!(status.addon_subtitles.contains(&AddonSubtitleView {
            url: fre.clone(),
            label: "fre · French".into(),
            selected: false,
        }));

        cineo.select_addon_subtitle(&fre);
        until(&|c| c.iter().any(|c| c == "subtitle French fre salut"));
        cineo.select_addon_subtitle(&format!("{}/files/spa", server.uri()));
        until(&|c| {
            c.iter()
                .filter(|c| c.starts_with("subtitle Subtitles Fixture spa"))
                .count()
                == 2
        });
        drop(cineo);
    });

    let requests = server.received_requests().await.unwrap();
    let fetched = |at: &str| requests.iter().filter(|r| r.url.path() == at).count();
    assert_eq!(
        fetched("/files/spa"),
        1,
        "a loaded subtitle is not fetched again"
    );
    assert_eq!(fetched("/files/fre"), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_played_stream_resumes_where_it_stopped_after_a_restart() {
    let server = MockServer::start().await;
    let manifest = movie_addon(&server).await;
    let dir = local_data_dir();

    tokio::task::block_in_place(|| {
        let players = Arc::new(FakePlayers::default());
        let (cineo, views) = start_with(dir.path(), &players);
        cineo.install_addon(manifest);
        wait_for(&views, |v| v.addons.len() == 1);
        cineo.open_detail("movie".into(), "tt0000001".into());
        wait_for(&views, streams_ready);
        cineo.play(0, 0);
        wait_for(&views, |v| v.playback.is_some());
        let events = players.events.lock().unwrap().take().unwrap();
        events
            .send(PlayerEvent::Progress {
                time_ms: 60_000,
                duration_ms: 120_000,
            })
            .unwrap();
        events
            .send(PlayerEvent::Closed {
                time_ms: 61_000,
                duration_ms: 120_000,
            })
            .unwrap();
        let stopped = wait_for(&views, |v| {
            v.playback.is_none() && !v.continue_watching.is_empty()
        });
        let item = &stopped.continue_watching[0];
        assert_eq!(item.id, "tt0000001");
        assert!(item.resumable && item.played && !item.favorite);
        assert!((item.progress - 61.0 / 120.0).abs() < 1e-6);
        assert_eq!(stopped.recent, stopped.continue_watching);
        drop(cineo);

        let players = Arc::new(FakePlayers::default());
        let (cineo, views) = start_with(dir.path(), &players);
        wait_for(&views, |v| v.continue_watching.len() == 1);
        cineo.resume("tt0000001".into());
        let playing = wait_for(&views, |v| v.playback.is_some());
        assert_eq!(playing.playback.unwrap().art.title, "First Example Film");
        let request = players.requests.lock().unwrap()[0].clone();
        assert_eq!(request.url.as_str(), "https://media.example/v/1.mp4");
        assert_eq!(request.start_ms, 61_000);
        assert!(
            request
                .headers
                .iter()
                .any(|(k, v)| k == "User-Agent" && v == "Cineo-Test")
        );
        players
            .events
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .send(PlayerEvent::Closed {
                time_ms: 61_000,
                duration_ms: 120_000,
            })
            .unwrap();
        wait_for(&views, |v| v.playback.is_none());
        cineo.remove_from_library("tt0000001".into());
        wait_for(&views, |v| {
            v.continue_watching.is_empty() && v.recent.is_empty()
        });
        drop(cineo);
    });

    assert!(
        Store::open_in(dir.path())
            .unwrap()
            .library()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_favourite_stays_after_a_restart_until_it_is_unmarked_or_cleared() {
    let server = MockServer::start().await;
    let manifest = movie_addon(&server).await;
    let dir = local_data_dir();

    tokio::task::block_in_place(|| {
        let (cineo, views) = start(dir.path());
        cineo.install_addon(manifest);
        wait_for(&views, |v| v.addons.len() == 1);
        cineo.open_detail("movie".into(), "tt0000001".into());
        wait_for(&views, streams_ready);
        cineo.set_favorite("tt0000001".into(), true);
        let marked = wait_for(&views, |v| !v.favorites.is_empty());
        assert!(marked.detail.unwrap().favorite);
        assert_eq!(marked.favorites[0].name, "First Example Film");
        assert!(!marked.favorites[0].played);
        assert!(marked.recent.is_empty() && marked.continue_watching.is_empty());
        drop(cineo);

        let (cineo, views) = start(dir.path());
        wait_for(&views, |v| v.favorites.len() == 1);
        cineo.open_detail("movie".into(), "tt0000001".into());
        let opened = wait_for(&views, |v| v.detail.is_some());
        let detail = opened.detail.unwrap();
        assert_eq!(detail.name.as_deref(), Some("First Example Film"));
        assert!(detail.favorite);
        cineo.set_favorite("tt0000001".into(), false);
        wait_for(&views, |v| {
            v.favorites.is_empty() && v.detail.as_ref().is_some_and(|d| !d.favorite)
        });
        cineo.set_favorite("tt0000001".into(), true);
        wait_for(&views, |v| v.favorites.len() == 1);
        cineo.clear_library();
        wait_for(&views, |v| v.favorites.is_empty());
        drop(cineo);
    });

    assert!(
        Store::open_in(dir.path())
            .unwrap()
            .library()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn changed_settings_reach_the_player_and_survive_a_restart_until_reset() {
    let server = MockServer::start().await;
    let manifest = movie_addon(&server).await;
    let dir = local_data_dir();

    tokio::task::block_in_place(|| {
        let players = Arc::new(FakePlayers::default());
        let (cineo, views) = start_with(dir.path(), &players);
        let first = wait_for(&views, |_| true);
        assert!(first.settings.hardware_decoding);
        assert_eq!(first.settings.seek_steps_s, [5, 10, 15, 30]);
        assert_eq!(first.settings.watched_at_choices, [80, 85, 90, 92, 95]);
        assert!(first.settings.languages.iter().any(|c| c == "spa"));
        cineo.install_addon(manifest);
        wait_for(&views, |v| v.addons.len() == 1);
        for change in [
            SettingChange::HardwareDecoding { on: false },
            SettingChange::SeekStep { seconds: 30 },
            SettingChange::AudioLanguage {
                code: Some("spa".into()),
            },
            SettingChange::SubtitleLanguage {
                code: Some("eng".into()),
            },
            SettingChange::WatchedAt { percent: 80 },
            SettingChange::SeekStep { seconds: 7 },
            SettingChange::AudioLanguage {
                code: Some("klingon".into()),
            },
        ] {
            cineo.change_setting(change);
        }
        wait_for(&views, |v| v.settings.watched_at_percent == 80);
        cineo.open_detail("movie".into(), "tt0000001".into());
        wait_for(&views, streams_ready);
        cineo.play(0, 0);
        let playing = wait_for(&views, |v| v.playback.is_some());
        assert_eq!(playing.playback.unwrap().seek_step_s, 30);
        let request = players.requests.lock().unwrap()[0].clone();
        assert!(!request.settings.hardware_decoding);
        assert_eq!(
            request.settings.audio_language.map(|l| l.code()),
            Some("spa")
        );
        assert_eq!(
            request.settings.subtitle_language.map(|l| l.code()),
            Some("eng")
        );
        drop(cineo);

        let (cineo, views) = start(dir.path());
        let restored = wait_for(&views, |_| true).settings;
        assert!(!restored.hardware_decoding);
        assert_eq!(restored.seek_step_s, 30);
        assert_eq!(restored.audio_language.as_deref(), Some("spa"));
        assert_eq!(restored.subtitle_language.as_deref(), Some("eng"));
        assert_eq!(restored.watched_at_percent, 80);
        assert!(restored.allow_private_network);
        cineo.change_setting(SettingChange::AudioLanguage { code: None });
        wait_for(&views, |v| v.settings.audio_language.is_none());
        cineo.reset_settings();
        let reset = wait_for(&views, |v| v.settings.hardware_decoding).settings;
        assert_eq!(reset.seek_step_s, 10);
        assert_eq!(reset.watched_at_percent, 92);
        assert_eq!(reset.subtitle_language, None);
        assert!(!reset.allow_private_network);
        assert_eq!(reset.torrent_storage.value, "disk");
        drop(cineo);
    });

    let saved = Store::open_in(dir.path()).unwrap().settings().unwrap();
    assert_eq!(
        saved,
        Settings {
            torrent_storage: TorrentStorage::Disk4096,
            ..Settings::default()
        }
    );
}

#[test]
fn a_new_install_keeps_torrent_data_on_disk_until_changed_and_again_after_a_reset() {
    let dir = tempdir::Dir::new();
    Store::open_in(dir.path())
        .unwrap()
        .save_addons(&[])
        .unwrap();

    let (cineo, views) = start(dir.path());
    let first = wait_for(&views, |_| true).settings;
    assert_eq!(first.torrent_storage.value, "disk");
    cineo.change_setting(SettingChange::Value {
        key: "torrent_storage".into(),
        value: "ram-512".into(),
    });
    wait_for(&views, |v| v.settings.torrent_storage.value == "ram-512");
    cineo.reset_settings();
    wait_for(&views, |v| v.settings.torrent_storage.value == "disk");
    drop(cineo);

    let saved = Store::open_in(dir.path()).unwrap().settings().unwrap();
    assert_eq!(saved.torrent_storage, TorrentStorage::Disk4096);
}

#[tokio::test(flavor = "multi_thread")]
async fn subtitle_torrent_and_player_settings_change_by_key_and_reach_the_player() {
    let server = MockServer::start().await;
    let manifest = movie_addon(&server).await;
    let dir = local_data_dir();

    tokio::task::block_in_place(|| {
        let players = Arc::new(FakePlayers::default());
        let (cineo, views) = start_with(dir.path(), &players);
        let first = wait_for(&views, |_| true).settings;
        assert_eq!(first.subtitle_font.key, "subtitle_font");
        assert_eq!(first.subtitle_font.value, "sans");
        assert_eq!(first.subtitle_font.options, ["sans", "serif", "mono"]);
        assert_eq!(first.hide_controls.options, ["1.5", "2.5", "5"]);
        assert_eq!(
            (first.subtitle_size.min, first.subtitle_size.max),
            (50, 200)
        );
        assert_eq!(first.torrent_port.value, 42713);
        assert!(first.torrent_upload.on);
        cineo.install_addon(manifest);
        wait_for(&views, |v| v.addons.len() == 1);
        let value = |key: &str, value: &str| SettingChange::Value {
            key: key.into(),
            value: value.into(),
        };
        for change in [
            value("subtitle_font", "serif"),
            value("subtitle_size", "150"),
            value("subtitle_bold", "true"),
            value("torrent_upload", "false"),
            value("download_limit", "5"),
            value("torrent_port", "50000"),
            value("hide_controls", "5"),
            value("subtitle_font", "comic"),
            value("no_such_setting", "1"),
            SettingChange::SecondaryAudioLanguage {
                code: Some("eng".into()),
            },
            SettingChange::SecondarySubtitleLanguage {
                code: Some("fre".into()),
            },
        ] {
            cineo.change_setting(change);
        }
        let changed =
            wait_for(&views, |v| v.settings.secondary_subtitle_language.is_some()).settings;
        assert_eq!(changed.subtitle_font.value, "serif");
        assert_eq!(changed.subtitle_size.value, 150);
        assert!(changed.subtitle_bold.on);
        assert!(!changed.torrent_upload.on);
        assert_eq!(changed.download_limit.value, "5");
        assert_eq!(changed.torrent_port.value, 50000);
        assert_eq!(changed.secondary_audio_language.as_deref(), Some("eng"));
        cineo.open_detail("movie".into(), "tt0000001".into());
        wait_for(&views, streams_ready);
        cineo.play(0, 0);
        let playing = wait_for(&views, |v| v.playback.is_some());
        assert_eq!(playing.playback.unwrap().hide_controls_ms, 5000);
        let request = players.requests.lock().unwrap()[0].clone();
        assert_eq!(request.settings.subtitle_font, SubtitleFont::Serif);
        assert_eq!(request.settings.subtitle_size.get(), 150);
        assert!(!request.settings.torrent_upload);
        assert_eq!(
            request
                .settings
                .audio_languages()
                .map(|l| l.code())
                .collect::<Vec<_>>(),
            ["eng"]
        );
        drop(cineo);

        let (_cineo, views) = start(dir.path());
        let restored = wait_for(&views, |_| true).settings;
        assert_eq!(restored.subtitle_font.value, "serif");
        assert_eq!(restored.hide_controls.value, "5");
        assert_eq!(restored.secondary_subtitle_language.as_deref(), Some("fre"));
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn addons_can_be_reordered_and_removed_with_their_rows() {
    let server = MockServer::start().await;
    serve(
        &server,
        "/a/manifest.json",
        fixture("addons/basic/manifest.json"),
    )
    .await;
    serve(
        &server,
        "/b/manifest.json",
        fixture("addons/quirks/manifest.json"),
    )
    .await;
    let basic = format!("{}/a/manifest.json", server.uri());
    let quirks = format!("{}/b/manifest.json", server.uri());
    let dir = local_data_dir();
    let names = |v: &AppView| v.addons.iter().map(|a| a.name.clone()).collect::<Vec<_>>();

    tokio::task::block_in_place(|| {
        let (cineo, views) = start(dir.path());
        cineo.install_addon(basic.clone());
        wait_for(&views, |v| v.addons.len() == 1);
        cineo.install_addon(quirks.clone());
        let both = wait_for(&views, |v| v.addons.len() == 2);
        assert_eq!(names(&both), ["Basic Fixture", "Quirks Fixture"]);
        assert!(both.board.iter().any(|r| r.addon_name == "Basic Fixture"));
        cineo.move_addon(1, 0);
        wait_for(&views, |v| names(v) == ["Quirks Fixture", "Basic Fixture"]);
        cineo.move_addon(5, 0);
        cineo.remove_addon("not a url");
        cineo.remove_addon(&basic);
        let removed = wait_for(&views, |v| v.addons.len() == 1);
        assert_eq!(names(&removed), ["Quirks Fixture"]);
        assert!(
            removed
                .board
                .iter()
                .all(|r| r.addon_name != "Basic Fixture")
        );
        drop(cineo);

        let (_cineo, views) = start(dir.path());
        let restored = wait_for(&views, |v| v.addons.len() == 1 && v.addons_loading == 0);
        assert_eq!(restored.addons[0].transport, quirks);
        assert!(restored.unavailable_addons.is_empty());
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn an_addon_that_no_longer_loads_is_listed_and_can_be_removed() {
    let server = MockServer::start().await;
    let manifest = movie_addon(&server).await;
    let dir = local_data_dir();
    tokio::task::block_in_place(|| {
        let (cineo, views) = start(dir.path());
        cineo.install_addon(manifest.clone());
        wait_for(&views, |v| v.addons.len() == 1);
    });
    server.reset().await;

    tokio::task::block_in_place(|| {
        let (cineo, views) = start(dir.path());
        let broken = wait_for(&views, |v| {
            v.addons_loading == 0 && !v.unavailable_addons.is_empty()
        });
        assert!(broken.addons.is_empty());
        assert_eq!(broken.unavailable_addons, std::slice::from_ref(&manifest));
        cineo.remove_addon(&manifest);
        wait_for(&views, |v| v.unavailable_addons.is_empty());
        drop(cineo);
    });
    assert!(
        Store::open_in(dir.path())
            .unwrap()
            .addons()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_search_fills_rows_from_search_catalogs_and_opens_its_results() {
    let server = MockServer::start().await;
    let manifest = movie_addon(&server).await;
    serve(
        &server,
        "/cfg/catalog/movie/search/search=first.json",
        fixture("addons/basic/catalog-movie-top.json"),
    )
    .await;
    let dir = local_data_dir();

    tokio::task::block_in_place(|| {
        let (cineo, views) = start(dir.path());
        cineo.install_addon(manifest);
        wait_for(&views, |v| v.addons.len() == 1);
        cineo.search("  first ".into());
        let found = wait_for(&views, |v| {
            !v.search.is_empty() && v.search.iter().all(|r| r.items != RowItems::Loading)
        });
        assert_eq!(found.search_query, "first");
        assert_eq!(found.search.len(), 1);
        let RowItems::Ready { items } = &found.search[0].items else {
            panic!("the search did not load: {:?}", found.search[0].items);
        };
        assert_eq!(items[0].name, "First Example Film");
        cineo.open_detail("movie".into(), items[0].id.clone());
        let opened = wait_for(&views, |v| v.detail.is_some());
        assert_eq!(
            opened.detail.unwrap().name.as_deref(),
            Some("First Example Film")
        );
        cineo.search(String::new());
        wait_for(&views, |v| v.search.is_empty() && v.search_query.is_empty());
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn addon_lists_offer_installs_and_configuration_links() {
    let server = MockServer::start().await;
    serve(
        &server,
        "/lists/manifest.json",
        fixture("addons/basic/manifest-addon-lists.json"),
    )
    .await;
    serve(
        &server,
        "/cfg/manifest.json",
        fixture("addons/basic/manifest.json"),
    )
    .await;
    let base = server.uri();
    let basic: serde_json::Value =
        serde_json::from_slice(&fixture("addons/basic/manifest.json")).unwrap();
    let mut setup = basic.clone();
    setup["id"] = "org.cineo.fixture.setup".into();
    setup["name"] = "Setup Fixture".into();
    setup["behaviorHints"] =
        serde_json::json!({ "configurable": true, "configurationRequired": true });
    let official = serde_json::json!({ "addons": [
        { "transportUrl": format!("{base}/cfg/manifest.json"), "manifest": basic },
        { "transportUrl": format!("{base}/setup/manifest.json"), "manifest": setup },
    ]});
    serve(
        &server,
        "/lists/addon_catalog/all/official.json",
        official.to_string().into_bytes(),
    )
    .await;
    serve(
        &server,
        "/lists/addon_catalog/all/community.json",
        br#"{"addons":[]}"#.to_vec(),
    )
    .await;
    let dir = local_data_dir();

    tokio::task::block_in_place(|| {
        let (cineo, views) = start(dir.path());
        cineo.install_addon(format!("{base}/lists/manifest.json"));
        wait_for(&views, |v| v.addons.len() == 1);
        cineo.load_addon_lists();
        let loaded = wait_for(&views, |v| {
            v.addon_lists.len() == 2
                && v.addon_lists
                    .iter()
                    .all(|l| l.items != AddonListItems::Loading)
        });
        let names: Vec<_> = loaded.addon_lists.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["Official", "Community"]);
        let AddonListItems::Ready { addons } = &loaded.addon_lists[0].items else {
            panic!("the list did not load: {:?}", loaded.addon_lists[0].items);
        };
        let listed: Vec<_> = addons.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(listed, ["Basic Fixture", "Setup Fixture"]);
        assert!(!addons[0].installed && !addons[0].needs_setup);
        assert!(addons[1].needs_setup);
        assert_eq!(addons[1].configure_url, format!("{base}/setup/configure"));

        cineo.install_addon(addons[0].transport.clone());
        let installed = wait_for(&views, |v| v.addons.len() == 2);
        let AddonListItems::Ready { addons } = &installed.addon_lists[0].items else {
            panic!("the list is gone: {:?}", installed.addon_lists[0].items);
        };
        assert!(addons[0].installed);
        assert_eq!(
            installed.addons[1].configure_url.as_deref(),
            Some(format!("{base}/cfg/configure").as_str())
        );
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn streams_can_be_filtered_and_sorted_like_on_the_desktop() {
    let server = MockServer::start().await;
    serve(
        &server,
        "/cfg/manifest.json",
        fixture("addons/basic/manifest.json"),
    )
    .await;
    serve(
        &server,
        "/cfg/meta/movie/tt0000001.json",
        fixture("addons/basic/meta-movie.json"),
    )
    .await;
    let streams = r#"{"streams":[
        {"name":"Fixture\n720p","description":"Film.720p.WEB 👤 5 💾 700 MB","url":"https://media.example/720.mp4"},
        {"name":"Fixture\n4k HDR","description":"Film.2160p.REMUX 👤 2 💾 15 GB","url":"https://media.example/4k.mkv"},
        {"name":"Fixture\n1080p","description":"Film.1080p.WEB 👤 40 💾 2 GB","url":"https://media.example/1080.mp4"},
        {"name":"Fixture\n1080p","description":"Film.1080p.BluRay 👤 90","infoHash":"0123456789abcdef0123456789abcdef01234567"}
    ]}"#;
    serve(
        &server,
        "/cfg/stream/movie/tt0000001.json",
        streams.as_bytes().to_vec(),
    )
    .await;
    let manifest = format!("{}/cfg/manifest.json", server.uri());
    let dir = local_data_dir();
    let order = |v: &AppView| -> (Vec<u32>, u32) {
        let detail = v.detail.as_ref().unwrap();
        match &detail.streams[0].items {
            StreamItems::Ready { order, hidden, .. } => (order.clone(), *hidden),
            other => panic!("streams are not ready: {other:?}"),
        }
    };

    tokio::task::block_in_place(|| {
        let (cineo, views) = start(dir.path());
        cineo.install_addon(manifest);
        wait_for(&views, |v| v.addons.len() == 1);
        cineo.open_detail("movie".into(), "tt0000001".into());
        let ready = wait_for(&views, streams_ready);
        assert_eq!(order(&ready), (vec![0, 1, 2, 3], 0));
        let detail = ready.detail.as_ref().unwrap();
        let shown = [Resolution::Uhd, Resolution::FullHd, Resolution::Hd];
        assert_eq!(detail.qualities, shown);
        let StreamItems::Ready { streams, .. } = &detail.streams[0].items else {
            unreachable!()
        };
        let four_k = streams[1].quality.as_ref().unwrap();
        assert_eq!(
            (four_k.resolution, four_k.tier, four_k.flags.clone()),
            (Resolution::Uhd, Tier::Ultra, vec!["HDR".to_owned()])
        );
        assert_eq!(streams[2].seeders, Some(40));
        assert_eq!(streams[0].size, Some(734_003_200));

        cineo.set_stream_sort(StreamSort::Seeders);
        wait_for(&views, |v| order(v).0 == [3, 2, 0, 1]);
        cineo.set_stream_quality(Some(Resolution::FullHd));
        wait_for(&views, |v| order(v).0 == [3, 2]);
        cineo.set_stream_query("web".into());
        wait_for(&views, |v| order(v).0 == [2]);
        cineo.set_stream_addon(Some(1));
        wait_for(&views, |v| order(v).0.is_empty());
        cineo.set_stream_addon(None);
        cineo.set_stream_query(String::new());
        cineo.set_stream_quality(Some(Resolution::Sd));
        wait_for(&views, |v| order(v).0.is_empty());
        cineo.set_stream_quality(None);
        cineo.change_setting(SettingChange::P2pEnabled { on: false });
        let off = wait_for(&views, |v| order(v) == (vec![2, 0, 1], 1));
        assert_eq!(off.detail.unwrap().qualities, shown);
        cineo.change_setting(SettingChange::P2pEnabled { on: true });
        cineo.set_stream_query("bluray".into());
        wait_for(&views, |v| order(v).0 == [3]);

        cineo.close_detail();
        wait_for(&views, |v| v.detail.is_none());
        cineo.open_detail("movie".into(), "tt0000001".into());
        let reopened = wait_for(&views, streams_ready);
        let filter = &reopened.detail.as_ref().unwrap().filter;
        assert_eq!(filter.query, "");
        assert_eq!(filter.sort, StreamSort::Seeders);
        assert_eq!(order(&reopened).0, [3, 2, 0, 1]);
    });
}

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
    serve(
        server,
        "/cfg/manifest.json",
        fixture("addons/basic/manifest.json"),
    )
    .await;
    serve(
        server,
        "/cfg/meta/movie/tt0000001.json",
        fixture("addons/basic/meta-movie.json"),
    )
    .await;
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
        cineo.install_addon(manifest);
        wait_for(&views, |v| v.addons.len() == 1);
        cineo.open_detail("movie".into(), "tt0000001".into());
        wait_for(&views, streams_ready);
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
        cineo.install_addon(manifest);
        wait_for(&views, |v| v.addons.len() == 1);
        cineo.open_detail("movie".into(), "tt0000001".into());
        wait_for(&views, streams_ready);
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

mod tempdir {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT: AtomicU32 = AtomicU32::new(0);

    pub(super) struct Dir(PathBuf);

    impl Dir {
        pub(super) fn new() -> Self {
            let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
                "cineo-android-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        pub(super) fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[test]
fn the_app_reports_the_workspace_version() {
    let manifest =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml"))
            .unwrap();
    let workspace = manifest
        .split("[workspace.package]")
        .nth(1)
        .and_then(|section| section.lines().find_map(|l| l.strip_prefix("version = ")))
        .map(|v| v.trim_matches('"'))
        .unwrap();
    assert_eq!(version(), workspace);
}

fn binge_streams(file: &str) -> Vec<u8> {
    format!(
        r#"{{"streams":[{{"name":"1080p","url":"https://media.example/{file}.mp4","behaviorHints":{{"bingeGroup":"ex-1080"}}}}]}}"#
    )
    .into_bytes()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_player_offers_the_next_episode_and_plays_it_on_request() {
    let server = MockServer::start().await;
    serve(
        &server,
        "/cfg/manifest.json",
        fixture("addons/basic/manifest.json"),
    )
    .await;
    serve(
        &server,
        "/cfg/meta/series/tt0000010.json",
        fixture("addons/basic/meta-series.json"),
    )
    .await;
    Mock::given(method("GET"))
        .and(path_regex(
            r"^/cfg/stream/series/tt0000010(:|%3A)1(:|%3A)1\.json$",
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(binge_streams("e1"), "application/json"),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(
            r"^/cfg/stream/series/tt0000010(:|%3A)1(:|%3A)2\.json$",
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(binge_streams("e2"), "application/json"),
        )
        .mount(&server)
        .await;
    let manifest = format!("{}/cfg/manifest.json", server.uri());
    let dir = local_data_dir();
    let players = Arc::new(FakePlayers::default());

    tokio::task::block_in_place(|| {
        let (cineo, views) = start_with(dir.path(), &players);
        cineo.install_addon(manifest);
        wait_for(&views, |v| v.addons.len() == 1);
        cineo.open_detail("series".into(), "tt0000010".into());
        wait_for(&views, |v| {
            v.detail
                .as_ref()
                .is_some_and(|d| d.meta == MetaStatus::Ready)
        });
        cineo.select_video("tt0000010:1:1".into());
        wait_for(&views, streams_ready);
        cineo.play(0, 0);
        let playing = wait_for(&views, |v| v.playback.is_some()).playback.unwrap();
        assert!(playing.binge_watching);
        assert_eq!(playing.next_notice_s, 35);
        assert_eq!(playing.next, None);

        players
            .events
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .send(PlayerEvent::Progress {
                time_ms: 10_000,
                duration_ms: 1_320_000,
            })
            .unwrap();
        let next = wait_for(&views, |v| {
            v.playback.as_ref().is_some_and(|p| p.next.is_some())
        })
        .playback
        .unwrap()
        .next
        .unwrap();
        assert!(next.title.contains("Pilot, Part Two"), "{}", next.title);

        cineo.pause();
        assert!(
            players
                .calls
                .lock()
                .unwrap()
                .iter()
                .any(|c| c == "SetPause(true)")
        );
        wait_for(&views, |v| {
            v.playback
                .as_ref()
                .and_then(|p| p.next.as_ref())
                .is_some_and(|n| n.ready)
        });
        cineo.play_next();
        wait_for(&views, |v| {
            v.playback.as_ref().is_some_and(|p| {
                p.next
                    .as_ref()
                    .is_none_or(|n| !n.title.contains("Part Two"))
            })
        });
        let requests = players.requests.lock().unwrap().clone();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[1].url.as_str(), "https://media.example/e2.mp4");
        assert_eq!(requests[1].video_id, "tt0000010:1:2");
        drop(cineo);
    });
}
