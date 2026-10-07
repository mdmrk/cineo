//! The imperative shell (ADR-0001, ADR-0011): owns the [`State`], runs the
//! effects [`update`] returns, and feeds IO results back as actions.
//!
//! - Addon fetches and playback run on the tokio runtime; their results come
//!   back through a channel that is drained before each frame.
//! - Store writes go, in order, to one dedicated thread.
//! - The torrent engine starts with the first torrent played (ADR-0012).
//! - Rendering ([`crate::view`]) performs no IO.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context as _;
use cineo_core::app::{Action, Effect, PlayRequest, State, TorrentRequest, update};
use cineo_net::{AddonClient, NetPolicy};
use cineo_player::PlayerError;
use cineo_player::PlayerEvent;
use cineo_player::embedded::{Player, PlayerCommand, ProcAddress, Renderer, Status, Video};
use cineo_store::Store;
use cineo_stream::{Engine, EngineOptions};
use eframe::egui;
use tracing::{debug, error};

use crate::images::NetImageLoader;
use crate::player::{self, Controls};
use crate::theme;
use crate::view::{self, ViewState};

/// Startup options (command line).
#[derive(Debug, Clone)]
pub struct Options {
    /// Directory holding `cineo.db`.
    pub data_dir: PathBuf,
    /// Directory for torrent data.
    pub cache_dir: PathBuf,
    /// Allow addons, images and torrent peers on loopback/LAN addresses
    /// (docs/SECURITY.md).
    pub allow_private_network: bool,
}

/// Opens the store, restores state and runs the window until it is closed.
pub fn run(options: Options) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("cannot start the async runtime")?;
    let client = Arc::new(AddonClient::new(NetPolicy {
        allow_private_networks: options.allow_private_network,
        ..NetPolicy::default()
    })?);
    let store = Store::open_in(&options.data_dir)
        .with_context(|| format!("cannot open the database in {}", options.data_dir.display()))?;
    let restore = Action::Restore {
        addons: store.addons()?,
        library: store.library()?,
    };
    let settings = Action::RestoreSettings(store.settings()?);
    let (results_tx, results_rx) = channel();
    let (store_tx, writer) =
        spawn_store_writer(store, results_tx.clone()).context("cannot start the store thread")?;
    let io = Io {
        runtime: runtime.handle().clone(),
        client: Arc::clone(&client),
        gl: None,
        engine: EngineOptions {
            allow_private_network: options.allow_private_network,
            ..EngineOptions::new(options.cache_dir)
        },
    };
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Cineo")
            .with_app_id("cineo")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };
    let handle = runtime.handle().clone();
    let result = eframe::run_native(
        "Cineo",
        native,
        Box::new(move |cc| {
            theme::apply(&cc.egui_ctx);
            cc.egui_ctx
                .add_image_loader(Arc::new(NetImageLoader::new(client, handle)));
            let mut io = io;
            io.gl = cc.get_proc_address.clone();
            Ok(Box::new(CineoApp::new(
                cc.egui_ctx.clone(),
                io,
                [restore, settings],
                store_tx,
                (results_tx, results_rx),
            )))
        }),
    );
    // Every store sender is gone once the window closes; wait for the last
    // writes (e.g. the final playback position).
    if writer.join().is_err() {
        error!("the store thread panicked");
    }
    result.map_err(|err| anyhow::anyhow!("the window failed: {err}"))
}

/// What background work reports back to the UI thread.
#[derive(Debug)]
pub(crate) enum Msg {
    Action(Action),
    /// A problem outside the core's vocabulary (e.g. a failed save).
    Notice(String),
}

/// Everything the shell needs to run effects.
pub(crate) struct Io {
    pub(crate) runtime: tokio::runtime::Handle,
    pub(crate) client: Arc<AddonClient>,
    /// Resolves OpenGL functions in the window's context, for the embedded
    /// player (set once the window exists).
    pub(crate) gl: Option<ProcAddress>,
    pub(crate) engine: EngineOptions,
}

/// How often the status of a streaming torrent is reported.
const TORRENT_STATUS_INTERVAL: Duration = Duration::from_secs(1);

/// The running playback: mpv drawing into this window (ADR-0014).
struct Embedded {
    player: Player,
    /// Shared with the paint callback. Dropped on the UI thread, where the
    /// GL context is current, before `player`.
    renderer: Arc<Mutex<Option<Renderer>>>,
    title: String,
    controls: Controls,
    /// Forwards the player's events as actions. Aborted when the playback
    /// is replaced; `None` once it ended, so its last events still arrive.
    forward: Option<tokio::task::AbortHandle>,
}

