//! The application state and its pure transition function.

use url::Url;

use super::library::{LibraryItem, SavedStream};
use super::plan::{self, CatalogTarget};
use super::settings::{Setting, Settings};
use super::torrent::{TorrentPlayback, TorrentRequest, TorrentStatus, is_engine_url};
use crate::addon::{
    ContentType, ExtraValue, Manifest, Meta, MetaPreview, ResourcePath, Stream, StreamSource,
    Subtitle, TransportUrl,
};

/// An installed addon: identity (transport URL) plus its manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct InstalledAddon {
    pub transport: TransportUrl,
    pub manifest: Manifest,
}

/// The state of one asynchronous value.
#[derive(Debug, Clone, PartialEq)]
pub enum Loadable<T> {
    Loading,
    Ready(T),
    Failed(String),
}

impl<T> Loadable<T> {
    pub fn ready(&self) -> Option<&T> {
        match self {
            Self::Ready(value) => Some(value),
            _ => None,
        }
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }
}

/// A row of catalog items (board, search).
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub target: CatalogTarget,
    pub items: Loadable<Vec<MetaPreview>>,
}

/// The Discover page: one catalog, an optional genre, paged with `skip`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Discover {
    pub target: Option<CatalogTarget>,
    pub genre: Option<String>,
    pub items: Vec<MetaPreview>,
    /// The page request in flight, if any.
    pub pending: Option<ResourcePath>,
    pub error: Option<String>,
    /// `skip` value of the next page, if the catalog supports paging and the
    /// last page was not empty.
    pub next_skip: Option<usize>,
}

/// Streams from one addon.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamGroup {
    pub addon: TransportUrl,
    pub addon_name: String,
    pub path: ResourcePath,
    pub streams: Loadable<Vec<Stream>>,
}

/// The detail page of one item.
#[derive(Debug, Clone, PartialEq)]
pub struct Detail {
    pub content_type: ContentType,
    pub id: String,
    /// Shown while the full meta loads.
    pub preview: Option<MetaPreview>,
    pub meta: Loadable<Meta>,
    /// Remaining meta addons to try if the current one fails.
    pub meta_fallbacks: Vec<(TransportUrl, ResourcePath)>,
    pub meta_request: Option<(TransportUrl, ResourcePath)>,
    pub selected_video: Option<String>,
    pub streams: Vec<StreamGroup>,
}

/// What the player shell needs to start playback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayRequest {
    /// Always `http(s)` (checked by [`update`]); for a torrent, the engine's
    /// loopback URL.
    pub url: Url,
    pub title: String,
    /// The item's logo art (meta `logo`), shown while the file loads.
    pub logo: Option<Url>,
    /// The item's backdrop (meta `background`), shown behind the logo.
    pub background: Option<Url>,
    pub headers: Vec<(String, String)>,
    /// The settings when playback was asked for.
    pub settings: Settings,
    pub start_ms: u64,
    pub meta_id: String,
    pub video_id: String,
}

/// Subtitles for the current playback from one addon. `path` is `None` for
/// the subtitles that came with the stream itself.
#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleGroup {
    pub addon: TransportUrl,
    pub addon_name: String,
    pub path: Option<ResourcePath>,
    pub subtitles: Loadable<Vec<Subtitle>>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct State {
    /// Every installed addon in user order, including those still loading
    /// or whose manifest failed to load. This is what gets saved.
    pub installed: Vec<TransportUrl>,
    /// The installed addons whose manifest loaded, in user order.
    pub addons: Vec<InstalledAddon>,
    /// Addons whose manifests are being (re)loaded, e.g. at startup.
    pub addons_loading: Vec<TransportUrl>,
    /// Result of the last install attempt, for the addon screen.
    pub install: Option<Loadable<String>>,
    pub board: Vec<Row>,
    pub discover: Discover,
    pub search_query: String,
    pub search: Vec<Row>,
    pub detail: Option<Detail>,
    pub library: Vec<LibraryItem>,
    /// Last playback problem to show to the user.
    pub notice: Option<String>,
    pub settings: Settings,
    /// A torrent stream (`group`, `stream`) waiting for the user to accept
    /// the P2P notice.
    pub p2p_prompt: Option<(usize, usize)>,
    pub torrent: Option<TorrentPlayback>,
    /// Addon subtitles for the current playback, stream subtitles first.
    pub subtitles: Vec<SubtitleGroup>,
    pub playing: Option<Playing>,
}

