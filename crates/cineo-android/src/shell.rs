use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cineo_core::addon::{ContentType, MetaPreview, PosterShape, TransportUrl};
use cineo_core::app::{
    Action, Effect, Language, LibraryItem, Notice, PlayRequest, SeekStep, Setting, State,
    WatchedAt, update,
};
use cineo_net::{AddonClient, MAX_IMAGE_BYTES, NetPolicy, fetch_task};
use cineo_player::embedded::PlayerCommand;
use cineo_player::{EventActions, PlayerEvent, event_actions};
use cineo_store::Store;
use cineo_stream::{EngineOptions, Torrents};
use tokio::runtime::{Handle, Runtime};
use tracing::{error, warn};
use url::Url;

use crate::player::{Mpv, Playback, Players};
use crate::view::{AppView, ArtView, StreamFilterView, StreamSortView, view};

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
}

#[derive(Debug)]
enum FilterChange {
    Query(String),
    Quality(Option<String>),
    Addon(Option<u32>),
    Sort(StreamSortView),
}

#[derive(Debug)]
enum Msg {
    Action(Action),
    Filter(FilterChange),
    OpenDetail { content_type: String, id: String },
    Notice(Notice),
    Shutdown,
}

#[derive(Debug, Default)]
struct Screen {
    playback: Option<Box<dyn Playback>>,
    wid: Option<i64>,
    art: Option<ArtView>,
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
        let engine = EngineOptions::new(PathBuf::from(cache_dir).join("torrents"));
        Self::start(data_dir, engine, listener, Arc::new(Mpv))
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

    pub fn set_stream_quality(&self, quality: Option<String>) {
        self.filter(FilterChange::Quality(quality));
    }

    pub fn set_stream_addon(&self, addon: Option<u32>) {
        self.filter(FilterChange::Addon(addon));
    }

    pub fn set_stream_sort(&self, sort: StreamSortView) {
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
        let screen = lock(&self.screen);
        let status = screen.playback.as_ref()?.status();
        Some(PlayerStatusView {
            loaded: status.loaded,
            position_s: status.position_s,
            duration_s: status.duration_s,
            paused: status.paused,
            waiting: status.is_waiting(),
        })
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
}

impl Drop for Cineo {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Shutdown);
        if let Some(shell) = self.shell.take()
            && shell.join().is_err()
        {
            error!("the shell thread panicked");
        }
        let playback = lock(&self.screen).playback.take();
        drop(playback);
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
                    .map(library_preview)
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
        let old = lock(&self.screen).playback.take();
        drop(old);
        match self.players.start(request) {
            Ok((playback, events)) => {
                let mut screen = lock(&self.screen);
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
                self.runtime.spawn(async move {
                    let mut events = events;
                    while let Some(event) = events.recv().await {
                        match &event {
                            PlayerEvent::Buffered { seconds } => feed.buffered(*seconds),
                            PlayerEvent::Progress { duration_ms, .. }
                            | PlayerEvent::Ended { duration_ms, .. }
                            | PlayerEvent::Closed { duration_ms, .. } => {
                                feed.duration(*duration_ms);
                            }
                            _ => {}
                        }
                        let EventActions { actions, last } =
                            event_actions(event, &meta_id, &video_id, now_ms());
                        for action in actions {
                            let _ = tx.send(Msg::Action(action));
                        }
                        if last {
                            break;
                        }
                    }
                });
            }
            Err(err) => {
                error!(%err, "playback could not start");
                self.dispatch(Action::PlaybackFailed(err.to_string()));
            }
        }
    }

    fn publish(&mut self) {
        let (playing, finished) = {
            let mut screen = lock(&self.screen);
            let finished = if self.state.playing.is_none() {
                screen.art = None;
                screen.playback.take()
            } else {
                None
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

fn library_preview(item: &LibraryItem) -> MetaPreview {
    MetaPreview {
        id: item.id.clone(),
        content_type: item.content_type.clone(),
        name: item.name.clone(),
        poster: item.poster.clone(),
        poster_shape: PosterShape::Poster,
        background: None,
        logo: None,
        description: None,
        release_info: None,
        imdb_rating: None,
        genres: Vec::new(),
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
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