impl Drop for Embedded {
    fn drop(&mut self) {
        if let Some(forward) = self.forward.take() {
            forward.abort();
        }
        self.renderer
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
    }
}

pub(crate) struct CineoApp {
    state: State,
    view: ViewState,
    io: Io,
    ctx: egui::Context,
    results_tx: Sender<Msg>,
    results_rx: Receiver<Msg>,
    /// Taken on exit so the store thread can finish its last writes.
    store_tx: Option<Sender<Effect>>,
    playback: Option<Box<Embedded>>,
    /// The player screen's controls while a torrent is being prepared.
    connecting_controls: Controls,
    /// The engine, once a torrent has been played.
    engine: Arc<tokio::sync::Mutex<Option<Engine>>>,
    /// Bumped by every start or stop, so stale torrent work gives up.
    torrent_generation: Arc<AtomicU64>,
    /// Opens the current torrent, then reports its status.
    torrent_task: Option<tokio::task::AbortHandle>,
    seq: u64,
}

impl CineoApp {
    /// Creates the app and dispatches `restore` (the saved addons, library
    /// and settings).
    pub(crate) fn new(
        ctx: egui::Context,
        io: Io,
        restore: [Action; 2],
        store_tx: Sender<Effect>,
        (results_tx, results_rx): (Sender<Msg>, Receiver<Msg>),
    ) -> Self {
        let mut app = Self {
            state: State::default(),
            view: ViewState::default(),
            io,
            ctx,
            results_tx,
            results_rx,
            store_tx: Some(store_tx),
            playback: None,
            connecting_controls: Controls::default(),
            engine: Arc::default(),
            torrent_generation: Arc::default(),
            torrent_task: None,
            seq: 0,
        };
        for action in restore {
            app.dispatch(action);
        }
        app
    }

    fn dispatch(&mut self, action: Action) {
        self.seq += 1;
        let effects = update(&mut self.state, action);
        debug!(seq = self.seq, effects = effects.len(), "dispatch");
        for effect in effects {
            self.run(effect);
        }
    }

    fn run(&mut self, effect: Effect) {
        let client = Arc::clone(&self.io.client);
        match effect {
            Effect::FetchManifest { transport, install } => {
                self.spawn(async move {
                    let result = client
                        .fetch_manifest(&transport)
                        .await
                        .map(|m| Box::new(m.value))
                        .map_err(|e| e.to_string());
                    Action::ManifestLoaded {
                        transport,
                        result,
                        install,
                    }
                });
            }
            Effect::FetchCatalog { addon, path } => {
                self.spawn(async move {
                    let result = client
                        .fetch_catalog(&addon, &path)
                        .await
                        .map(|c| c.value.metas)
                        .map_err(|e| e.to_string());
                    Action::CatalogLoaded {
                        addon,
                        path,
                        result,
                    }
                });
            }
            Effect::FetchMeta { addon, path } => {
                self.spawn(async move {
                    let result = client
                        .fetch_meta(&addon, &path)
                        .await
                        .map(|m| Box::new(m.value))
                        .map_err(|e| e.to_string());
                    Action::MetaLoaded {
                        addon,
                        path,
                        result,
                    }
                });
            }
            Effect::FetchStreams { addon, path } => {
                self.spawn(async move {
                    let result = client
                        .fetch_streams(&addon, &path)
                        .await
                        .map(|s| s.value)
                        .map_err(|e| e.to_string());
                    Action::StreamsLoaded {
                        addon,
                        path,
                        result,
                    }
                });
            }
            effect @ (Effect::SaveAddons(_)
            | Effect::SaveLibraryItem(_)
            | Effect::DeleteLibraryItem(_)
            | Effect::SaveSettings(_)) => {
                let sent = self
                    .store_tx
                    .as_ref()
                    .is_some_and(|tx| tx.send(effect).is_ok());
                if !sent {
                    self.state.notice = Some("Changes can no longer be saved".into());
                }
            }
            Effect::Play(request) => self.play(&request),
            Effect::StartTorrent(request) => self.start_torrent(request),
            Effect::StopTorrent => self.stop_torrent(),
        }
    }

