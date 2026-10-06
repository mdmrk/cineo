//! The imperative shell (ADR-0001, ADR-0011): owns the [`State`], runs the
//! effects [`update`] returns, and feeds IO results back as actions.
//!
//! - Addon fetches and playback run on the tokio runtime; their results come
//!   back through a channel that is drained before each frame.
//! - Store writes go, in order, to one dedicated thread.
//! - Rendering ([`crate::view`]) performs no IO.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context as _;
use cineo_core::app::{Action, Effect, PlayRequest, State, update};
use cineo_net::{AddonClient, NetPolicy};
use cineo_player_mpv::PlayerEvent;
use cineo_store::Store;
use eframe::egui;
use tracing::{debug, error};

use crate::images::NetImageLoader;
use crate::theme;
use crate::view::{self, ViewState};

/// Startup options (command line).
#[derive(Debug, Clone)]
pub struct Options {
    /// Directory holding `cineo.db`.
    pub data_dir: PathBuf,
    /// Allow addons and images on loopback/LAN addresses (docs/SECURITY.md).
    pub allow_private_network: bool,
    /// The mpv executable; `None` means `mpv` on `PATH`.
    pub mpv: Option<PathBuf>,
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
        mpv: options.mpv,
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
            // Decoders only (`image` feature); bytes come from `NetImageLoader`.
            egui_extras::install_image_loaders(&cc.egui_ctx);
            cc.egui_ctx
                .add_bytes_loader(Arc::new(NetImageLoader::new(client, handle)));
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
    pub(crate) mpv: Option<PathBuf>,
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
    playback: Option<tokio::task::AbortHandle>,
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
            Effect::Play(request) => self.play(request),
            // The streaming engine is not wired in yet; the view keeps
            // torrent streams disabled until it is.
            Effect::StartTorrent(request) => {
                self.spawn(async move {
                    Action::TorrentFailed {
                        info_hash: request.info_hash,
                        reason: "torrent streaming is not available in this build".into(),
                    }
                });
            }
            Effect::StopTorrent => {}
        }
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

    /// Starts mpv, replacing any running playback, and reports its progress.
    fn play(&mut self, request: PlayRequest) {
        if let Some(previous) = self.playback.take() {
            previous.abort(); // drops the handle, which closes that mpv
        }
        let tx = self.results_tx.clone();
        let ctx = self.ctx.clone();
        let mpv = self.io.mpv.clone();
        let task = self.io.runtime.spawn(async move {
            let meta_id = request.meta_id.clone();
            let video_id = request.video_id.clone();
            let send = |action| {
                let _ = tx.send(Msg::Action(action));
                ctx.request_repaint();
            };
            let mut handle = match cineo_player_mpv::launch(mpv, request).await {
                Ok(handle) => handle,
                Err(err) => {
                    send(Action::PlaybackFailed(err.to_string()));
                    return;
                }
            };
            while let Some(event) = handle.events.recv().await {
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
        });
        self.playback = Some(task.abort_handle());
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
        for action in view::show(ui, &self.state, &mut self.view) {
            self.dispatch(action);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.store_tx = None;
        if let Some(playback) = self.playback.take() {
            playback.abort();
        }
    }
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