/// The playback in progress, until it stops or fails.
#[derive(Debug, Clone, PartialEq)]
pub struct Playing {
    pub meta_id: String,
    /// Replays a saved stream that has not progressed yet; if it fails, the
    /// detail page opens instead.
    pub resumed: bool,
    /// Saved on the item once playback progresses.
    pub stream: Option<Box<SavedStream>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Restore {
        addons: Vec<TransportUrl>,
        library: Vec<LibraryItem>,
    },
    RestoreSettings(Settings),
    InstallAddon(String),
    RemoveAddon(TransportUrl),
    MoveAddon {
        from: usize,
        to: usize,
    },
    LoadBoard,
    OpenDiscover {
        addon: TransportUrl,
        path: ResourcePath,
    },
    SetDiscoverGenre(Option<String>),
    LoadMoreDiscover,
    Search(String),
    OpenDetail {
        content_type: ContentType,
        id: String,
        preview: Option<Box<MetaPreview>>,
    },
    CloseDetail,
    SelectVideo(String),
    /// Plays a library item from its saved stream, or opens its detail page.
    Resume(String),
    Play {
        group: usize,
        stream: usize,
    },
    PlaybackProgress {
        meta_id: String,
        video_id: String,
        time_ms: u64,
        duration_ms: u64,
        now_ms: u64,
    },
    PlaybackFailed(String),
    PlaybackStopped,
    RemoveFromLibrary(String),
    DismissContinueWatching(String),
    SetFavorite {
        id: String,
        favorite: bool,
    },
    ClearLibrary,
    DismissNotice,
    AcceptP2p,
    DeclineP2p,
    ChangeSetting(Setting),
    ResetSettings,
    ManifestLoaded {
        transport: TransportUrl,
        result: Result<Box<Manifest>, String>,
        install: bool,
    },
    CatalogLoaded {
        addon: TransportUrl,
        path: ResourcePath,
        result: Result<Vec<MetaPreview>, String>,
    },
    MetaLoaded {
        addon: TransportUrl,
        path: ResourcePath,
        result: Result<Box<Meta>, String>,
    },
    StreamsLoaded {
        addon: TransportUrl,
        path: ResourcePath,
        result: Result<Vec<Stream>, String>,
    },
    SubtitlesLoaded {
        addon: TransportUrl,
        path: ResourcePath,
        result: Result<Vec<Subtitle>, String>,
    },
    TorrentReady {
        info_hash: String,
        url: Url,
    },
    TorrentStatus {
        info_hash: String,
        status: TorrentStatus,
    },
    TorrentFailed {
        info_hash: String,
        reason: String,
    },
}

/// Side effects requested by [`update`]; executed by the shell.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    FetchManifest {
        transport: TransportUrl,
        install: bool,
    },
    FetchCatalog {
        addon: TransportUrl,
        path: ResourcePath,
    },
    FetchMeta {
        addon: TransportUrl,
        path: ResourcePath,
    },
    FetchStreams {
        addon: TransportUrl,
        path: ResourcePath,
    },
    FetchSubtitles {
        addon: TransportUrl,
        path: ResourcePath,
    },
    SaveAddons(Vec<TransportUrl>),
    SaveLibraryItem(LibraryItem),
    DeleteLibraryItem(String),
    ClearLibrary,
    SaveSettings(Settings),
    Play(Box<PlayRequest>),
    StartTorrent(TorrentRequest),
    StopTorrent,
}