    /// Starts the engine if needed, opens `request`'s torrent, answers with
    /// `TorrentReady` or `TorrentFailed`, then reports its status until a
    /// newer start or stop.
    fn start_torrent(&mut self, request: TorrentRequest) {
        let generation = self.next_torrent_generation();
        let current = Arc::clone(&self.torrent_generation);
        let engine = Arc::clone(&self.engine);
        let options = self.io.engine.clone();
        let tx = self.results_tx.clone();
        let ctx = self.ctx.clone();
        let task = self.io.runtime.spawn(async move {
            let send = |action| {
                let _ = tx.send(Msg::Action(action));
                ctx.request_repaint();
            };
            let info_hash = request.info_hash.clone();
            let opened = {
                let mut engine = engine.lock().await;
                if current.load(Ordering::SeqCst) != generation {
                    return;
                }
                open_torrent(&mut engine, options, &request).await
            };
            match opened {
                Ok(url) => send(Action::TorrentReady {
                    info_hash: info_hash.clone(),
                    url,
                }),
                Err(reason) => {
                    send(Action::TorrentFailed { info_hash, reason });
                    return;
                }
            }
            let mut ticks = tokio::time::interval(TORRENT_STATUS_INTERVAL);
            loop {
                ticks.tick().await;
                if current.load(Ordering::SeqCst) != generation {
                    return;
                }
                let status = engine.lock().await.as_ref().and_then(Engine::status);
                match status {
                    Some((hash, status)) if hash == info_hash => send(Action::TorrentStatus {
                        info_hash: info_hash.clone(),
                        status,
                    }),
                    _ => return,
                }
            }
        });
        self.torrent_task = Some(task.abort_handle());
    }

    /// Stops the current torrent; its data stays in the cache.
    fn stop_torrent(&mut self) {
        let generation = self.next_torrent_generation();
        let current = Arc::clone(&self.torrent_generation);
        let engine = Arc::clone(&self.engine);
        self.io.runtime.spawn(async move {
            let engine = engine.lock().await;
            // A newer start replaces the torrent itself.
            if current.load(Ordering::SeqCst) == generation
                && let Some(engine) = engine.as_ref()
            {
                engine.stop().await;
            }
        });
    }

    /// Cancels the running torrent task and returns the new generation.
    fn next_torrent_generation(&mut self) -> u64 {
        if let Some(task) = self.torrent_task.take() {
            task.abort();
        }
        self.torrent_generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Runs `task` and dispatches the action it returns.
    fn spawn(&self, task: impl Future<Output = Action> + Send + 'static) {
        let tx = self.results_tx.clone();
        let ctx = self.ctx.clone();
        self.io.runtime.spawn(async move {
            let _ = tx.send(Msg::Action(task.await));
            ctx.request_repaint();
        });
    }

    /// Plays `request` in the window, replacing any running playback, and
    /// reports its progress.
    fn play(&mut self, request: &PlayRequest) {
        self.playback = None;
        match self.start_player(request) {
            Ok(embedded) => self.playback = Some(Box::new(embedded)),
            Err(err) => {
                error!(%err, "playback could not start");
                self.sender()(Action::PlaybackFailed(err.to_string()));
            }
        }
    }

    fn start_player(&self, request: &PlayRequest) -> Result<Embedded, PlayerError> {
        let gl = self
            .io
            .gl
            .clone()
            .ok_or_else(|| PlayerError::Render("the window has no OpenGL context".into()))?;
        let (repaint, on_frame) = (self.ctx.clone(), self.ctx.clone());
        let video = Video {
            get_proc_address: gl,
            on_frame: Box::new(move || on_frame.request_repaint()),
        };
        let (player, renderer, events) =
            Player::start(request, Arc::new(move || repaint.request_repaint()), video)?;
        let (meta_id, video_id) = (request.meta_id.clone(), request.video_id.clone());
        let send = self.sender();
        let forward = self.io.runtime.spawn(async move {
            let mut events = events;
            forward_events(&mut events, meta_id, video_id, send).await;
        });
        Ok(Embedded {
            player,
            renderer: Arc::new(Mutex::new(Some(renderer))),
            title: request.title.clone(),
            controls: Controls::default(),
            forward: Some(forward.abort_handle()),
        })
    }

    /// Sends an action to the UI thread and wakes it.
    fn sender(&self) -> impl Fn(Action) + Send + Sync + 'static {
        let tx = self.results_tx.clone();
        let ctx = self.ctx.clone();
        move |action| {
            let _ = tx.send(Msg::Action(action));
            ctx.request_repaint();
        }
    }

