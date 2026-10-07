//! The imperative shell (ADR-0001, ADR-0011): owns the [`State`], runs the
//! effects [`update`] returns, and feeds IO results back as actions.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context as _;
use cineo_core::app::{
    Action, Effect, Language, Percent, PlayRequest, Setting, Settings, State, TorrentRequest,
    update,
};
use cineo_net::{AddonClient, NetPolicy};
use cineo_player::PlayerError;
use cineo_player::PlayerEvent;
use cineo_player::embedded::{Player, PlayerCommand, ProcAddress, Renderer, Status, Video};
use cineo_store::Store;
use cineo_stream::{Engine, EngineOptions};
use eframe::egui;
use tracing::{debug, error, warn};

use crate::images::{self, NetImageLoader};
use crate::player::{self, Controls, Playback};
use crate::subtitles::{self, AutoPick, SubtitleFiles};
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
            images::install(&cc.egui_ctx, NetImageLoader::new(client, handle));
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
    if writer.join().is_err() {
        error!("the store thread panicked");
    }
    // Dropping the runtime would wait for every blocking task, such as a tracker DNS lookup.
    runtime.shutdown_background();
    result.map_err(|err| anyhow::anyhow!("the window failed: {err}"))
}

#[derive(Debug)]
pub(crate) enum Msg {
    Action(Action),
    Notice(String),
    SubtitleFetched {
        url: url::Url,
        result: Result<Vec<u8>, String>,
    },
}

pub(crate) struct Io {
    pub(crate) runtime: tokio::runtime::Handle,
    pub(crate) client: Arc<AddonClient>,
    pub(crate) gl: Option<ProcAddress>,
    pub(crate) engine: EngineOptions,
}

const TORRENT_STATUS_INTERVAL: Duration = Duration::from_secs(1);

struct Embedded {
    player: Player,
    renderer: Arc<Mutex<Option<Renderer>>>,
    title: String,
    logo: Option<String>,
    controls: Controls,
    forward: Option<tokio::task::AbortHandle>,
    subtitle_files: SubtitleFiles,
    /// The preferred languages, until a subtitle in one is on or the user
    /// picks one.
    auto_subtitle: Vec<Language>,
    /// The volume the playback is at, once loaded.
    volume: Option<f64>,
    minimized: bool,
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
    store_tx: Option<Sender<Effect>>,
    playback: Option<Box<Embedded>>,
    connecting_controls: Controls,
    engine: Arc<tokio::sync::Mutex<Option<Engine>>>,
    torrent_generation: Arc<AtomicU64>,
    torrent_task: Option<tokio::task::AbortHandle>,
    seq: u64,
}