/// Applies `action` to `state` and returns the effects to run.
pub fn update(state: &mut State, action: Action) -> Vec<Effect> {
    match action {
        Action::Restore { addons, library } => {
            state.library = library;
            state.installed.clone_from(&addons);
            state.addons_loading.clone_from(&addons);
            addons
                .into_iter()
                .map(|transport| Effect::FetchManifest {
                    transport,
                    install: false,
                })
                .collect()
        }
        Action::RestoreSettings(settings) => {
            state.settings = settings;
            Vec::new()
        }
        Action::InstallAddon(input) => match TransportUrl::parse(&input) {
            Ok(transport)
                if state.addons.iter().any(|a| a.transport == transport)
                    || state.addons_loading.contains(&transport) =>
            {
                state.install = Some(Loadable::Failed("This addon is already installed".into()));
                Vec::new()
            }
            Ok(transport) => {
                state.install = Some(Loadable::Loading);
                vec![Effect::FetchManifest {
                    transport,
                    install: true,
                }]
            }
            Err(err) => {
                state.install = Some(Loadable::Failed(format!("Invalid addon URL: {err}")));
                Vec::new()
            }
        },
        Action::RemoveAddon(transport) => {
            state.installed.retain(|t| *t != transport);
            state.addons_loading.retain(|t| *t != transport);
            state.addons.retain(|a| a.transport != transport);
            let mut effects = vec![save_addons(state)];
            effects.extend(refresh_board(state));
            effects
        }
        Action::MoveAddon { from, to } => {
            if from < state.addons.len() && to < state.addons.len() && from != to {
                let addon = state.addons.remove(from);
                state.addons.insert(to, addon);
                let mut order = state.addons.iter().map(|a| a.transport.clone());
                for slot in &mut state.installed {
                    if state.addons.iter().any(|a| a.transport == *slot)
                        && let Some(next) = order.next()
                    {
                        *slot = next;
                    }
                }
                let mut effects = vec![save_addons(state)];
                effects.extend(refresh_board(state));
                effects
            } else {
                Vec::new()
            }
        }
        Action::LoadBoard => load_board(state),
        Action::OpenDiscover { addon, path } => {
            let target = plan::board_targets(&state.addons)
                .into_iter()
                .chain(catalogs_with_required(state))
                .find(|t| t.addon == addon && t.path == path);
            state.discover = Discover {
                target,
                genre: default_genre(state, &addon, &path),
                ..Discover::default()
            };
            discover_page(state, 0)
        }
        Action::SetDiscoverGenre(genre) => {
            state.discover.genre = genre;
            state.discover.items.clear();
            discover_page(state, 0)
        }
        Action::LoadMoreDiscover => match state.discover.next_skip {
            Some(skip) if state.discover.pending.is_none() => discover_page(state, skip),
            _ => Vec::new(),
        },
        Action::Search(query) => {
            let query = query.trim().to_owned();
            state.search_query.clone_from(&query);
            if query.is_empty() {
                state.search.clear();
                return Vec::new();
            }
            state.search = plan::search_targets(&state.addons, &query)
                .into_iter()
                .map(|target| Row {
                    target,
                    items: Loadable::Loading,
                })
                .collect();
            state
                .search
                .iter()
                .map(|r| fetch_catalog(&r.target))
                .collect()
        }
        Action::OpenDetail {
            content_type,
            id,
            preview,
        } => open_detail(state, content_type, id, preview.map(|p| *p)),
        Action::CloseDetail => {
            state.detail = None;
            Vec::new()
        }
        Action::SelectVideo(video_id) => select_video(state, video_id),
        Action::Resume(meta_id) => resume(state, &meta_id),
        Action::Play { group, stream } => play(state, group, stream),
        Action::PlaybackProgress {
            meta_id,
            video_id,
            time_ms,
            duration_ms,
            now_ms,
        } => {
            let mut stream = None;
            if let Some(playing) = state.playing.as_mut().filter(|p| p.meta_id == meta_id) {
                playing.resumed = false;
                stream = playing.stream.take();
            }
            let Some(item) = state.library.iter_mut().find(|i| i.id == meta_id) else {
                return Vec::new();
            };
            if stream.is_some() {
                item.stream = stream;
            }
            item.video_id = video_id;
            item.time_offset_ms = time_ms;
            if duration_ms > 0 {
                item.duration_ms = duration_ms;
            }
            item.updated_ms = now_ms;
            vec![Effect::SaveLibraryItem(item.clone())]
        }
        Action::PlaybackFailed(reason) => {
            state.subtitles.clear();
            let mut effects = stop_torrent(state);
            effects.extend(playback_ended(state, Some(reason)));
            effects
        }
        Action::PlaybackStopped => {
            state.subtitles.clear();
            let mut effects = stop_torrent(state);
            effects.extend(playback_ended(state, None));
            effects
        }
        Action::SubtitlesLoaded {
            addon,
            path,
            result,
        } => {
            if let Some(group) = state
                .subtitles
                .iter_mut()
                .find(|g| g.addon == addon && g.path.as_ref() == Some(&path))
            {
                group.subtitles = match result {
                    Ok(mut subtitles) => {
                        dedup_by_url(&mut subtitles);
                        Loadable::Ready(subtitles)
                    }
                    Err(err) => Loadable::Failed(err),
                };
            }
            Vec::new()
        }
        Action::AcceptP2p => {
            state.settings.p2p_acknowledged = true;
            let mut effects = vec![Effect::SaveSettings(state.settings)];
            if let Some((group, stream)) = state.p2p_prompt.take() {
                effects.extend(play(state, group, stream));
            }
            effects
        }
        Action::DeclineP2p => {
            state.p2p_prompt = None;
            Vec::new()
        }
        Action::ChangeSetting(setting) => {
            state.settings.set(setting);
            settings_changed(state)
        }
        Action::ResetSettings => {
            state.settings = state.settings.reset();
            settings_changed(state)
        }
        Action::SetFavorite { id, favorite } => set_favorite(state, id, favorite),
        Action::RemoveFromLibrary(id) => {
            state.library.retain(|i| i.id != id);
            vec![Effect::DeleteLibraryItem(id)]
        }
        Action::DismissContinueWatching(id) => {
            let Some(item) = state.library.iter_mut().find(|i| i.id == id) else {
                return Vec::new();
            };
            item.time_offset_ms = 0;
            vec![Effect::SaveLibraryItem(item.clone())]
        }
        Action::ClearLibrary => {
            state.library.clear();
            vec![Effect::ClearLibrary]
        }
        Action::DismissNotice => {
            state.notice = None;
            Vec::new()
        }
        Action::ManifestLoaded {
            transport,
            result,
            install,
        } => manifest_loaded(state, &transport, result, install),
        Action::CatalogLoaded {
            addon,
            path,
            result,
        } => {
            catalog_loaded(state, &addon, &path, result);
            Vec::new()
        }
        Action::MetaLoaded {
            addon,
            path,
            result,
        } => meta_loaded(state, &addon, &path, result),
        Action::StreamsLoaded {
            addon,
            path,
            result,
        } => {
            if let Some(group) = state.detail.as_mut().and_then(|d| {
                d.streams
                    .iter_mut()
                    .find(|g| g.addon == addon && g.path == path)
            }) {
                group.streams = match result {
                    Ok(streams) => Loadable::Ready(streams),
                    Err(err) => Loadable::Failed(err),
                };
            }
            Vec::new()
        }
        Action::TorrentReady { info_hash, url } => torrent_ready(state, &info_hash, url),
        Action::TorrentStatus { info_hash, status } => {
            if let Some(torrent) = state.torrent.as_mut().filter(|t| t.info_hash == info_hash) {
                torrent.status = status;
            }
            Vec::new()
        }
        Action::TorrentFailed { info_hash, reason } => {
            if state
                .torrent
                .as_ref()
                .is_some_and(|t| t.info_hash == info_hash)
            {
                let mut effects = stop_torrent(state);
                effects.extend(playback_ended(
                    state,
                    Some(format!("The torrent could not be played: {reason}")),
                ));
                effects
            } else {
                Vec::new()
            }
        }
    }
}

