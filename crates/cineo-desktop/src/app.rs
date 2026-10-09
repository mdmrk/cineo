use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context as _;
use cineo_core::app::{
    Action, Effect, Language, Notice, Percent, PlayRequest, Setting, Settings, State,
    TorrentRequest, parse_link, update,
};
use cineo_core::diagnostics::Parsed;
use cineo_net::{AddonClient, FetchError, NetPolicy};
use cineo_player::PlayerError;
use cineo_player::PlayerEvent;
use cineo_player::embedded::{Player, PlayerCommand, ProcAddress, Renderer, Status, Video};
use cineo_store::Store;
use cineo_stream::{Engine, EngineOptions};
use eframe::egui;
use tracing::{debug, error, warn};

use crate::brand;
use crate::i18n::{self, Locale};
use crate::images::{self, NetImageLoader};
use crate::player::{self, Controls, NextUp, Playback};
use crate::subtitles::{self, AutoPick, SubtitleFiles};
use crate::theme;
use crate::view::{self, ViewState};
use crate::{instance, register};

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
    pub link: Option<String>,
}

/// Opens the store, restores state and runs the window until it is closed.
pub fn run(options: Options) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("cannot start the async runtime")?;
    let store = Store::open_in(&options.data_dir)
        .with_context(|| format!("cannot open the database in {}", options.data_dir.display()))?;
    let restore = Action::Restore {
        addons: store.addons()?,
        library: store.library()?,
    };
    let saved = store.settings()?;
    let allow_private_network = private_networks_allowed(options.allow_private_network, &saved);
    let client = Arc::new(AddonClient::new(NetPolicy {
        allow_private_networks: allow_private_network,
        ..NetPolicy::default()
    })?);
    let settings = Action::RestoreSettings(saved);
    let (results_tx, results_rx) = channel();
    let (store_tx, writer) =
        spawn_store_writer(store, results_tx.clone()).context("cannot start the store thread")?;
    let io = Io {
        runtime: runtime.handle().clone(),
        client: Arc::clone(&client),
        gl: None,
        engine: EngineOptions {
            allow_private_network,
            ..EngineOptions::new(options.cache_dir)
        },
    };
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Cineo")
        .with_app_id("cineo")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([800.0, 500.0]);
    if let Some(icon) = brand::icon() {
        viewport = viewport.with_icon(icon);
    }
    let native = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    let handle = runtime.handle().clone();
    let (data_dir, link) = (options.data_dir, options.link);
    let result = eframe::run_native(
        "Cineo",
        native,
        Box::new(move |cc| {
            theme::apply(&cc.egui_ctx);
            images::install(&cc.egui_ctx, NetImageLoader::new(client, handle));
            let mut io = io;
            io.gl = cc.get_proc_address.clone();
            let (tx, ctx) = (results_tx.clone(), cc.egui_ctx.clone());
            if let Err(err) = instance::listen(&data_dir, &io.runtime, move |link| {
                let _ = tx.send(Msg::Link(link));
                ctx.request_repaint();
            }) {
                warn!(%err, "links from other launches will not reach this window");
            }
            let mut app = CineoApp::new(
                cc.egui_ctx.clone(),
                io,
                [restore, settings],
                store_tx,
                (results_tx, results_rx),
            );
            app.pending_link = link;
            Ok(Box::new(app))
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
    Notice(Notice),
    SubtitleFetched {
        url: url::Url,
        result: Result<Vec<u8>, String>,
    },
    Link(String),
    LinksRegistered(Result<(), String>),
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
    background: Option<String>,
    controls: Controls,
    forward: Option<tokio::task::AbortHandle>,
    subtitle_files: SubtitleFiles,
    auto_subtitle: Vec<Language>,
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
    pending_link: Option<String>,
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
            view: ViewState {
                system_locale: Locale::system(),
                ..ViewState::default()
            },
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
            pending_link: None,
        };
        for action in restore {
            app.dispatch(action);
        }
        app.view.page = app.state.settings.start_page.into();
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
            Effect::FetchManifest { transport, install } => self.spawn(async move {
                let result = fetched(client.fetch_manifest(&transport).await).map(Box::new);
                Action::ManifestLoaded {
                    transport,
                    result,
                    install,
                }
            }),
            Effect::FetchCatalog { addon, path } => self.spawn(async move {
                let result = fetched(client.fetch_catalog(&addon, &path).await).map(|c| c.metas);
                Action::CatalogLoaded {
                    addon,
                    path,
                    result,
                }
            }),
            Effect::FetchMeta { addon, path } => self.spawn(async move {
                let result = fetched(client.fetch_meta(&addon, &path).await).map(Box::new);
                Action::MetaLoaded {
                    addon,
                    path,
                    result,
                }
            }),
            Effect::FetchStreams { addon, path } => self.spawn(async move {
                let result = fetched(client.fetch_streams(&addon, &path).await);
                Action::StreamsLoaded {
                    addon,
                    path,
                    result,
                }
            }),
            Effect::FetchSubtitles { addon, path } => self.spawn(async move {
                let result = fetched(client.fetch_subtitles(&addon, &path).await);
                Action::SubtitlesLoaded {
                    addon,
                    path,
                    result,
                }
            }),
            effect @ (Effect::SaveAddons(_)
            | Effect::SaveLibraryItem(_)
            | Effect::DeleteLibraryItem(_)
            | Effect::ClearLibrary
            | Effect::SaveSettings(_)) => {
                let sent = self
                    .store_tx
                    .as_ref()
                    .is_some_and(|tx| tx.send(effect).is_ok());
                if !sent {
                    self.state.notice = Some(Notice::StoreClosed);
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
        let options = engine_options(&self.io.engine, &self.state.settings);
        let send = self.sender();
        let task = self.io.runtime.spawn(async move {
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
            let mut last = None;
            loop {
                ticks.tick().await;
                if current.load(Ordering::SeqCst) != generation {
                    return;
                }
                let status = engine.lock().await.as_ref().and_then(Engine::status);
                match status {
                    Some((hash, status)) if hash == info_hash => {
                        if last.as_ref() != Some(&status) {
                            last = Some(status.clone());
                            send(Action::TorrentStatus {
                                info_hash: info_hash.clone(),
                                status,
                            });
                        }
                    }
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
        let send = self.sender();
        self.io.runtime.spawn(async move { send(task.await) });
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
            background: request.background.as_ref().map(ToString::to_string),
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
        let post = self.post();
        self.io.runtime.spawn(async move {
            let result = client.fetch_subtitle(&url).await.map_err(|e| e.to_string());
            post(Msg::SubtitleFetched { url, result });
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
            self.state.notice = Some(Notice::SubtitleFailed(err));
        }
    }

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

    fn post(&self) -> impl Fn(Msg) + Send + Sync + 'static {
        let tx = self.results_tx.clone();
        let ctx = self.ctx.clone();
        move |msg| {
            let _ = tx.send(msg);
            ctx.request_repaint();
        }
    }

    fn sender(&self) -> impl Fn(Action) + Send + Sync + 'static {
        let post = self.post();
        move |action| post(Msg::Action(action))
    }

    fn open_link(&mut self, link: &str) {
        match parse_link(link) {
            Ok(route) => {
                for action in view::follow_link(route, &self.state, &mut self.view) {
                    self.dispatch(action);
                }
            }
            Err(err) => {
                warn!(%err, "ignored a link");
                self.state.notice = Some(Notice::InvalidLink);
            }
        }
    }

    fn end_playback(&mut self) {
        let volume = self.playback.take().and_then(|p| p.volume);
        if let Some(volume) = volume_to_save(&self.state.settings, volume) {
            self.dispatch(Action::ChangeSetting(Setting::Volume(volume)));
        }
    }

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
        let Some(torrent) = &self.state.torrent else {
            return false;
        };
        let Some(request) = torrent.connecting() else {
            return false;
        };
        let mut commands = Vec::new();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::BLACK))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                let playback = Playback {
                    status: &Status::default(),
                    title: &request.title,
                    logo: request.logo.as_ref().map(url::Url::as_str),
                    background: request.background.as_ref().map(url::Url::as_str),
                    addon_subtitles: &[],
                    settings: &self.state.settings,
                    next: None,
                    torrent: Some(&torrent.status),
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
                    background: embedded.background.as_deref(),
                    addon_subtitles,
                    settings: &self.state.settings,
                    next: self
                        .state
                        .playing
                        .as_ref()
                        .and_then(|p| p.next.as_ref())
                        .map(|n| NextUp {
                            title: &n.title,
                            image: n.image.as_ref().map(url::Url::as_str),
                        }),
                    torrent: self.state.torrent.as_ref().map(|t| &t.status),
                };
                commands = player::show(ui, rect, &playback, &mut embedded.controls);
            });
        let play_next = embedded.controls.take_next();
        let changes = embedded.controls.take_settings();
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
            match subtitles::auto_pick(&embedded.auto_subtitle, &status.tracks, addon_subtitles) {
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
        if !changes.is_empty() {
            for action in changes {
                self.dispatch(action);
            }
            if let Some(embedded) = &self.playback {
                embedded.player.set_subtitle_style(&self.state.settings);
            }
        }
        if play_next {
            self.end_playback();
            self.dispatch(Action::PlayNext);
            if self.playback.is_none() && self.state.playing.is_none() {
                player::leave_fullscreen(&self.ctx);
            }
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
            } => (time_ms, duration_ms, None),
            PlayerEvent::Ended {
                time_ms,
                duration_ms,
            } => (time_ms, duration_ms, Some(Action::PlaybackEnded)),
            PlayerEvent::Closed {
                time_ms,
                duration_ms,
            } => (time_ms, duration_ms, Some(Action::PlaybackStopped)),
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
        if let Some(last) = last {
            send(last);
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
                Msg::Link(link) => {
                    self.pending_link = Some(link);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                Msg::LinksRegistered(result) => self.view.links_registered = Some(result),
            }
        }
        if self.state.addons_loading.is_empty()
            && let Some(link) = self.pending_link.take()
        {
            self.open_link(&link);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        i18n::set(Locale::resolve(
            self.state.settings.ui_language,
            self.view.system_locale,
        ));
        if self.show_player(ui) || self.show_connecting(ui) {
            return;
        }
        for action in view::show(ui, &self.state, &mut self.view) {
            self.dispatch(action);
        }
        if std::mem::take(&mut self.view.register_links) {
            let post = self.post();
            self.io.runtime.spawn_blocking(move || {
                post(Msg::LinksRegistered(
                    register::register().map_err(|err| format!("{err:#}")),
                ));
            });
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
    if let Some(stale) = slot.take_if(|engine| *engine.options() != options) {
        stale.shutdown().await;
    }
    let engine = match slot {
        Some(engine) => engine,
        None => slot.insert(Engine::start(options).await.map_err(|e| e.to_string())?),
    };
    engine.open(request).await.map_err(|err| {
        error!(error = %err, "opening the torrent failed");
        err.to_string()
    })
}

fn fetched<T>(result: Result<Parsed<T>, FetchError>) -> Result<T, String> {
    result
        .map(|parsed| parsed.value)
        .map_err(|err| err.to_string())
}

fn private_networks_allowed(command_line: bool, settings: &Settings) -> bool {
    command_line || settings.allow_private_network
}

fn engine_options(base: &EngineOptions, settings: &Settings) -> EngineOptions {
    EngineOptions {
        dht: base.dht && settings.torrent_dht,
        upload: settings.torrent_upload,
        download_limit_bps: settings
            .download_limit
            .bytes_per_second()
            .and_then(NonZeroU32::new),
        upload_limit_bps: settings
            .upload_limit
            .bytes_per_second()
            .and_then(NonZeroU32::new),
        peer_limit: Some(settings.peer_limit.peers()),
        ..base.clone()
    }
}

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
            while let Ok(first) = rx.recv() {
                for effect in coalesce(std::iter::once(first).chain(rx.try_iter()).collect()) {
                    if let Err(err) = store.apply(&effect) {
                        error!(error = %err, "saving failed");
                        let _ = results.send(Msg::Notice(Notice::SaveFailed(err.to_string())));
                    }
                }
            }
        })?;
    Ok((tx, thread))
}

fn coalesce(batch: Vec<Effect>) -> impl Iterator<Item = Effect> {
    let keep: Vec<bool> = (0..batch.len())
        .map(|i| {
            !batch[i + 1..]
                .iter()
                .any(|later| supersedes(later, &batch[i]))
        })
        .collect();
    batch
        .into_iter()
        .zip(keep)
        .filter_map(|(effect, keep)| keep.then_some(effect))
}

fn supersedes(later: &Effect, earlier: &Effect) -> bool {
    match (later, earlier) {
        (Effect::SaveSettings(_), Effect::SaveSettings(_))
        | (Effect::SaveAddons(_), Effect::SaveAddons(_)) => true,
        (Effect::SaveLibraryItem(later), Effect::SaveLibraryItem(earlier)) => {
            later.id == earlier.id
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cineo_core::app::{DownloadLimit, PeerLimit, UploadLimit};

    #[test]
    fn queued_saves_keep_only_the_last_write_of_each_key() {
        let item = |id: &str, time_offset_ms| {
            Effect::SaveLibraryItem(cineo_core::app::LibraryItem {
                id: id.into(),
                content_type: cineo_core::addon::ContentType::new("movie").unwrap_or_else(|| {
                    panic!("valid type");
                }),
                name: id.into(),
                poster: None,
                video_id: id.into(),
                time_offset_ms,
                duration_ms: 0,
                updated_ms: 1,
                favorited: None,
                stream: None,
            })
        };
        let quiet = Settings {
            volume: Percent::new(10),
            ..Settings::default()
        };
        let batch = vec![
            Effect::SaveSettings(Settings::default()),
            item("a", 1),
            item("b", 1),
            Effect::DeleteLibraryItem("a".into()),
            item("a", 2),
            Effect::SaveSettings(quiet),
            Effect::ClearLibrary,
        ];
        let kept: Vec<Effect> = coalesce(batch).collect();
        assert_eq!(
            kept,
            [
                item("b", 1),
                Effect::DeleteLibraryItem("a".into()),
                item("a", 2),
                Effect::SaveSettings(quiet),
                Effect::ClearLibrary,
            ]
        );
    }

    #[test]
    fn private_networks_stay_blocked_unless_asked_for() {
        assert!(!private_networks_allowed(false, &Settings::default()));
        assert!(private_networks_allowed(true, &Settings::default()));
        let on = Settings {
            allow_private_network: true,
            ..Settings::default()
        };
        assert!(private_networks_allowed(false, &on));
    }

    #[test]
    fn torrent_settings_shape_the_next_engine() {
        let base = EngineOptions::new(PathBuf::from("/c"));
        let defaults = engine_options(&base, &Settings::default());
        assert_eq!(
            defaults,
            EngineOptions {
                peer_limit: Some(128),
                ..base.clone()
            },
            "the defaults match librqbit's"
        );
        let settings = Settings {
            torrent_upload: false,
            torrent_dht: false,
            download_limit: DownloadLimit::M5,
            upload_limit: UploadLimit::K100,
            peer_limit: PeerLimit::P50,
            ..Settings::default()
        };
        let options = engine_options(&base, &settings);
        assert!(!options.upload && !options.dht);
        assert_eq!(options.download_limit_bps, NonZeroU32::new(5_000_000));
        assert_eq!(options.upload_limit_bps, NonZeroU32::new(100_000));
        assert_eq!(options.peer_limit, Some(50));
    }

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