impl CineoApp {
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
            Effect::FetchSubtitles { addon, path } => {
                self.spawn(async move {
                    let result = client
                        .fetch_subtitles(&addon, &path)
                        .await
                        .map(|s| s.value)
                        .map_err(|e| e.to_string());
                    Action::SubtitlesLoaded {
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

    fn stop_torrent(&mut self) {
        let generation = self.next_torrent_generation();
        let current = Arc::clone(&self.torrent_generation);
        let engine = Arc::clone(&self.engine);
        self.io.runtime.spawn(async move {
            let engine = engine.lock().await;
            if current.load(Ordering::SeqCst) == generation
                && let Some(engine) = engine.as_ref()
            {
                engine.stop().await;
            }
        });
    }

    fn next_torrent_generation(&mut self) -> u64 {
        if let Some(task) = self.torrent_task.take() {
            task.abort();
        }
        self.torrent_generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn spawn(&self, task: impl Future<Output = Action> + Send + 'static) {
        let tx = self.results_tx.clone();
        let ctx = self.ctx.clone();
        self.io.runtime.spawn(async move {
            let _ = tx.send(Msg::Action(task.await));
            ctx.request_repaint();
        });
    }

    fn play(&mut self, request: &PlayRequest) {
        self.end_playback();
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
            logo: request.logo.as_ref().map(ToString::to_string),
            controls: Controls::default(),
            forward: Some(forward.abort_handle()),
            subtitle_files: SubtitleFiles::new(self.subtitle_dir()),
            auto_subtitle: request.settings.subtitle_languages().collect(),
            volume: None,
            minimized: false,
        })
    }

    fn subtitle_dir(&self) -> PathBuf {
        self.io.engine.cache_dir.join("subtitles")
    }

    fn load_addon_subtitle(&mut self, url: url::Url) {
        let Some(embedded) = &self.playback else {
            return;
        };
        let Some(subtitle) = self.find_subtitle(&url) else {
            return;
        };
        if let Some(path) = embedded.subtitle_files.get(&url) {
            if let Err(err) = embedded.player.add_subtitle(path, &subtitle.0, &subtitle.1) {
                warn!(%err, "could not select the subtitle");
            }
            return;
        }
        let client = Arc::clone(&self.io.client);
        let tx = self.results_tx.clone();
        let ctx = self.ctx.clone();
        self.io.runtime.spawn(async move {
            let result = client.fetch_subtitle(&url).await.map_err(|e| e.to_string());
            let _ = tx.send(Msg::SubtitleFetched { url, result });
            ctx.request_repaint();
        });
    }

    fn subtitle_fetched(&mut self, url: &url::Url, result: Result<Vec<u8>, String>) {
        let Some((title, lang)) = self.find_subtitle(url) else {
            return;
        };
        let Some(embedded) = &mut self.playback else {
            return;
        };
        let loaded = result.and_then(|bytes| {
            let path = embedded
                .subtitle_files
                .write(url, &bytes)
                .map_err(|e| format!("cannot save it: {e}"))?;
            embedded
                .player
                .add_subtitle(&path, &title, &lang)
                .map_err(|e| e.to_string())
        });
        if let Err(err) = loaded {
            warn!(%err, "addon subtitle failed");
            self.state.notice = Some(format!("Could not load the subtitle: {err}"));
        }
    }

    /// The menu title and language of an offered addon subtitle.
    fn find_subtitle(&self, url: &url::Url) -> Option<(String, String)> {
        self.state.subtitles.iter().find_map(|g| {
            g.subtitles
                .ready()?
                .iter()
                .find(|s| s.url == *url)
                .map(|s| {
                    let source = s.label.as_deref().unwrap_or(&g.addon_name);
                    (source.to_owned(), s.lang.clone())
                })
        })
    }

    fn sender(&self) -> impl Fn(Action) + Send + Sync + 'static {
        let tx = self.results_tx.clone();
        let ctx = self.ctx.clone();
        move |action| {
            let _ = tx.send(Msg::Action(action));
            ctx.request_repaint();
        }
    }

    /// Drops the playback, keeping its volume for the next one.
    fn end_playback(&mut self) {
        let volume = self.playback.take().and_then(|p| p.volume);
        if let Some(volume) = volume_to_save(&self.state.settings, volume) {
            self.dispatch(Action::ChangeSetting(Setting::Volume(volume)));
        }
    }

    /// Pauses when the window is minimized, if the user asked for it.
    fn pause_on_minimize(&mut self, ctx: &egui::Context) {
        let Some(embedded) = &mut self.playback else {
            return;
        };
        let minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
        if minimized
            && !embedded.minimized
            && should_pause_on_minimize(&self.state.settings, &embedded.player.status())
        {
            embedded.player.send(PlayerCommand::TogglePause);
        }
        embedded.minimized = minimized;
    }

    fn show_connecting(&mut self, ui: &mut egui::Ui) -> bool {
        let Some((title, logo)) = self
            .state
            .torrent
            .as_ref()
            .and_then(|t| t.connecting())
            .map(|request| (request.title.clone(), request.logo.clone()))
        else {
            return false;
        };
        let mut commands = Vec::new();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::BLACK))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                let playback = Playback {
                    status: &Status::default(),
                    title: &title,
                    logo: logo.as_ref().map(url::Url::as_str),
                    addon_subtitles: &[],
                    settings: &self.state.settings,
                };
                commands = player::show(ui, rect, &playback, &mut self.connecting_controls);
            });
        if commands.contains(&PlayerCommand::Stop) {
            self.connecting_controls = Controls::default();
            player::leave_fullscreen(&self.ctx);
            self.dispatch(Action::PlaybackStopped);
        }
        true
    }

    fn show_player(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(embedded) = &mut self.playback else {
            return false;
        };
        if embedded.player.is_finished() {
            embedded.forward = None;
            self.end_playback();
            player::leave_fullscreen(&self.ctx);
            return false;
        }
        let status = embedded.player.status();
        if status.loaded {
            embedded.volume = Some(status.volume);
        }
        let addon_subtitles = embedded
            .subtitle_files
            .entries(&self.state.subtitles, &status.tracks);
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
                let playback = Playback {
                    status: &status,
                    title: &embedded.title,
                    logo: embedded.logo.as_deref(),
                    addon_subtitles: &addon_subtitles,
                    settings: &self.state.settings,
                };
                commands = player::show(ui, rect, &playback, &mut embedded.controls);
            });
        let mut picked = embedded.controls.take_addon_subtitle();
        if picked.is_some()
            || commands
                .iter()
                .any(|c| matches!(c, PlayerCommand::SetSubtitle(_)))
        {
            embedded.auto_subtitle.clear();
        }
        for command in commands {
            embedded.player.send(command);
        }
        if !embedded.auto_subtitle.is_empty() && status.loaded && !status.tracks.is_empty() {
            match subtitles::auto_pick(&embedded.auto_subtitle, &status.tracks, &addon_subtitles) {
                AutoPick::Done => embedded.auto_subtitle.clear(),
                AutoPick::Load(url) => {
                    embedded.auto_subtitle.clear();
                    picked = Some(url);
                }
                AutoPick::Wait => {}
            }
        }
        if let Some(url) = picked {
            self.load_addon_subtitle(url);
        }
        true
    }
}

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
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.pause_on_minimize(ctx);
        while let Ok(msg) = self.results_rx.try_recv() {
            match msg {
                Msg::Action(action) => self.dispatch(action),
                Msg::Notice(notice) => self.state.notice = Some(notice),
                Msg::SubtitleFetched { url, result } => self.subtitle_fetched(&url, result),
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
        self.end_playback();
        self.store_tx = None;
        self.next_torrent_generation();
        let engine = Arc::clone(&self.engine);
        let shutdown = async move {
            if let Some(engine) = engine.lock().await.take() {
                engine.shutdown().await;
            }
        };
        let _ = self
            .io
            .runtime
            .block_on(async { tokio::time::timeout(Duration::from_secs(2), shutdown).await });
    }
}

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

