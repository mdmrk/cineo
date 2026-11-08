use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use anyhow::Context as _;
use cineo_core::app::{
    Action, Effect, Language, Notice, Percent, PlayRequest, Setting, Settings, State,
    TorrentRequest, find_subtitle, parse_link, update,
};
use cineo_net::{AddonClient, NetPolicy};
use cineo_player::PlayerError;
use cineo_player::embedded::{
    Display, Player, PlayerCommand, ProcAddress, Renderer, Status, Video,
};
use cineo_player::{PlayerEvent, forward_events};
use cineo_store::Store;
use cineo_stream::{EngineOptions, PlayerFeed, Torrents};
use eframe::egui;
use raw_window_handle::{HasDisplayHandle as _, RawDisplayHandle};
use tracing::{debug, error, warn};

use crate::brand;
use crate::i18n::{self, Locale};
use crate::images::{self, NetImageLoader};
use crate::keep_awake::KeepAwake;
use crate::media_keys::{MediaKeys, Remote};
use crate::player::{self, Controls, NextUp, Playback};
use crate::theme;
use crate::view::{self, Page, ViewState};
use crate::{instance, register};
use cineo_player::subtitles::{self, AutoPick, SubtitleFiles};

#[derive(Debug, Clone)]
pub struct Options {
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub allow_private_network: bool,
    pub link: Option<String>,
}

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
    let client = AddonClient::new(NetPolicy {
        allow_private_networks: allow_private_network,
        ..NetPolicy::default()
    })?;
    let settings = Action::RestoreSettings(saved);
    let (results_tx, results_rx) = channel();
    let (store_tx, writer) =
        spawn_store_writer(store, results_tx.clone()).context("cannot start the store thread")?;
    let io = Io {
        runtime: runtime.handle().clone(),
        client: client.clone(),
        gl: None,
        display: None,
        #[cfg(windows)]
        window: None,
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
            io.gl.clone_from(&cc.get_proc_address);
            io.display = native_display(cc);
            #[cfg(windows)]
            {
                io.window = native_window(cc);
            }
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
    pub(crate) client: AddonClient,
    pub(crate) gl: Option<ProcAddress>,
    pub(crate) display: Option<Display>,
    #[cfg(windows)]
    pub(crate) window: Option<isize>,
    pub(crate) engine: EngineOptions,
}

fn native_display(cc: &eframe::CreationContext<'_>) -> Option<Display> {
    match cc.display_handle().ok()?.as_raw() {
        RawDisplayHandle::Wayland(handle) => Some(Display::Wayland(handle.display)),
        RawDisplayHandle::Xlib(handle) => handle.display.map(Display::X11),
        _ => None,
    }
}

#[cfg(windows)]
fn native_window(cc: &eframe::CreationContext<'_>) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
    match cc.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
        _ => None,
    }
}

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
    torrents: Torrents,
    seq: u64,
    pending_link: Option<String>,
    keep_awake: KeepAwake,
    media_keys: MediaKeys,
}

