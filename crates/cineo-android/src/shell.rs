use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use cineo_core::addon::{ContentType, TransportUrl};
use cineo_core::app::{
    Action, Effect, Language, LibraryItem, Notice, PlayRequest, Resolution, SeekStep, Setting,
    State, StreamSort, SubtitleGroup, WatchedAt, find_subtitle, update,
};
use cineo_net::{AddonClient, MAX_IMAGE_BYTES, NetPolicy, fetch_task};
use cineo_player::embedded::{PlayerCommand, Status, TrackKind};
use cineo_player::subtitles::{AddonSubtitle, AutoPick, SubtitleFiles, auto_pick};
use cineo_player::{PlayerEvent, forward_events, plain_text};
use cineo_store::Store;
use cineo_stream::{EngineOptions, Torrents};
use tokio::runtime::{Handle, Runtime};
use tracing::{error, warn};
use url::Url;

use crate::player::{Mpv, Playback, Players};
use crate::view::{AppView, ArtView, StreamFilterView, view};

const ENGINE_SHUTDOWN: Duration = Duration::from_secs(2);

#[uniffi::export(foreign)]
pub trait StateListener: Send + Sync + std::fmt::Debug {
    fn changed(&self, view: AppView);
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum StartError {
    #[error("cannot start the async runtime: {reason}")]
    Runtime { reason: String },
    #[error("cannot open the database: {reason}")]
    Store { reason: String },
    #[error("cannot create the HTTP client: {reason}")]
    Http { reason: String },
    #[error("cannot start a thread: {reason}")]
    Thread { reason: String },
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum ImageError {
    #[error("invalid image URL: {reason}")]
    InvalidUrl { reason: String },
    #[error("image request failed: {reason}")]
    Fetch { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum SettingChange {
    HardwareDecoding { on: bool },
    SeekStep { seconds: u32 },
    AudioLanguage { code: Option<String> },
    SubtitleLanguage { code: Option<String> },
    WatchedAt { percent: u32 },
    AllowPrivateNetwork { on: bool },
    P2pEnabled { on: bool },
}

impl SettingChange {
    fn setting(self) -> Option<Setting> {
        let language = |code: Option<String>| match code {
            Some(code) => Language::from_code(&code).map(Some),
            None => Some(None),
        };
        Some(match self {
            Self::HardwareDecoding { on } => Setting::HardwareDecoding(on),
            Self::SeekStep { seconds } => Setting::SeekStep(SeekStep::from_seconds(seconds)?),
            Self::AudioLanguage { code } => Setting::AudioLanguage(language(code)?),
            Self::SubtitleLanguage { code } => Setting::SubtitleLanguage(language(code)?),
            Self::WatchedAt { percent } => Setting::WatchedAt(WatchedAt::from_percent(percent)?),
            Self::AllowPrivateNetwork { on } => Setting::AllowPrivateNetwork(on),
            Self::P2pEnabled { on } => Setting::P2pEnabled(on),
        })
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PlayerStatusView {
    pub loaded: bool,
    pub position_s: f64,
    pub duration_s: f64,
    pub paused: bool,
    pub waiting: bool,
    pub stats: PlayerStatsView,
    pub audio: Vec<TrackView>,
    pub subtitles: Vec<TrackView>,
    pub subtitles_off: bool,
    pub addon_subtitles: Vec<AddonSubtitleView>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TrackView {
    pub id: i64,
    pub number: u32,
    pub label: Option<String>,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AddonSubtitleView {
    pub url: String,
    pub label: String,
    pub selected: bool,
}

fn track_views(status: &Status, kind: TrackKind) -> Vec<TrackView> {
    status
        .tracks
        .iter()
        .filter(|t| t.kind == kind && t.external_file.is_none())
        .zip(1..)
        .map(|(track, number)| {
            let parts: Vec<&str> = [track.lang.as_deref(), track.title.as_deref()]
                .into_iter()
                .flatten()
                .collect();
            TrackView {
                id: track.id,
                number,
                label: (!parts.is_empty()).then(|| plain_text(&parts.join(" · "))),
                selected: track.selected,
            }
        })
        .collect()
}

fn addon_subtitle_view(entry: &AddonSubtitle) -> AddonSubtitleView {
    let source = entry.label.as_deref().unwrap_or(&entry.addon_name);
    AddonSubtitleView {
        url: entry.url.to_string(),
        label: plain_text(&format!("{} · {source}", entry.lang)),
        selected: entry.selected,
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PlayerStatsView {
    pub network_bytes_per_sec: Option<f64>,
    pub buffered_s: Option<f64>,
    pub resolution: Option<VideoSizeView>,
    pub video_codec: Option<String>,
    pub video_bits_per_sec: Option<f64>,
    pub decoder: Option<DecoderView>,
    pub dropped_frames: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct VideoSizeView {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum DecoderView {
    Software,
    Hardware { name: String },
}

impl PlayerStatsView {
    fn of(status: &Status) -> Self {
        let stats = &status.stats;
        let whole = |value: f64| {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a finite, non-negative count that mpv reports as a double"
            )]
            (value.is_finite() && value >= 0.0).then_some(value.round() as u64)
        };
        let size = |value: Option<f64>| value.and_then(whole).and_then(|v| u32::try_from(v).ok());
        Self {
            network_bytes_per_sec: stats.cache_speed,
            buffered_s: stats.cache_s,
            resolution: size(stats.width)
                .zip(size(stats.height))
                .map(|(width, height)| VideoSizeView { width, height }),
            video_codec: status
                .tracks
                .iter()
                .find(|t| t.kind == TrackKind::Video && t.selected)
                .and_then(|t| t.codec.as_deref())
                .map(plain_text),
            video_bits_per_sec: stats.video_bitrate.filter(|b| *b > 0.0),
            decoder: stats.hwdec.as_deref().map(|hwdec| match hwdec {
                "no" => DecoderView::Software,
                name => DecoderView::Hardware {
                    name: plain_text(name),
                },
            }),
            dropped_frames: stats.dropped_frames.and_then(whole),
        }
    }
}

#[derive(Debug)]
enum FilterChange {
    Query(String),
    Quality(Option<Resolution>),
    Addon(Option<u32>),
    Sort(StreamSort),
}

#[derive(Debug)]
enum Msg {
    Action(Action),
    Filter(FilterChange),
    OpenDetail {
        content_type: String,
        id: String,
    },
    Notice(Notice),
    LoadSubtitle(Url),
    SubtitleFetched {
        url: Url,
        result: Result<Vec<u8>, String>,
    },
    Shutdown,
}

#[derive(Debug, Default)]
struct Screen {
    playback: Option<Box<dyn Playback>>,
    subtitles: Option<Subtitles>,
    wid: Option<i64>,
    art: Option<ArtView>,
}

#[derive(Debug)]
struct Subtitles {
    files: SubtitleFiles,
    groups: Vec<SubtitleGroup>,
    auto: Vec<Language>,
}

fn lock(screen: &Mutex<Screen>) -> MutexGuard<'_, Screen> {
    screen.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Debug, uniffi::Object)]
pub struct Cineo {
    tx: Sender<Msg>,
    client: AddonClient,
    screen: Arc<Mutex<Screen>>,
    shell: Option<JoinHandle<()>>,
    runtime: Option<Runtime>,
}

#[uniffi::export]
impl Cineo {
    #[uniffi::constructor]
    pub fn new(
        data_dir: &str,
        cache_dir: &str,
        listener: Arc<dyn StateListener>,
    ) -> Result<Arc<Self>, StartError> {
        let cache_dir = PathBuf::from(cache_dir);
        let engine = EngineOptions::new(cache_dir.join("torrents"));
        Self::start(
            data_dir,
            cache_dir.join("subtitles"),
            engine,
            listener,
            Arc::new(Mpv),
        )
    }

    pub fn accept_p2p(&self) {
        self.send(Action::AcceptP2p);
    }

    pub fn decline_p2p(&self) {
        self.send(Action::DeclineP2p);
    }

    pub fn install_addon(&self, url: String) {
        self.send(Action::InstallAddon(url));
    }

    pub fn dismiss_notice(&self) {
        self.send(Action::DismissNotice);
    }

    pub fn set_stream_query(&self, query: String) {
        self.filter(FilterChange::Query(query));
    }

    pub fn set_stream_quality(&self, quality: Option<Resolution>) {
        self.filter(FilterChange::Quality(quality));
    }

    pub fn set_stream_addon(&self, addon: Option<u32>) {
        self.filter(FilterChange::Addon(addon));
    }

    pub fn set_stream_sort(&self, sort: StreamSort) {
        self.filter(FilterChange::Sort(sort));
    }

    pub fn load_addon_lists(&self) {
        self.send(Action::LoadAddonLists);
    }

    pub fn search(&self, query: String) {
        self.send(Action::Search(query));
    }

    pub fn open_detail(&self, content_type: String, id: String) {
        if self.tx.send(Msg::OpenDetail { content_type, id }).is_err() {
            warn!("the shell thread has stopped");
        }
    }

    pub fn close_detail(&self) {
        self.send(Action::CloseDetail);
    }

    pub fn select_video(&self, id: String) {
        self.send(Action::SelectVideo(id));
    }

    pub fn resume(&self, id: String) {
        self.send(Action::Resume(id));
    }

    pub fn set_favorite(&self, id: String, favorite: bool) {
        self.send(Action::SetFavorite { id, favorite });
    }

    pub fn remove_from_library(&self, id: String) {
        self.send(Action::RemoveFromLibrary(id));
    }

    pub fn change_setting(&self, change: SettingChange) {
        match change.setting() {
            Some(setting) => self.send(Action::ChangeSetting(setting)),
            None => warn!("not a valid setting"),
        }
    }

    pub fn reset_settings(&self) {
        self.send(Action::ResetSettings);
    }

    pub fn clear_library(&self) {
        self.send(Action::ClearLibrary);
    }

    pub fn remove_addon(&self, transport: &str) {
        match TransportUrl::parse(transport) {
            Ok(transport) => self.send(Action::RemoveAddon(transport)),
            Err(err) => warn!(%err, "not an addon URL"),
        }
    }

    pub fn move_addon(&self, from: u32, to: u32) {
        self.send(Action::MoveAddon {
            from: from as usize,
            to: to as usize,
        });
    }

    pub fn play(&self, group: u32, stream: u32) {
        self.send(Action::Play {
            group: group as usize,
            stream: stream as usize,
        });
    }

    pub fn stop_playback(&self) {
        if lock(&self.screen).playback.is_some() {
            self.command(PlayerCommand::Stop);
        } else {
            self.send(Action::PlaybackStopped);
        }
    }

    pub fn toggle_pause(&self) {
        self.command(PlayerCommand::TogglePause);
    }

    pub fn seek_to(&self, seconds: f64) {
        self.command(PlayerCommand::SeekTo(seconds));
    }

    pub fn seek_by(&self, seconds: f64) {
        self.command(PlayerCommand::SeekBy(seconds));
    }

    pub fn player_status(&self) -> Option<PlayerStatusView> {
        let mut screen = lock(&self.screen);
        let screen = &mut *screen;
        let status = screen.playback.as_ref()?.status();
        let addon_subtitles = match &mut screen.subtitles {
            Some(subtitles) => {
                let entries = subtitles.files.entries(&subtitles.groups, &status.tracks);
                if !subtitles.auto.is_empty() && status.loaded && !status.tracks.is_empty() {
                    match auto_pick(&subtitles.auto, &status.tracks, entries) {
                        AutoPick::Done => subtitles.auto.clear(),
                        AutoPick::Load(url) => {
                            subtitles.auto.clear();
                            let _ = self.tx.send(Msg::LoadSubtitle(url));
                        }
                        AutoPick::Wait => {}
                    }
                }
                entries.iter().map(addon_subtitle_view).collect()
            }
            None => Vec::new(),
        };
        Some(PlayerStatusView {
            loaded: status.loaded,
            position_s: status.position_s,
            duration_s: status.duration_s,
            paused: status.paused,
            waiting: status.is_waiting(),
            stats: PlayerStatsView::of(&status),
            audio: track_views(&status, TrackKind::Audio),
            subtitles: track_views(&status, TrackKind::Subtitle),
            subtitles_off: !status
                .tracks
                .iter()
                .any(|t| t.kind == TrackKind::Subtitle && t.selected),
            addon_subtitles,
        })
    }

    pub fn select_audio(&self, id: Option<i64>) {
        self.command(PlayerCommand::SetAudio(id));
    }

    pub fn select_subtitle(&self, id: Option<i64>) {
        self.stop_auto_subtitle();
        self.command(PlayerCommand::SetSubtitle(id));
    }

    pub fn select_addon_subtitle(&self, url: &str) {
        let Ok(url) = Url::parse(url) else {
            warn!("not a subtitle URL");
            return;
        };
        self.stop_auto_subtitle();
        if self.tx.send(Msg::LoadSubtitle(url)).is_err() {
            warn!("the shell thread has stopped");
        }
    }

    pub fn attach_surface(&self, wid: i64) {
        let mut screen = lock(&self.screen);
        screen.wid = Some(wid);
        if let Some(playback) = &screen.playback
            && let Err(err) = playback.attach_surface(wid)
        {
            warn!(%err, "the video surface was not attached");
        }
    }

    pub fn resize_surface(&self, width: u32, height: u32) {
        if let Some(playback) = &lock(&self.screen).playback
            && let Err(err) = playback.resize_surface(width, height)
        {
            warn!(%err, "the video surface size was not set");
        }
    }

    pub fn detach_surface(&self) {
        let mut screen = lock(&self.screen);
        screen.wid = None;
        if let Some(playback) = &screen.playback
            && let Err(err) = playback.detach_surface()
        {
            warn!(%err, "the video surface was not detached");
        }
    }

    pub async fn fetch_image(&self, url: String) -> Result<Vec<u8>, ImageError> {
        let url = Url::parse(&url).map_err(|err| ImageError::InvalidUrl {
            reason: err.to_string(),
        })?;
        let runtime = self.runtime.as_ref().ok_or_else(|| ImageError::Fetch {
            reason: "the runtime has stopped".into(),
        })?;
        let client = self.client.clone();
        let task = runtime
            .handle()
            .spawn(async move { client.fetch_image(&url, MAX_IMAGE_BYTES).await });
        match task.await {
            Ok(result) => result.map_err(|err| ImageError::Fetch {
                reason: err.to_string(),
            }),
            Err(err) => Err(ImageError::Fetch {
                reason: err.to_string(),
            }),
        }
    }
}

impl Cineo {
    pub fn start(
        data_dir: &str,
        subtitle_dir: PathBuf,
        engine: EngineOptions,
        listener: Arc<dyn StateListener>,
        players: Arc<dyn Players>,
    ) -> Result<Arc<Self>, StartError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|err| StartError::Runtime {
                reason: err.to_string(),
            })?;
        let store_error = |err: cineo_store::StoreError| StartError::Store {
            reason: err.to_string(),
        };
        let store = Store::open_in(Path::new(data_dir)).map_err(store_error)?;
        let restore = Action::Restore {
            addons: store.addons().map_err(store_error)?,
            library: store.library().map_err(store_error)?,
        };
        let settings = store.settings().map_err(store_error)?;
        let client = AddonClient::new(NetPolicy {
            allow_private_networks: settings.allow_private_network,
            ..NetPolicy::default()
        })
        .map_err(|err| StartError::Http {
            reason: err.to_string(),
        })?;
        let thread_error = |err: std::io::Error| StartError::Thread {
            reason: err.to_string(),
        };
        let (tx, rx) = channel();
        let notices = tx.clone();
        let (store_tx, writer) = cineo_store::spawn_writer(store, move |err| {
            let _ = notices.send(Msg::Notice(Notice::SaveFailed(err.to_string())));
        })
        .map_err(thread_error)?;
        let screen = Arc::new(Mutex::new(Screen::default()));
        let torrents = Torrents::new(
            runtime.handle().clone(),
            EngineOptions {
                allow_private_network: settings.allow_private_network,
                ..engine
            },
        );
        let shell = Shell {
            state: State::default(),
            client: client.clone(),
            runtime: runtime.handle().clone(),
            tx: tx.clone(),
            store_tx,
            listener,
            players,
            screen: Arc::clone(&screen),
            subtitle_dir,
            torrents,
            filter: StreamFilterView::default(),
            filtered: None,
            shown: None,
        };
        let restore = [restore, Action::RestoreSettings(settings)];
        let shell = std::thread::Builder::new()
            .name("shell".into())
            .spawn(move || {
                shell.run(&rx, restore);
                if writer.join().is_err() {
                    error!("the store thread panicked");
                }
            })
            .map_err(thread_error)?;
        Ok(Arc::new(Self {
            tx,
            client,
            screen,
            shell: Some(shell),
            runtime: Some(runtime),
        }))
    }

    fn send(&self, action: Action) {
        if self.tx.send(Msg::Action(action)).is_err() {
            warn!("the shell thread has stopped");
        }
    }

    fn filter(&self, change: FilterChange) {
        if self.tx.send(Msg::Filter(change)).is_err() {
            warn!("the shell thread has stopped");
        }
    }

    fn command(&self, command: PlayerCommand) {
        if let Some(playback) = &lock(&self.screen).playback {
            playback.send(command);
        }
    }

    fn stop_auto_subtitle(&self) {
        if let Some(subtitles) = &mut lock(&self.screen).subtitles {
            subtitles.auto.clear();
        }
    }
}

impl Drop for Cineo {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Shutdown);
        if let Some(shell) = self.shell.take()
            && shell.join().is_err()
        {
            error!("the shell thread panicked");
        }
        let finished = {
            let mut screen = lock(&self.screen);
            (screen.playback.take(), screen.subtitles.take())
        };
        drop(finished);
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

#[derive(Debug)]
struct Shell {
    state: State,
    client: AddonClient,
    runtime: Handle,
    tx: Sender<Msg>,
    store_tx: Sender<Effect>,
    listener: Arc<dyn StateListener>,
    players: Arc<dyn Players>,
    screen: Arc<Mutex<Screen>>,
    subtitle_dir: PathBuf,
    torrents: Torrents,
    filter: StreamFilterView,
    filtered: Option<String>,
    shown: Option<AppView>,
}

impl Shell {
    fn run(mut self, rx: &Receiver<Msg>, restore: [Action; 2]) {
        for action in restore {
            self.dispatch(action);
        }
        self.publish();
        while let Ok(first) = rx.recv() {
            let mut running = self.handle(first);
            while running && let Ok(msg) = rx.try_recv() {
                running = self.handle(msg);
            }
            if !running {
                self.torrents.shutdown(ENGINE_SHUTDOWN);
                return;
            }
            self.publish();
        }
    }

    fn handle(&mut self, msg: Msg) -> bool {
        match msg {
            Msg::Action(action) => self.dispatch(action),
            Msg::OpenDetail { content_type, id } => self.open_detail(content_type, id),
            Msg::Notice(notice) => self.state.notice = Some(notice),
            Msg::LoadSubtitle(url) => self.load_subtitle(url),
            Msg::SubtitleFetched { url, result } => self.subtitle_fetched(&url, result),
            Msg::Filter(change) => match change {
                FilterChange::Query(query) => self.filter.query = query,
                FilterChange::Quality(quality) => self.filter.quality = quality,
                FilterChange::Addon(addon) => self.filter.addon = addon,
                FilterChange::Sort(sort) => self.filter.sort = sort,
            },
            Msg::Shutdown => return false,
        }
        true
    }

    fn open_detail(&mut self, content_type: String, id: String) {
        let Some(content_type) = ContentType::new(content_type) else {
            warn!("not a content type");
            return;
        };
        let preview = self
            .state
            .board
            .iter()
            .chain(&self.state.search)
            .filter_map(|row| row.items.ready())
            .flatten()
            .find(|item| item.id == id && item.content_type == content_type)
            .cloned()
            .or_else(|| {
                self.state
                    .library
                    .iter()
                    .find(|item| item.id == id && item.content_type == content_type)
                    .map(LibraryItem::preview)
            })
            .map(Box::new);
        self.dispatch(Action::OpenDetail {
            content_type,
            id,
            preview,
        });
    }

    fn dispatch(&mut self, action: Action) {
        for effect in update(&mut self.state, action) {
            self.run_effect(effect);
        }
    }

    fn run_effect(&mut self, effect: Effect) {
        match effect {
            effect @ (Effect::FetchManifest { .. }
            | Effect::FetchCatalog { .. }
            | Effect::FetchAddonList { .. }
            | Effect::FetchMeta { .. }
            | Effect::FetchStreams { .. }
            | Effect::FetchSubtitles { .. }) => {
                if let Some(task) = fetch_task(&self.client, effect) {
                    let tx = self.tx.clone();
                    self.runtime.spawn(async move {
                        let _ = tx.send(Msg::Action(task.await));
                    });
                }
            }
            effect @ (Effect::SaveAddons(_)
            | Effect::SaveLibraryItem(_)
            | Effect::DeleteLibraryItem(_)
            | Effect::ClearLibrary
            | Effect::SaveSettings(_)) => {
                if self.store_tx.send(effect).is_err() {
                    self.state.notice = Some(Notice::StoreClosed);
                }
            }
            Effect::Play(request) => self.play(&request),
            Effect::StartTorrent(request) => {
                let tx = self.tx.clone();
                self.torrents
                    .start(request, &self.state.settings, move |action| {
                        let _ = tx.send(Msg::Action(action));
                    });
            }
            Effect::StopTorrent => self.torrents.stop(),
            Effect::CheckForUpdate(_) => {}
        }
    }

    fn play(&mut self, request: &PlayRequest) {
        let old = {
            let mut screen = lock(&self.screen);
            (screen.playback.take(), screen.subtitles.take())
        };
        drop(old);
        match self.players.start(request) {
            Ok((playback, events)) => {
                let subtitles = Subtitles {
                    files: SubtitleFiles::new(self.subtitle_dir.clone()),
                    groups: self.state.subtitles.clone(),
                    auto: request.settings.subtitle_languages().collect(),
                };
                let mut screen = lock(&self.screen);
                screen.subtitles = Some(subtitles);
                if let Some(wid) = screen.wid
                    && let Err(err) = playback.attach_surface(wid)
                {
                    warn!(%err, "the video surface was not attached");
                }
                screen.playback = Some(playback);
                screen.art = Some(ArtView::of(request));
                drop(screen);
                let tx = self.tx.clone();
                let (meta_id, video_id) = (request.meta_id.clone(), request.video_id.clone());
                let feed = self.torrents.feed();
                feed.playing(&request.url);
                self.runtime.spawn(forward_events(
                    events,
                    meta_id,
                    video_id,
                    move |event| match event {
                        PlayerEvent::Buffered { seconds } => feed.buffered(*seconds),
                        PlayerEvent::Progress { duration_ms, .. }
                        | PlayerEvent::Ended { duration_ms, .. }
                        | PlayerEvent::Closed { duration_ms, .. } => feed.duration(*duration_ms),
                        PlayerEvent::Failed(_) => {}
                    },
                    move |action| {
                        let _ = tx.send(Msg::Action(action));
                    },
                ));
            }
            Err(err) => {
                error!(%err, "playback could not start");
                self.dispatch(Action::PlaybackFailed(err.to_string()));
            }
        }
    }

    fn load_subtitle(&mut self, url: Url) {
        let Some((title, lang)) = find_subtitle(&self.state.subtitles, &url) else {
            return;
        };
        {
            let screen = lock(&self.screen);
            let (Some(playback), Some(subtitles)) = (&screen.playback, &screen.subtitles) else {
                return;
            };
            if let Some(path) = subtitles.files.get(&url) {
                if let Err(err) = playback.add_subtitle(path, &title, &lang) {
                    warn!(%err, "could not select the subtitle");
                }
                return;
            }
        }
        let client = self.client.clone();
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let result = client
                .fetch_subtitle(&url)
                .await
                .map_err(|err| err.to_string());
            let _ = tx.send(Msg::SubtitleFetched { url, result });
        });
    }

    fn subtitle_fetched(&mut self, url: &Url, result: Result<Vec<u8>, String>) {
        let Some((title, lang)) = find_subtitle(&self.state.subtitles, url) else {
            return;
        };
        let loaded = {
            let mut screen = lock(&self.screen);
            let screen = &mut *screen;
            let (Some(playback), Some(subtitles)) = (&screen.playback, &mut screen.subtitles)
            else {
                return;
            };
            result.and_then(|bytes| {
                let path = subtitles
                    .files
                    .write(url, &bytes)
                    .map_err(|err| format!("cannot save it: {err}"))?;
                playback
                    .add_subtitle(&path, &title, &lang)
                    .map_err(|err| err.to_string())
            })
        };
        if let Err(err) = loaded {
            warn!(%err, "addon subtitle failed");
            self.state.notice = Some(Notice::SubtitleFailed(err));
        }
    }

    fn publish(&mut self) {
        let (playing, finished) = {
            let mut screen = lock(&self.screen);
            let finished = if self.state.playing.is_none() {
                screen.art = None;
                (screen.playback.take(), screen.subtitles.take())
            } else {
                if let Some(subtitles) = &mut screen.subtitles
                    && subtitles.groups != self.state.subtitles
                {
                    subtitles.groups.clone_from(&self.state.subtitles);
                }
                (None, None)
            };
            (screen.art.clone(), finished)
        };
        drop(finished);
        let detail = self.state.detail.as_ref().map(|d| d.id.clone());
        if detail != self.filtered {
            self.filter = StreamFilterView {
                sort: self.filter.sort,
                ..StreamFilterView::default()
            };
            self.filtered = detail;
        }
        let next = view(&self.state, playing.as_ref(), &self.filter);
        if self.shown.as_ref() != Some(&next) {
            self.listener.changed(next.clone());
            self.shown = Some(next);
        }
    }
}

#[cfg(test)]
mod tests {
    use cineo_core::app::Settings;

    use super::*;

    #[derive(Debug)]
    struct Ignore;

    impl StateListener for Ignore {
        fn changed(&self, _view: AppView) {}
    }

    #[test]
    fn the_desktop_update_check_does_not_run() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (tx, rx) = channel();
        let (store_tx, _store_rx) = channel();
        let mut shell = Shell {
            state: State::default(),
            client: AddonClient::new(NetPolicy::default()).unwrap(),
            runtime: runtime.handle().clone(),
            tx,
            store_tx,
            listener: Arc::new(Ignore),
            players: Arc::new(Mpv),
            screen: Arc::default(),
            subtitle_dir: std::env::temp_dir(),
            torrents: Torrents::new(
                runtime.handle().clone(),
                EngineOptions::new(std::env::temp_dir()),
            ),
            filter: StreamFilterView::default(),
            filtered: None,
            shown: None,
        };
        shell.dispatch(Action::RestoreSettings(Settings {
            check_for_updates: true,
            ..Settings::default()
        }));
        assert!(shell.state.update_checked);
        assert_eq!(runtime.metrics().num_alive_tasks(), 0);
        assert!(rx.try_recv().is_err());
    }
}