/// The volume a finished playback leaves for the next one, if it changed.
fn volume_to_save(settings: &Settings, volume: Option<f64>) -> Option<Percent> {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "mpv volume, rounded and clamped by Percent"
    )]
    let volume = Percent::new(volume?.round().max(0.0) as u32);
    (settings.remember_volume && settings.volume != volume).then_some(volume)
}

fn should_pause_on_minimize(settings: &Settings, status: &Status) -> bool {
    settings.pause_on_minimize && status.loaded && !status.paused
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_volume_is_kept_only_when_remembering() {
        let settings = Settings::default();
        assert_eq!(
            volume_to_save(&settings, Some(42.4)),
            Some(Percent::new(42))
        );
        assert_eq!(volume_to_save(&settings, Some(100.0)), None, "unchanged");
        assert_eq!(volume_to_save(&settings, None), None, "never loaded");
        let off = Settings {
            remember_volume: false,
            ..settings
        };
        assert_eq!(volume_to_save(&off, Some(42.0)), None);
    }

    #[test]
    fn minimizing_pauses_only_a_playing_video_when_asked() {
        let on = Settings {
            pause_on_minimize: true,
            ..Settings::default()
        };
        let playing = Status {
            loaded: true,
            ..Status::default()
        };
        assert!(should_pause_on_minimize(&on, &playing));
        assert!(!should_pause_on_minimize(&Settings::default(), &playing));
        let paused = Status {
            paused: true,
            ..playing
        };
        assert!(!should_pause_on_minimize(&on, &paused));
    }
}