impl CineoApp {
    pub(crate) fn new(
        ctx: egui::Context,
        io: Io,
        restore: [Action; 2],
        store_tx: Sender<Effect>,
        (results_tx, results_rx): (Sender<Msg>, Receiver<Msg>),
    ) -> Self {
        #[cfg(not(windows))]
        let media_keys = MediaKeys::new(ctx.clone());
        #[cfg(windows)]
        let media_keys = MediaKeys::new(ctx.clone(), io.window);
        let torrents = Torrents::new(io.runtime.clone(), io.engine.clone());
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
            torrents,
            seq: 0,
            pending_link: None,
            keep_awake: KeepAwake::new(),
            media_keys,
        };
        for action in restore {
            app.dispatch(action);
        }
        app.view.page = app.state.settings.start_page.into();
        if app.view.page == Page::Addons {
            app.dispatch(Action::LoadAddonLists);
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
        match effect {
            effect @ (Effect::FetchManifest { .. }
            | Effect::FetchCatalog { .. }
            | Effect::FetchAddonList { .. }
            | Effect::FetchMeta { .. }
            | Effect::FetchStreams { .. }
            | Effect::FetchSubtitles { .. }
            | Effect::CheckForUpdate(_)) => {
                if let Some(task) = cineo_net::fetch_task(&self.io.client, effect) {
                    self.spawn(task);
                }
            }
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
        let send = self.sender();
        self.torrents.start(request, &self.state.settings, send);
    }

    fn stop_torrent(&mut self) {
        self.torrents.stop();
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
            display: self.io.display,
        };
        let (player, renderer, events) =
            Player::start(request, Arc::new(move || repaint.request_repaint()), video)?;
        let (meta_id, video_id) = (request.meta_id.clone(), request.video_id.clone());
        let send = self.sender();
        let feed = self.torrents.feed();
        feed.playing(&request.url);
        let forward = self.io.runtime.spawn(forward_events(
            events,
            meta_id,
            video_id,
            move |event| feed_player(&feed, event),
            send,
        ));
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

    fn load_addon_subtitle(&self, url: url::Url) {
        let Some(embedded) = &self.playback else {
            return;
        };
        let Some(subtitle) = find_subtitle(&self.state.subtitles, &url) else {
            return;
        };
        if let Some(path) = embedded.subtitle_files.get(&url) {
            if let Err(err) = embedded.player.add_subtitle(path, &subtitle.0, &subtitle.1) {
                warn!(%err, "could not select the subtitle");
            }
            return;
        }
        let client = self.io.client.clone();
        let post = self.post();
        self.io.runtime.spawn(async move {
            let result = client.fetch_subtitle(&url).await.map_err(|e| e.to_string());
            post(Msg::SubtitleFetched { url, result });
        });
    }

    fn subtitle_fetched(&mut self, url: &url::Url, result: Result<Vec<u8>, String>) {
        let Some((title, lang)) = find_subtitle(&self.state.subtitles, url) else {
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

    fn remote(&self, remote: Remote) {
        if remote == Remote::Raise {
            self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            return;
        }
        let Some(embedded) = &self.playback else {
            return;
        };
        if let Some(command) = remote_command(remote, embedded.player.status().paused) {
            embedded.player.send(command);
        }
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

impl eframe::App for CineoApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.pause_on_minimize(ctx);
        for remote in self.media_keys.take() {
            self.remote(remote);
        }
        let status = self.playback.as_ref().map(|e| e.player.status());
        self.keep_awake
            .set(status.as_ref().is_some_and(keeps_screen_awake));
        let shown = self
            .playback
            .as_ref()
            .zip(status.filter(|s| s.loaded))
            .map(|(e, s)| (e.title.as_str(), s.paused));
        self.media_keys.set(shown);
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
        self.torrents.shutdown(Duration::from_secs(2));
    }
}

fn private_networks_allowed(command_line: bool, settings: &Settings) -> bool {
    command_line || settings.allow_private_network
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

fn remote_command(remote: Remote, paused: bool) -> Option<PlayerCommand> {
    match remote {
        Remote::PlayPause => Some(PlayerCommand::TogglePause),
        Remote::Play if paused => Some(PlayerCommand::TogglePause),
        Remote::Pause if !paused => Some(PlayerCommand::TogglePause),
        Remote::Stop => Some(PlayerCommand::Stop),
        Remote::Play | Remote::Pause | Remote::Raise => None,
    }
}

fn keeps_screen_awake(status: &Status) -> bool {
    status.loaded && !status.paused
}

fn feed_player(feed: &PlayerFeed, event: &PlayerEvent) {
    match event {
        PlayerEvent::Buffered { seconds } => feed.buffered(*seconds),
        PlayerEvent::Progress { duration_ms, .. }
        | PlayerEvent::Ended { duration_ms, .. }
        | PlayerEvent::Closed { duration_ms, .. } => feed.duration(*duration_ms),
        PlayerEvent::Failed(_) => {}
    }
}

fn should_pause_on_minimize(settings: &Settings, status: &Status) -> bool {
    settings.pause_on_minimize && status.loaded && !status.paused
}

pub(crate) fn spawn_store_writer(
    store: Store,
    results: Sender<Msg>,
) -> std::io::Result<(Sender<Effect>, std::thread::JoinHandle<()>)> {
    cineo_store::spawn_writer(store, move |err| {
        let _ = results.send(Msg::Notice(Notice::SaveFailed(err.to_string())));
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn only_a_playing_video_keeps_the_screen_awake() {
        let playing = Status {
            loaded: true,
            ..Status::default()
        };
        assert!(keeps_screen_awake(&playing));
        let paused = Status {
            paused: true,
            ..playing
        };
        assert!(!keeps_screen_awake(&paused));
        assert!(!keeps_screen_awake(&Status::default()), "not loaded yet");
    }

    #[test]
    fn media_keys_become_player_commands() {
        let toggle = Some(PlayerCommand::TogglePause);
        for paused in [false, true] {
            assert_eq!(remote_command(Remote::PlayPause, paused), toggle);
            assert_eq!(
                remote_command(Remote::Stop, paused),
                Some(PlayerCommand::Stop)
            );
            assert_eq!(remote_command(Remote::Raise, paused), None);
        }
        assert_eq!(remote_command(Remote::Play, true), toggle);
        assert_eq!(remote_command(Remote::Play, false), None, "already playing");
        assert_eq!(remote_command(Remote::Pause, false), toggle);
        assert_eq!(remote_command(Remote::Pause, true), None, "already paused");
    }
}
