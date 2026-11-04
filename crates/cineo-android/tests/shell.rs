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
    AppView, Cineo, ImageError, InstallView, MetaStatus, Playback, Players, ProblemView, RowItems,
    SettingChange, Started, StateListener, StreamItems, TorrentView,
};
use cineo_core::app::PlayRequest;
use cineo_core::app::Settings;
use cineo_player::embedded::{PlayerCommand, Status};
use cineo_player::{PlayerError, PlayerEvent};
use cineo_store::Store;
use cineo_stream::EngineOptions;
use librqbit::spawn_utils::BlockingSpawner;
use librqbit::{
    AddTorrent, AddTorrentOptions, CreateTorrentOptions, ListenerOptions, Session, SessionOptions,
    create_torrent,
};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use wiremock::matchers::{method, path};
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
}

#[derive(Debug)]
struct FakePlayback(Arc<Mutex<Vec<String>>>);

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

    fn status(&self) -> Status {
        Status {
            loaded: true,
            position_s: 12.0,
            duration_s: 120.0,
            ..Status::default()
        }
    }
}

impl Players for FakePlayers {
    fn start(&self, request: &PlayRequest) -> Result<Started, PlayerError> {
        self.requests.lock().unwrap().push(request.clone());
        let (tx, rx) = unbounded_channel();
        *self.events.lock().unwrap() = Some(tx);
        Ok((Box::new(FakePlayback(Arc::clone(&self.calls))), rx))
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
        let StreamItems::Ready { streams } = &detail.streams[0].items else {
            panic!("streams did not load: {:?}", detail.streams);
        };
        assert_eq!(streams[0].name.as_deref(), Some("1080p"));

        cineo.play(0, 0);
        let playing = wait_for(&views, |v| v.playback.is_some());
        assert_eq!(playing.playback.unwrap().title, "First Example Film");
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

        cineo.resize_surface(1080, 2340);
        cineo.detach_surface();
        cineo.toggle_pause();
        cineo.stop_playback();
        assert_eq!(
            players.calls.lock().unwrap().as_slice(),
            [
                "attach 7",
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
        assert_eq!(playing.playback.unwrap().title, "First Example Film");
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
        drop(cineo);
    });

    let saved = Store::open_in(dir.path()).unwrap().settings().unwrap();
    assert_eq!(saved, Settings::default());
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
            |v| matches!(v.torrent, Some(TorrentView::Streaming { peers, .. }) if peers > 0),
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
        assert_eq!(connecting.connecting.as_deref(), Some("First Example Film"));
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