    /// While a torrent is prepared for playback, shows the player screen
    /// with its spinner, so Play goes straight to the player. Back cancels.
    fn show_connecting(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(title) = self
            .state
            .torrent
            .as_ref()
            .and_then(|t| t.connecting_title())
            .map(str::to_owned)
        else {
            return false;
        };
        let mut commands = Vec::new();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::BLACK))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                commands = player::show(
                    ui,
                    rect,
                    &Status::default(),
                    &title,
                    &mut self.connecting_controls,
                );
            });
        if commands.contains(&PlayerCommand::Stop) {
            self.connecting_controls = Controls::default();
            player::leave_fullscreen(&self.ctx);
            self.dispatch(Action::PlaybackStopped);
        }
        true
    }

    /// Draws the embedded player over the whole window. Returns `false`
    /// when there is none (or it just ended).
    fn show_player(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(embedded) = &mut self.playback else {
            return false;
        };
        if embedded.player.is_finished() {
            // The forwarding task ends by itself after the last event.
            embedded.forward = None;
            self.playback = None;
            player::leave_fullscreen(&self.ctx);
            return false;
        }
        let status = embedded.player.status();
        let renderer = Arc::clone(&embedded.renderer);
        let mut commands = Vec::new();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::BLACK))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                ui.painter().add(egui::PaintCallback {
                    rect,
                    callback: Arc::new(eframe::egui_glow::CallbackFn::new(move |info, _| {
                        if let Some(renderer) = renderer
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .as_mut()
                        {
                            let [w, h] = info.screen_size_px;
                            renderer.draw(0, w, h);
                        }
                    })),
                });
                commands = player::show(ui, rect, &status, &embedded.title, &mut embedded.controls);
            });
        for command in commands {
            embedded.player.send(command);
        }
        true
    }
}

/// Turns player events into progress, stop and failure actions until the
/// playback ends.
async fn forward_events(
    events: &mut tokio::sync::mpsc::UnboundedReceiver<PlayerEvent>,
    meta_id: String,
    video_id: String,
    send: impl Fn(Action),
) {
    while let Some(event) = events.recv().await {
        let (time_ms, duration_ms, last) = match event {
            PlayerEvent::Progress {
                time_ms,
                duration_ms,
            } => (time_ms, duration_ms, false),
            PlayerEvent::Ended {
                time_ms,
                duration_ms,
            }
            | PlayerEvent::Closed {
                time_ms,
                duration_ms,
            } => (time_ms, duration_ms, true),
            PlayerEvent::Failed(reason) => {
                send(Action::PlaybackFailed(reason));
                break;
            }
        };
        if time_ms > 0 {
            send(Action::PlaybackProgress {
                meta_id: meta_id.clone(),
                video_id: video_id.clone(),
                time_ms,
                duration_ms,
                now_ms: now_ms(),
            });
        }
        if last {
            send(Action::PlaybackStopped);
            break;
        }
    }
}

impl eframe::App for CineoApp {
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(msg) = self.results_rx.try_recv() {
            match msg {
                Msg::Action(action) => self.dispatch(action),
                Msg::Notice(notice) => self.state.notice = Some(notice),
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.show_player(ui) || self.show_connecting(ui) {
            return;
        }
        for action in view::show(ui, &self.state, &mut self.view) {
            self.dispatch(action);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.store_tx = None;
        // The GL context is current here, as the renderer requires.
        self.playback = None;
        self.next_torrent_generation();
        let engine = Arc::clone(&self.engine);
        let shutdown = async move {
            if let Some(engine) = engine.lock().await.take() {
                engine.shutdown().await;
            }
        };
        // Bounded: data is already on disk, so a slow shutdown is not worth
        // keeping the window around for.
        let _ = self
            .io
            .runtime
            .block_on(async { tokio::time::timeout(Duration::from_secs(2), shutdown).await });
    }
}

/// Opens `request` on the engine in `slot`, starting it first if needed.
/// Errors are user-facing text.
async fn open_torrent(
    slot: &mut Option<Engine>,
    options: EngineOptions,
    request: &TorrentRequest,
) -> Result<url::Url, String> {
    let engine = match slot {
        Some(engine) => engine,
        None => slot.insert(Engine::start(options).await.map_err(|e| e.to_string())?),
    };
    engine.open(request).await.map_err(|err| {
        error!(error = %err, "opening the torrent failed");
        err.to_string()
    })
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Applies persistence effects in order on their own thread. The thread
/// ends, after the last write, when every sender is dropped; join it before
/// exiting.
pub(crate) fn spawn_store_writer(
    mut store: Store,
    results: Sender<Msg>,
) -> std::io::Result<(Sender<Effect>, std::thread::JoinHandle<()>)> {
    let (tx, rx) = channel::<Effect>();
    let thread = std::thread::Builder::new()
        .name("store".into())
        .spawn(move || {
            for effect in rx {
                if let Err(err) = store.apply(&effect) {
                    error!(error = %err, "saving failed");
                    let _ = results.send(Msg::Notice(format!("Could not save changes: {err}")));
                }
            }
        })?;
    Ok((tx, thread))
}