fn settings_changed(state: &mut State) -> Vec<Effect> {
    let mut effects = vec![Effect::SaveSettings(state.settings)];
    if !state.settings.p2p_enabled {
        state.p2p_prompt = None;
        effects.extend(stop_torrent(state));
    }
    effects
}

fn stop_torrent(state: &mut State) -> Vec<Effect> {
    if state.torrent.take().is_some() {
        vec![Effect::StopTorrent]
    } else {
        Vec::new()
    }
}

fn torrent_ready(state: &mut State, info_hash: &str, url: Url) -> Vec<Effect> {
    let Some(torrent) = state.torrent.as_mut().filter(|t| t.info_hash == info_hash) else {
        return Vec::new(); // stale
    };
    if !is_engine_url(&url) {
        state.notice = Some("The torrent engine returned an unexpected address".into());
        return stop_torrent(state);
    }
    match torrent.pending.take() {
        Some(request) => vec![Effect::Play(Box::new(PlayRequest { url, ..request }))],
        None => Vec::new(),
    }
}

impl State {
    /// Installed addons whose manifest failed to load.
    pub fn unavailable_addons(&self) -> Vec<&TransportUrl> {
        self.installed
            .iter()
            .filter(|t| {
                !self.addons_loading.contains(t) && !self.addons.iter().any(|a| a.transport == **t)
            })
            .collect()
    }
}

fn save_addons(state: &State) -> Effect {
    Effect::SaveAddons(state.installed.clone())
}

fn fetch_catalog(target: &CatalogTarget) -> Effect {
    Effect::FetchCatalog {
        addon: target.addon.clone(),
        path: target.path.clone(),
    }
}

fn load_board(state: &mut State) -> Vec<Effect> {
    state.board = plan::board_targets(&state.addons)
        .into_iter()
        .map(|target| Row {
            target,
            items: Loadable::Loading,
        })
        .collect();
    state
        .board
        .iter()
        .map(|r| fetch_catalog(&r.target))
        .collect()
}

fn refresh_board(state: &mut State) -> Vec<Effect> {
    let mut old = std::mem::take(&mut state.board);
    let mut effects = Vec::new();
    state.board = plan::board_targets(&state.addons)
        .into_iter()
        .map(|target| match old.iter().position(|r| r.target == target) {
            Some(index) => old.swap_remove(index),
            None => {
                effects.push(fetch_catalog(&target));
                Row {
                    target,
                    items: Loadable::Loading,
                }
            }
        })
        .collect();
    effects
}

fn catalogs_with_required(state: &State) -> Vec<CatalogTarget> {
    state
        .addons
        .iter()
        .flat_map(|a| {
            a.manifest
                .catalogs
                .iter()
                .filter(|c| !c.is_browsable())
                .map(move |c| CatalogTarget {
                    addon: a.transport.clone(),
                    addon_name: a.manifest.name.clone(),
                    path: ResourcePath::catalog(c.content_type.clone(), c.id.clone()),
                    title: c.name.clone().unwrap_or_else(|| c.id.clone()),
                })
        })
        .collect()
}

