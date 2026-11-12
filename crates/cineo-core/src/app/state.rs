use semver::Version;
use url::Url;

use super::library::{LibraryItem, SavedStream};
use super::link::Route;
use super::message::{Notice, PlaybackFailure, Problem};
use super::plan::{self, CatalogTarget};
use super::playback::{check_next, dedup_by_url, play, play_next, playback_ended, resume};
use super::refresh::{
    addons_changed, catalog_loaded, default_genre, discover_page, load_board, loading_rows,
    manifest_loaded, meta_loaded, open_detail, refresh_addon_lists, retry_failed, select_video,
};
use super::release;
use super::settings::{Setting, Settings};
use super::torrent::{TorrentPlayback, TorrentRequest, TorrentStatus, is_engine_url};
use crate::addon::{
    AddonDescriptor, ContentType, Manifest, Meta, MetaPreview, ResourcePath, Stream, Subtitle,
    TransportUrl, Video,
};

#[derive(Debug, Clone, PartialEq)]
pub struct InstalledAddon {
    pub transport: TransportUrl,
    pub manifest: Manifest,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Loadable<T> {
    Loading,
    Ready(T),
    Failed(Problem),
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

    pub(crate) fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }
}

impl<T> From<Result<T, String>> for Loadable<T> {
    fn from(result: Result<T, String>) -> Self {
        match result {
            Ok(value) => Self::Ready(value),
            Err(err) => Self::Failed(Problem::Request(err)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub target: CatalogTarget,
    pub items: Loadable<Vec<MetaPreview>>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Discover {
    pub target: Option<CatalogTarget>,
    pub genre: Option<String>,
    pub items: Vec<MetaPreview>,
    pub pending: Option<ResourcePath>,
    pub error: Option<Problem>,
    pub next_skip: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AddonList {
    pub target: CatalogTarget,
    pub addons: Loadable<Vec<AddonDescriptor>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StreamGroup {
    pub addon: TransportUrl,
    pub addon_name: String,
    pub path: ResourcePath,
    pub streams: Loadable<Vec<Stream>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Detail {
    pub content_type: ContentType,
    pub id: String,
    pub preview: Option<MetaPreview>,
    pub meta: Loadable<Meta>,
    pub meta_fallbacks: Vec<(TransportUrl, ResourcePath)>,
    pub meta_request: Option<(TransportUrl, ResourcePath)>,
    pub selected_video: Option<String>,
    pub streams: Vec<StreamGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayRequest {
    pub url: Url,
    pub title: String,
    pub logo: Option<Url>,
    pub background: Option<Url>,
    pub headers: Vec<(String, String)>,
    pub settings: Settings,
    pub start_ms: u64,
    pub meta_id: String,
    pub video_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleGroup {
    pub addon: TransportUrl,
    pub addon_name: String,
    pub path: Option<ResourcePath>,
    pub subtitles: Loadable<Vec<Subtitle>>,
}

pub fn find_subtitle(groups: &[SubtitleGroup], url: &Url) -> Option<(String, String)> {
    groups.iter().find_map(|group| {
        let subtitle = group.subtitles.ready()?.iter().find(|s| s.url == *url)?;
        let source = subtitle.label.as_deref().unwrap_or(&group.addon_name);
        Some((source.to_owned(), subtitle.lang.clone()))
    })
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct State {
    pub installed: Vec<TransportUrl>,
    pub addons: Vec<InstalledAddon>,
    pub addons_loading: Vec<TransportUrl>,
    pub install: Option<Loadable<String>>,
    pub addon_lists: Option<Vec<AddonList>>,
    pub board: Vec<Row>,
    pub discover: Discover,
    pub search_query: String,
    pub search: Vec<Row>,
    pub detail: Option<Detail>,
    pub library: Vec<LibraryItem>,
    pub notice: Option<Notice>,
    pub settings: Settings,
    pub default_settings: Settings,
    pub p2p_prompt: Option<(usize, usize)>,
    pub link_prompt: Option<TransportUrl>,
    pub torrent: Option<TorrentPlayback>,
    pub subtitles: Vec<SubtitleGroup>,
    pub playing: Option<Playing>,
    pub update: Option<Version>,
    pub update_checked: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Playing {
    pub meta_id: String,
    pub resumed: bool,
    pub stream: Option<Box<SavedStream>>,
    pub content_type: ContentType,
    pub video_id: String,
    pub addon: TransportUrl,
    pub binge_group: Option<String>,
    pub meta: Option<Meta>,
    pub meta_request: Option<(TransportUrl, ResourcePath)>,
    pub next: Option<NextVideo>,
    pub next_checked: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NextVideo {
    pub video: Video,
    pub title: String,
    pub image: Option<Url>,
    pub path: Option<ResourcePath>,
    pub stream: Option<Stream>,
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
    RetryFailed,
    LoadAddonLists,
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
    PlaybackEnded,
    PlayNext,
    RemoveFromLibrary(String),
    SetFavorite {
        id: String,
        favorite: bool,
    },
    ClearLibrary,
    DismissNotice,
    AcceptP2p,
    DeclineP2p,
    OpenLink(Route),
    AcceptLink,
    DeclineLink,
    ChangeSetting(Setting),
    ResetSettings,
    LatestReleaseLoaded(Result<Version, String>),
    ManifestLoaded {
        transport: TransportUrl,
        result: Result<Box<Manifest>, String>,
        origin: ManifestOrigin,
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
    AddonListLoaded {
        addon: TransportUrl,
        path: ResourcePath,
        result: Result<Vec<AddonDescriptor>, String>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestOrigin {
    Restore,
    Install,
    Retry,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    FetchManifest {
        transport: TransportUrl,
        origin: ManifestOrigin,
    },
    FetchCatalog {
        addon: TransportUrl,
        path: ResourcePath,
    },
    FetchMeta {
        addon: TransportUrl,
        path: ResourcePath,
    },
    FetchAddonList {
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
    CheckForUpdate(Url),
}

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
                    origin: ManifestOrigin::Restore,
                })
                .collect()
        }
        Action::RestoreSettings(settings) => {
            state.settings = settings;
            check_for_update(state).into_iter().collect()
        }
        Action::InstallAddon(input) => match TransportUrl::parse(&input) {
            Ok(transport)
                if state.addons.iter().any(|a| a.transport == transport)
                    || state.addons_loading.contains(&transport) =>
            {
                state.install = Some(Loadable::Failed(Problem::AlreadyInstalled));
                Vec::new()
            }
            Ok(transport) => {
                state.install = Some(Loadable::Loading);
                vec![Effect::FetchManifest {
                    transport,
                    origin: ManifestOrigin::Install,
                }]
            }
            Err(err) => {
                state.install = Some(Loadable::Failed(Problem::InvalidAddonUrl(err.to_string())));
                Vec::new()
            }
        },
        Action::RemoveAddon(transport) => {
            state.installed.retain(|t| *t != transport);
            state.addons_loading.retain(|t| *t != transport);
            state.addons.retain(|a| a.transport != transport);
            addons_changed(state, true)
        }
        Action::MoveAddon { from, to } => move_addon(state, from, to),
        Action::LoadBoard => load_board(state),
        Action::RetryFailed => retry_failed(state),
        Action::LoadAddonLists => {
            state.addon_lists.get_or_insert_default();
            refresh_addon_lists(state, true)
        }
        Action::OpenDiscover { addon, path } => {
            let target = plan::catalogs(&state.addons, |_| true)
                .map(|(a, c)| plan::catalog_target(a, c))
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
            let effects;
            (state.search, effects) = loading_rows(plan::search_targets(&state.addons, &query));
            effects
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
        } => playback_progress(state, &meta_id, video_id, time_ms, duration_ms, now_ms),
        Action::PlaybackFailed(reason) => {
            state.subtitles.clear();
            playback_ended(state, Some(PlaybackFailure::Player(reason)))
        }
        Action::PlaybackStopped => {
            state.subtitles.clear();
            playback_ended(state, None)
        }
        Action::PlaybackEnded => {
            state.subtitles.clear();
            if state.settings.binge_watching {
                play_next(state)
            } else {
                playback_ended(state, None)
            }
        }
        Action::PlayNext => {
            state.subtitles.clear();
            play_next(state)
        }
        Action::SubtitlesLoaded {
            addon,
            path,
            result,
        } => {
            subtitles_loaded(state, &addon, &path, result);
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
        Action::OpenLink(route) => open_link(state, route),
        Action::AcceptLink => match state.link_prompt.take() {
            Some(transport) => update(state, Action::InstallAddon(transport.to_string())),
            None => Vec::new(),
        },
        Action::DeclineLink => {
            state.link_prompt = None;
            Vec::new()
        }
        Action::ChangeSetting(setting) => {
            state.settings.set(setting);
            let mut effects = settings_changed(state);
            if setting == Setting::CheckForUpdates(true) {
                effects.extend(check_for_update(state));
            }
            effects
        }
        Action::ResetSettings => {
            let was_checking = state.settings.check_for_updates;
            state.settings = state.settings.reset(state.default_settings);
            let mut effects = settings_changed(state);
            if !was_checking {
                effects.extend(check_for_update(state));
            }
            effects
        }
        Action::LatestReleaseLoaded(result) => {
            state.update = result.ok().filter(release::is_newer);
            Vec::new()
        }
        Action::SetFavorite { id, favorite } => set_favorite(state, id, favorite),
        Action::RemoveFromLibrary(id) => remove_from_library(state, id),
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
            origin,
        } => manifest_loaded(state, &transport, result, origin),
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
        Action::AddonListLoaded {
            addon,
            path,
            result,
        } => {
            addon_list_loaded(state, &addon, &path, result);
            Vec::new()
        }
        Action::StreamsLoaded {
            addon,
            path,
            result,
        } => {
            streams_loaded(state, &addon, &path, result);
            Vec::new()
        }
        Action::TorrentReady { info_hash, url } => torrent_ready(state, &info_hash, url),
        Action::TorrentStatus { info_hash, status } => {
            let Some(torrent) = state.torrent.as_mut().filter(|t| t.info_hash == info_hash) else {
                return Vec::new();
            };
            torrent.status = status;
            Vec::new()
        }
        Action::TorrentFailed { info_hash, reason } => {
            if state
                .torrent
                .as_ref()
                .is_some_and(|t| t.info_hash == info_hash)
            {
                playback_ended(state, Some(PlaybackFailure::Torrent(reason)))
            } else {
                Vec::new()
            }
        }
    }
}

fn move_addon(state: &mut State, from: usize, to: usize) -> Vec<Effect> {
    if from >= state.addons.len() || to >= state.addons.len() || from == to {
        return Vec::new();
    }
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
    addons_changed(state, true)
}

fn playback_progress(
    state: &mut State,
    meta_id: &str,
    video_id: String,
    time_ms: u64,
    duration_ms: u64,
    now_ms: u64,
) -> Vec<Effect> {
    if state
        .playing
        .as_ref()
        .is_some_and(|p| p.meta_id == meta_id && p.video_id != video_id)
    {
        return Vec::new();
    }
    let mut effects = check_next(state, now_ms);
    let mut stream = None;
    if let Some(playing) = state.playing.as_mut().filter(|p| p.meta_id == meta_id) {
        playing.resumed = false;
        stream = playing.stream.take();
    }
    let Some(item) = state.library.iter_mut().find(|i| i.id == meta_id) else {
        return effects;
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
    effects.push(Effect::SaveLibraryItem(item.clone()));
    effects
}

fn subtitles_loaded(
    state: &mut State,
    addon: &TransportUrl,
    path: &ResourcePath,
    result: Result<Vec<Subtitle>, String>,
) {
    if let Some(group) = state
        .subtitles
        .iter_mut()
        .find(|g| g.addon == *addon && g.path.as_ref() == Some(path))
    {
        group.subtitles = result
            .map(|mut subtitles| {
                dedup_by_url(&mut subtitles);
                subtitles
            })
            .into();
    }
}

fn remove_from_library(state: &mut State, id: String) -> Vec<Effect> {
    if let Some(item) = state
        .library
        .iter_mut()
        .find(|i| i.id == id && i.is_favorite())
    {
        item.video_id.clone_from(&item.id);
        item.time_offset_ms = 0;
        item.duration_ms = 0;
        item.updated_ms = 0;
        item.stream = None;
        return vec![Effect::SaveLibraryItem(item.clone())];
    }
    state.library.retain(|i| i.id != id);
    vec![Effect::DeleteLibraryItem(id)]
}

fn addon_list_loaded(
    state: &mut State,
    addon: &TransportUrl,
    path: &ResourcePath,
    result: Result<Vec<AddonDescriptor>, String>,
) {
    if let Some(list) = state
        .addon_lists
        .iter_mut()
        .flatten()
        .find(|l| l.target.addon == *addon && l.target.path == *path)
    {
        list.addons = result
            .map(|mut addons| {
                addons.sort_by_cached_key(|a| a.manifest.name.to_lowercase());
                addons
            })
            .into();
    }
}

fn streams_loaded(
    state: &mut State,
    addon: &TransportUrl,
    path: &ResourcePath,
    result: Result<Vec<Stream>, String>,
) {
    if let Some(playing) = state.playing.as_mut().filter(|p| p.addon == *addon)
        && let Some(next) = playing
            .next
            .as_mut()
            .filter(|n| n.path.as_ref() == Some(path))
        && let Ok(streams) = &result
    {
        next.stream = streams
            .iter()
            .find(|s| s.binge_group.is_some() && s.binge_group == playing.binge_group)
            .cloned();
    }
    if let Some(group) = state.detail.as_mut().and_then(|d| {
        d.streams
            .iter_mut()
            .find(|g| g.addon == *addon && g.path == *path)
    }) {
        group.streams = result.into();
    }
}

fn open_link(state: &mut State, route: Route) -> Vec<Effect> {
    if let Route::InstallAddon(transport) = route {
        state.link_prompt = Some(transport);
        return Vec::new();
    }
    state.detail = None;
    match route {
        Route::Discover(Some(link)) => {
            let mut effects = update(
                state,
                Action::OpenDiscover {
                    addon: link.addon,
                    path: link.path,
                },
            );
            if link.genre.is_some() {
                state.discover.genre = link.genre;
                state.discover.items.clear();
                effects = discover_page(state, 0);
            }
            effects
        }
        Route::Search(query) => update(state, Action::Search(query)),
        Route::Detail {
            content_type,
            id,
            video_id,
        } => {
            let mut effects = open_detail(state, content_type, id, None);
            if let Some(video_id) = video_id {
                effects.extend(select_video(state, video_id));
            }
            effects
        }
        _ => Vec::new(),
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

fn check_for_update(state: &mut State) -> Option<Effect> {
    if !state.settings.check_for_updates || state.update_checked {
        return None;
    }
    state.update_checked = true;
    Url::parse(release::LATEST_RELEASE_URL)
        .ok()
        .map(Effect::CheckForUpdate)
}

pub(super) fn stop_torrent(state: &mut State) -> Vec<Effect> {
    if state.torrent.take().is_some() {
        vec![Effect::StopTorrent]
    } else {
        Vec::new()
    }
}

fn torrent_ready(state: &mut State, info_hash: &str, url: Url) -> Vec<Effect> {
    let Some(torrent) = state.torrent.as_mut().filter(|t| t.info_hash == info_hash) else {
        return Vec::new();
    };
    if !is_engine_url(&url) {
        state.notice = Some(Notice::UnexpectedEngineAddress);
        return stop_torrent(state);
    }
    match torrent.pending.take() {
        Some(request) => vec![Effect::Play(Box::new(PlayRequest { url, ..request }))],
        None => Vec::new(),
    }
}

impl State {
    pub fn available_update(&self) -> Option<&Version> {
        self.update
            .as_ref()
            .filter(|_| self.settings.check_for_updates)
    }

    pub fn unavailable_addons(&self) -> Vec<&TransportUrl> {
        self.installed
            .iter()
            .filter(|t| {
                !self.addons_loading.contains(t) && !self.addons.iter().any(|a| a.transport == **t)
            })
            .collect()
    }
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
