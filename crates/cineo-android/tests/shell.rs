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
    AppView, Cineo, ImageError, InstallView, ProblemView, RowItems, StateListener,
};
use cineo_core::app::Settings;
use cineo_store::Store;
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