fn default_genre(state: &State, addon: &TransportUrl, path: &ResourcePath) -> Option<String> {
    let catalog = state
        .addons
        .iter()
        .find(|a| &a.transport == addon)?
        .manifest
        .catalog(&path.content_type, &path.id)?;
    let genre = catalog.extra.iter().find(|e| e.name == "genre")?;
    if genre.is_required {
        genre.options.first().cloned()
    } else {
        None
    }
}

fn discover_page(state: &mut State, skip: usize) -> Vec<Effect> {
    let Some(target) = state.discover.target.clone() else {
        return Vec::new();
    };
    let mut extra = Vec::new();
    if let Some(genre) = &state.discover.genre {
        extra.push(ExtraValue::new("genre", genre.clone()));
    }
    if skip > 0 {
        extra.push(ExtraValue::new("skip", skip.to_string()));
    }
    let path = target.path.clone().with_extra(extra);
    let supported = state
        .addons
        .iter()
        .find(|a| a.transport == target.addon)
        .is_some_and(|a| a.manifest.supports(&path));
    if !supported {
        state.discover.error = Some("This catalog does not support that filter".into());
        return Vec::new();
    }
    state.discover.error = None;
    state.discover.pending = Some(path.clone());
    vec![Effect::FetchCatalog {
        addon: target.addon,
        path,
    }]
}

fn manifest_loaded(
    state: &mut State,
    transport: &TransportUrl,
    result: Result<Box<Manifest>, String>,
    install: bool,
) -> Vec<Effect> {
    let was_loading = state.addons_loading.contains(transport);
    state.addons_loading.retain(|t| t != transport);
    if !install && !was_loading {
        return Vec::new(); // removed while loading
    }
    match result {
        Ok(manifest) => {
            if install {
                state.install = Some(Loadable::Ready(manifest.name.clone()));
            }
            let addon = InstalledAddon {
                transport: transport.clone(),
                manifest: *manifest,
            };
            if let Some(existing) = state.addons.iter_mut().find(|a| &a.transport == transport) {
                *existing = addon;
            } else {
                if !state.installed.contains(transport) {
                    state.installed.push(transport.clone());
                }
                let rank = |t: &TransportUrl| state.installed.iter().position(|i| i == t);
                let at = state
                    .addons
                    .iter()
                    .take_while(|a| rank(&a.transport) < rank(transport))
                    .count();
                state.addons.insert(at, addon);
            }
            let mut effects = Vec::new();
            if install {
                effects.push(save_addons(state));
            }
            effects.extend(refresh_board(state));
            effects
        }
        Err(err) => {
            if install {
                state.install = Some(Loadable::Failed(err));
            } else {
                state.notice = Some(format!("An addon could not be loaded: {err}"));
            }
            Vec::new()
        }
    }
}

fn catalog_loaded(
    state: &mut State,
    addon: &TransportUrl,
    path: &ResourcePath,
    result: Result<Vec<MetaPreview>, String>,
) {
    let as_loadable = |result: &Result<Vec<MetaPreview>, String>| match result {
        Ok(items) => Loadable::Ready(items.clone()),
        Err(err) => Loadable::Failed(err.clone()),
    };
    for row in state.board.iter_mut().chain(state.search.iter_mut()) {
        if &row.target.addon == addon && &row.target.path == path {
            row.items = as_loadable(&result);
        }
    }
    let discover = &mut state.discover;
    let is_current = discover.pending.as_ref() == Some(path)
        && discover.target.as_ref().is_some_and(|t| &t.addon == addon);
    if is_current {
        discover.pending = None;
        match result {
            Ok(items) => {
                let skip = path
                    .extra
                    .iter()
                    .find(|e| e.name == "skip")
                    .and_then(|e| e.value.parse::<usize>().ok())
                    .unwrap_or(0);
                let pages = state
                    .addons
                    .iter()
                    .find(|a| &a.transport == addon)
                    .and_then(|a| a.manifest.catalog(&path.content_type, &path.id))
                    .is_some_and(|c| c.extra.iter().any(|e| e.name == "skip"));
                discover.next_skip = (pages && !items.is_empty()).then(|| skip + items.len());
                if skip == 0 {
                    discover.items = items;
                } else {
                    for item in items {
                        if !discover.items.iter().any(|i| i.id == item.id) {
                            discover.items.push(item);
                        }
                    }
                }
            }
            Err(err) => discover.error = Some(err),
        }
    }
}

