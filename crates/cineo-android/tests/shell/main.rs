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
    AddonListItems, AddonSubtitleView, AppView, CachedSpanView, Cineo, DecoderView, ImageError,
    InstallView, MetaStatus, Playback, PlayerStatsView, Players, ProblemView, RowItems,
    SettingChange, Started, StateListener, StreamItems, TorrentView, TrackView, VideoSizeView,
    version,
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
use tempfile::TempDir;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[derive(Debug)]
struct Views(Mutex<Sender<AppView>>);

mod browse;
mod playback;
mod settings;
mod torrent;

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
        dir.join("fonts").to_str().unwrap(),
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
            cached: Arc::from([(0.0, 30.0), (60.0, 120.0)]),
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

async fn serve_manifest(server: &MockServer) {
    serve(
        server,
        "/cfg/manifest.json",
        fixture("addons/basic/manifest.json"),
    )
    .await;
}

async fn serve_movie(server: &MockServer) {
    serve_manifest(server).await;
    serve(
        server,
        "/cfg/meta/movie/tt0000001.json",
        fixture("addons/basic/meta-movie.json"),
    )
    .await;
}

fn open_movie(cineo: &Cineo, views: &Receiver<AppView>, manifest: String) {
    cineo.install_addon(manifest);
    wait_for(views, |v| v.addons.len() == 1);
    cineo.open_detail("movie".into(), "tt0000001".into());
    wait_for(views, streams_ready);
}

fn play_a_minute_and_close(players: &FakePlayers) {
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
}

async fn movie_addon(server: &MockServer) -> String {
    serve_movie(server).await;
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

fn scratch_dir() -> TempDir {
    tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap()
}

fn local_data_dir() -> TempDir {
    let dir = scratch_dir();
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

fn shell_caught_up(cineo: &Cineo, views: &Receiver<AppView>) {
    for on in ["true", "false"] {
        cineo.change_setting(SettingChange::Value {
            key: "subtitle_bold".into(),
            value: on.into(),
        });
        wait_for(views, |v| v.settings.subtitle_bold.on.to_string() == on);
    }
}
