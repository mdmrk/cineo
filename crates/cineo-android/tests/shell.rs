#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers panic on purpose"
)]

use std::path::Path;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cineo_android::{
    AppView, Cineo, ImageError, InstallView, MetaStatus, Playback, Players, ProblemView, RowItems,
    Started, StateListener, StreamItems,
};
use cineo_core::app::PlayRequest;
use cineo_core::app::Settings;
use cineo_player::embedded::{PlayerCommand, Status};
use cineo_player::{PlayerError, PlayerEvent};
use cineo_store::Store;
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
    let cineo = Cineo::new(dir.to_str().unwrap(), Arc::new(Views(Mutex::new(tx)))).unwrap();
    (cineo, rx)
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
        let (tx, views) = channel();
        let cineo = Cineo::start(
            dir.path().to_str().unwrap(),
            Arc::new(Views(Mutex::new(tx))),
            Arc::clone(&players) as Arc<dyn Players>,
        )
        .unwrap();
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