fn meta_loaded(
    state: &mut State,
    addon: &TransportUrl,
    path: &ResourcePath,
    result: Result<Box<Meta>, String>,
) -> Vec<Effect> {
    let Some(detail) = state.detail.as_mut() else {
        return Vec::new();
    };
    if detail.meta_request.as_ref() != Some(&(addon.clone(), path.clone())) {
        return Vec::new(); // stale
    }
    match result {
        Ok(meta) => {
            let auto_video = if meta.videos.is_empty() {
                Some(meta.preview.id.clone())
            } else {
                meta.default_video_id
                    .clone()
                    .filter(|id| meta.videos.iter().any(|v| &v.id == id))
            };
            detail.meta = Loadable::Ready(*meta);
            detail.meta_request = None;
            match auto_video {
                Some(video_id) => select_video(state, video_id),
                None => Vec::new(),
            }
        }
        Err(err) => {
            if detail.meta_fallbacks.is_empty() {
                detail.meta = Loadable::Failed(err);
                detail.meta_request = None;
                Vec::new()
            } else {
                let (addon, path) = detail.meta_fallbacks.remove(0);
                detail.meta_request = Some((addon.clone(), path.clone()));
                vec![Effect::FetchMeta { addon, path }]
            }
        }
    }
}

fn select_video(state: &mut State, video_id: String) -> Vec<Effect> {
    let addons = &state.addons;
    let Some(detail) = state.detail.as_mut() else {
        return Vec::new();
    };
    detail.streams = plan::stream_targets(addons, &detail.content_type, &video_id)
        .into_iter()
        .map(|(addon, path)| StreamGroup {
            addon_name: addons
                .iter()
                .find(|a| a.transport == addon)
                .map(|a| a.manifest.name.clone())
                .unwrap_or_default(),
            addon,
            path,
            streams: Loadable::Loading,
        })
        .collect();
    detail.selected_video = Some(video_id);
    detail
        .streams
        .iter()
        .map(|g| Effect::FetchStreams {
            addon: g.addon.clone(),
            path: g.path.clone(),
        })
        .collect()
}

fn set_favorite(state: &mut State, id: String, favorite: bool) -> Vec<Effect> {
    let next = state
        .library
        .iter()
        .filter_map(|i| i.favorited)
        .max()
        .unwrap_or(0)
        + 1;
    if let Some(index) = state.library.iter().position(|i| i.id == id) {
        let item = &mut state.library[index];
        if !favorite && !item.was_played() {
            state.library.remove(index);
            return vec![Effect::DeleteLibraryItem(id)];
        }
        if favorite == item.is_favorite() {
            return Vec::new();
        }
        item.favorited = favorite.then_some(next);
        return vec![Effect::SaveLibraryItem(item.clone())];
    }
    let Some(detail) = state.detail.as_ref().filter(|d| favorite && d.id == id) else {
        return Vec::new();
    };
    let preview = detail
        .meta
        .ready()
        .map(|m| &m.preview)
        .or(detail.preview.as_ref());
    let item = LibraryItem {
        id: id.clone(),
        content_type: detail.content_type.clone(),
        name: preview.map_or_else(|| id.clone(), |p| p.name.clone()),
        poster: preview.and_then(|p| p.poster.clone()),
        video_id: id,
        time_offset_ms: 0,
        duration_ms: 0,
        updated_ms: 0,
        favorited: Some(next),
        stream: None,
    };
    state.library.push(item.clone());
    vec![Effect::SaveLibraryItem(item)]
}

fn open_detail(
    state: &mut State,
    content_type: ContentType,
    id: String,
    preview: Option<MetaPreview>,
) -> Vec<Effect> {
    let mut candidates = plan::meta_candidates(&state.addons, &content_type, &id);
    let first = if candidates.is_empty() {
        None
    } else {
        Some(candidates.remove(0))
    };
    state.detail = Some(Detail {
        meta: if first.is_some() {
            Loadable::Loading
        } else {
            Loadable::Failed("No installed addon provides details for this item".into())
        },
        content_type,
        id,
        preview,
        meta_fallbacks: candidates,
        meta_request: first.clone(),
        selected_video: None,
        streams: Vec::new(),
    });
    first
        .map(|(addon, path)| vec![Effect::FetchMeta { addon, path }])
        .unwrap_or_default()
}

enum Source {
    Play(Url, Option<TorrentRequest>),
    NeedsConsent,
    Refused(String),
    Unusable,
}

fn resolve(settings: &Settings, stream: &Stream) -> Source {
    match &stream.source {
        StreamSource::Url(url) if stream.source.is_playable() => Source::Play(url.clone(), None),
        StreamSource::Url(url) => {
            Source::Refused(format!("Unsupported stream scheme `{}`", url.scheme()))
        }
        StreamSource::Torrent { .. } if !settings.p2p_enabled => {
            Source::Refused("Torrent streams are turned off in Settings".into())
        }
        StreamSource::Torrent { .. } if !settings.p2p_acknowledged => Source::NeedsConsent,
        StreamSource::Torrent { .. } => match TorrentRequest::from_stream(stream) {
            Some(request) => Source::Play(request.magnet(), Some(request)),
            None => Source::Unusable,
        },
        _ => Source::Refused(format!(
            "{} streams are not supported yet",
            stream.source.kind_label()
        )),
    }
}

fn play(state: &mut State, group: usize, stream_index: usize) -> Vec<Effect> {
    let Some(detail) = state.detail.as_ref() else {
        return Vec::new();
    };
    let Some(video_id) = detail.selected_video.clone() else {
        return Vec::new();
    };
    let Some(source_group) = detail.streams.get(group) else {
        return Vec::new();
    };
    let Some(stream) = source_group
        .streams
        .ready()
        .and_then(|s| s.get(stream_index))
    else {
        return Vec::new();
    };
    let (url, torrent) = match resolve(&state.settings, stream) {
        Source::Play(url, torrent) => (url, torrent),
        Source::NeedsConsent => {
            state.p2p_prompt = Some((group, stream_index));
            return Vec::new();
        }
        Source::Refused(notice) => {
            state.notice = Some(notice);
            return Vec::new();
        }
        Source::Unusable => return Vec::new(),
    };
    let preview = detail
        .meta
        .ready()
        .map(|m| m.preview.clone())
        .or_else(|| detail.preview.clone());
    let name = preview.as_ref().map(|p| p.name.clone()).unwrap_or_default();
    let episode_title = detail.meta.ready().and_then(|m| {
        m.videos
            .iter()
            .find(|v| v.id == video_id)
            .map(|v| match (v.season, v.episode) {
                (Some(s), Some(e)) => format!("S{s:02}E{e:02} {}", v.title),
                _ => v.title.clone(),
            })
    });
    let title = match episode_title {
        Some(ep) => format!("{name} — {ep}"),
        None => name.clone(),
    };
    let subtitles = subtitle_groups(
        state,
        &detail.content_type,
        Some((&source_group.addon, &source_group.addon_name)),
        stream,
        &video_id,
    );
    let headers = if torrent.is_some() {
        Vec::new()
    } else {
        stream.request_headers.clone()
    };
    let saved = SavedStream {
        addon: source_group.addon.clone(),
        title: title.clone(),
        logo: preview.as_ref().and_then(|p| p.logo.clone()),
        background: preview.as_ref().and_then(|p| p.background.clone()),
        stream: stream.clone(),
    };
    let request = PlayRequest {
        url,
        title,
        logo: saved.logo.clone(),
        background: saved.background.clone(),
        headers,
        settings: state.settings,
        start_ms: 0,
        meta_id: detail.id.clone(),
        video_id: video_id.clone(),
    };
    let content_type = detail.content_type.clone();
    let meta_id = detail.id.clone();

    let start_ms = match state.library.iter_mut().find(|i| i.id == meta_id) {
        Some(item) => {
            let start = item.resume_ms(&video_id, state.settings.watched_at);
            if item.video_id != video_id {
                item.video_id.clone_from(&video_id);
                item.time_offset_ms = 0;
                item.duration_ms = 0;
            }
            start
        }
        None => {
            state.library.push(LibraryItem {
                id: meta_id.clone(),
                content_type,
                name,
                poster: preview.and_then(|p| p.poster),
                video_id,
                time_offset_ms: 0,
                duration_ms: 0,
                updated_ms: 0,
                favorited: None,
                stream: None,
            });
            0
        }
    };
    let mut effects = Vec::new();
    if let Some(item) = state.library.iter().find(|i| i.id == meta_id) {
        effects.push(Effect::SaveLibraryItem(item.clone()));
    }
    state.playing = Some(Playing {
        meta_id,
        resumed: false,
        stream: Some(Box::new(saved)),
    });
    effects.extend(start_playback(
        state,
        PlayRequest {
            start_ms,
            ..request
        },
        torrent,
        subtitles,
    ));
    effects
}

fn resume(state: &mut State, meta_id: &str) -> Vec<Effect> {
    let Some(item) = state.library.iter().find(|i| i.id == meta_id) else {
        return Vec::new();
    };
    let source = item
        .stream
        .as_ref()
        .map(|saved| resolve(&state.settings, &saved.stream));
    let (Some(saved), Some(Source::Play(url, torrent))) = (item.stream.as_deref().cloned(), source)
    else {
        return open_detail(state, item.content_type.clone(), item.id.clone(), None);
    };
    let video_id = item.video_id.clone();
    let addon_name = addon_name(state, &saved.addon);
    let subtitles = subtitle_groups(
        state,
        &item.content_type,
        Some((&saved.addon, &addon_name)),
        &saved.stream,
        &video_id,
    );
    let request = PlayRequest {
        url,
        title: saved.title,
        logo: saved.logo,
        background: saved.background,
        headers: if torrent.is_some() {
            Vec::new()
        } else {
            saved.stream.request_headers
        },
        settings: state.settings,
        start_ms: item.resume_ms(&video_id, state.settings.watched_at),
        meta_id: item.id.clone(),
        video_id,
    };
    state.playing = Some(Playing {
        meta_id: item.id.clone(),
        resumed: true,
        stream: None,
    });
    start_playback(state, request, torrent, subtitles)
}

/// Ends the current playback. A resumed stream that failed before it
/// progressed opens the detail page; an item that never played and is not a
/// favourite is forgotten.
fn playback_ended(state: &mut State, failure: Option<String>) -> Vec<Effect> {
    let playing = state.playing.take();
    let mut effects = Vec::new();
    if let Some(playing) = &playing
        && let Some(index) = state
            .library
            .iter()
            .position(|i| i.id == playing.meta_id && !i.was_played() && !i.is_favorite())
    {
        state.library.remove(index);
        effects.push(Effect::DeleteLibraryItem(playing.meta_id.clone()));
    }
    let Some(reason) = failure else {
        return effects;
    };
    let resumed = playing
        .filter(|p| p.resumed)
        .and_then(|p| state.library.iter().find(|i| i.id == p.meta_id))
        .map(|i| (i.content_type.clone(), i.id.clone()));
    let Some((content_type, id)) = resumed else {
        state.notice = Some(reason);
        return effects;
    };
    state.notice = Some(format!(
        "The last stream could not be played, pick another one. {reason}"
    ));
    effects.extend(open_detail(state, content_type, id, None));
    effects
}

fn start_playback(
    state: &mut State,
    request: PlayRequest,
    torrent: Option<TorrentRequest>,
    subtitles: Vec<SubtitleGroup>,
) -> Vec<Effect> {
    let mut effects = Vec::new();
    let fetch_subtitles: Vec<Effect> = subtitles
        .iter()
        .filter_map(|g| {
            g.path.clone().map(|path| Effect::FetchSubtitles {
                addon: g.addon.clone(),
                path,
            })
        })
        .collect();
    state.subtitles = subtitles;
    match torrent {
        None => {
            effects.extend(stop_torrent(state));
            effects.push(Effect::Play(Box::new(request)));
        }
        Some(torrent) => {
            state.torrent = Some(TorrentPlayback {
                info_hash: torrent.info_hash.clone(),
                status: TorrentStatus::Starting,
                pending: Some(request),
            });
            effects.push(Effect::StartTorrent(torrent));
        }
    }
    effects.extend(fetch_subtitles);
    effects
}

/// The stream's own subtitles plus one request per subtitles addon, with
/// the stream's `videoHash`, `videoSize` and `filename` hints as extras in
/// the reference client's order.
fn subtitle_groups(
    state: &State,
    content_type: &ContentType,
    source: Option<(&TransportUrl, &str)>,
    stream: &Stream,
    video_id: &str,
) -> Vec<SubtitleGroup> {
    let mut groups = Vec::new();
    if let Some((addon, addon_name)) = source
        && !stream.subtitles.is_empty()
    {
        let mut subtitles = stream.subtitles.clone();
        dedup_by_url(&mut subtitles);
        groups.push(SubtitleGroup {
            addon: addon.clone(),
            addon_name: addon_name.to_owned(),
            path: None,
            subtitles: Loadable::Ready(subtitles),
        });
    }
    let extra: Vec<ExtraValue> = [
        ("videoHash", stream.video_hash.clone()),
        ("videoSize", stream.video_size.map(|s| s.to_string())),
        ("filename", stream.filename.clone()),
    ]
    .into_iter()
    .filter_map(|(name, value)| value.map(|v| ExtraValue::new(name, v)))
    .collect();
    groups.extend(
        plan::subtitle_targets(&state.addons, content_type, video_id)
            .into_iter()
            .map(|(addon, path)| SubtitleGroup {
                addon_name: addon_name(state, &addon),
                addon,
                path: Some(path.with_extra(extra.clone())),
                subtitles: Loadable::Loading,
            }),
    );
    groups
}

fn addon_name(state: &State, addon: &TransportUrl) -> String {
    state
        .addons
        .iter()
        .find(|a| a.transport == *addon)
        .map(|a| a.manifest.name.clone())
        .unwrap_or_default()
}

fn dedup_by_url(subtitles: &mut Vec<Subtitle>) {
    let mut seen = std::collections::HashSet::new();
    subtitles.retain(|s| seen.insert(s.url.clone()));
}
