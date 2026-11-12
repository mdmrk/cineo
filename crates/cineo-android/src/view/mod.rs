use std::collections::HashSet;

use cineo_core::addon::{AddonDescriptor, MetaPreview, PosterShape, SourceKind, TransportUrl};
use cineo_core::app::{
    AddonList, CatalogTarget, HideControls, InstalledAddon, LibraryItem, Loadable, NextVideoNotice,
    Notice, PlayRequest, PlaybackFailure, Problem, Row, State, TorrentStatus, continue_watching,
};

mod detail;
mod settings;

use detail::detail;
pub use detail::{
    DetailView, MetaStatus, QualityView, StreamFilterView, StreamGroupView, StreamItems,
    StreamView, VideoView,
};
use settings::settings;
pub use settings::{ChoiceView, RangeView, SettingsView, ToggleView};

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct AppView {
    pub addons: Vec<AddonView>,
    pub addons_loading: u32,
    pub unavailable_addons: Vec<String>,
    pub addon_lists: Vec<AddonListView>,
    pub install: Option<InstallView>,
    pub board: Vec<RowView>,
    pub search_query: String,
    pub search: Vec<RowView>,
    pub continue_watching: Vec<LibraryItemView>,
    pub favorites: Vec<LibraryItemView>,
    pub recent: Vec<LibraryItemView>,
    pub detail: Option<DetailView>,
    pub playback: Option<PlaybackView>,
    pub connecting: Option<ArtView>,
    pub torrent: Option<TorrentView>,
    pub p2p_prompt: bool,
    pub settings: SettingsView,
    pub notice: Option<NoticeView>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum TorrentView {
    Starting,
    Streaming {
        peers: u32,
        download_bytes_per_sec: u64,
        downloaded: u64,
        size: u64,
    },
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct LibraryItemView {
    pub id: String,
    pub content_type: String,
    pub name: String,
    pub poster: Option<String>,
    pub progress: f32,
    pub favorite: bool,
    pub played: bool,
    pub resumable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PlaybackView {
    pub art: ArtView,
    pub seek_step_s: u32,
    pub hide_controls_ms: u32,
    pub next: Option<NextView>,
    pub binge_watching: bool,
    pub next_notice_s: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NextView {
    pub title: String,
    pub image: Option<String>,
    pub ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ArtView {
    pub title: String,
    pub logo: Option<String>,
    pub background: Option<String>,
}

impl ArtView {
    pub(crate) fn of(request: &PlayRequest) -> Self {
        Self {
            title: request.title.clone(),
            logo: request.logo.as_ref().map(ToString::to_string),
            background: request.background.as_ref().map(ToString::to_string),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AddonView {
    pub transport: String,
    pub name: String,
    pub version: String,
    pub logo: Option<String>,
    pub configure_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AddonListView {
    pub key: String,
    pub name: String,
    pub addon_name: String,
    pub items: AddonListItems,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum AddonListItems {
    Loading,
    Ready { addons: Vec<ListedAddonView> },
    Failed { problem: ProblemView },
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ListedAddonView {
    pub transport: String,
    pub name: String,
    pub description: Option<String>,
    pub logo: Option<String>,
    pub installed: bool,
    pub needs_setup: bool,
    pub configure_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum InstallView {
    Installing,
    Installed { name: String },
    Failed { problem: ProblemView },
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum ProblemView {
    AlreadyInstalled,
    ConfigurationRequired { configure_url: String },
    InvalidAddonUrl { reason: String },
    NoMetaAddon,
    UnsupportedFilter,
    Request { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RowView {
    pub key: String,
    pub addon_name: String,
    pub name: String,
    pub content_type: String,
    pub items: RowItems,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum RowItems {
    Loading,
    Ready { items: Vec<PosterView> },
    Failed { problem: ProblemView },
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PosterView {
    pub id: String,
    pub content_type: String,
    pub name: String,
    pub poster: Option<String>,
    pub shape: PosterShape,
    pub background: Option<String>,
    pub logo: Option<String>,
    pub description: Option<String>,
    pub release_info: Option<String>,
    pub imdb_rating: Option<String>,
    pub genres: Vec<String>,
}

#[uniffi::remote(Enum)]
pub enum PosterShape {
    Poster,
    Square,
    Landscape,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum NoticeView {
    AddonUnavailable {
        reason: String,
    },
    UnsupportedScheme {
        scheme: String,
    },
    TorrentsOff,
    UnsupportedSource {
        kind: SourceKind,
    },
    PlaybackFailed {
        failure: PlaybackFailureView,
        pick_another: bool,
    },
    UnexpectedEngineAddress,
    SubtitleFailed {
        reason: String,
    },
    SaveFailed {
        reason: String,
    },
    StoreClosed,
    InvalidLink,
}

#[uniffi::remote(Enum)]
pub enum SourceKind {
    Http,
    OtherUrl,
    YouTube,
    Torrent,
    External,
    Archive,
    Usenet,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum PlaybackFailureView {
    Player { reason: String },
    Torrent { reason: String },
}

pub(crate) fn view(state: &State, playing: Option<&ArtView>, filter: &StreamFilterView) -> AppView {
    let mut view = AppView {
        addons: Vec::new(),
        addons_loading: 0,
        unavailable_addons: Vec::new(),
        addon_lists: Vec::new(),
        install: None,
        board: Vec::new(),
        search_query: String::new(),
        search: Vec::new(),
        continue_watching: Vec::new(),
        favorites: Vec::new(),
        recent: Vec::new(),
        detail: None,
        playback: playing.map(|art| PlaybackView {
            art: art.clone(),
            seek_step_s: state.settings.seek_step.whole_seconds(),
            hide_controls_ms: hide_controls_ms(state.settings.hide_controls),
            next: state
                .playing
                .as_ref()
                .and_then(|p| p.next.as_ref())
                .map(|n| NextView {
                    title: n.title.clone(),
                    image: n.image.as_ref().map(ToString::to_string),
                    ready: n.stream.is_some(),
                }),
            binge_watching: state.settings.binge_watching,
            next_notice_s: next_notice_s(state.settings.next_video_notice),
        }),
        connecting: state
            .torrent
            .as_ref()
            .and_then(|t| t.connecting())
            .map(ArtView::of),
        torrent: state.torrent.as_ref().map(|t| match t.status {
            TorrentStatus::Starting => TorrentView::Starting,
            TorrentStatus::Streaming {
                peers,
                download_bytes_per_sec,
                downloaded,
                size,
            } => TorrentView::Streaming {
                peers,
                download_bytes_per_sec,
                downloaded,
                size,
            },
        }),
        p2p_prompt: state.p2p_prompt.is_some(),
        settings: settings(&state.settings),
        notice: state.notice.as_ref().map(notice_view),
    };
    if view.playback.is_none() && view.connecting.is_none() {
        fill_browsing(&mut view, state, filter);
    }
    view
}

fn fill_browsing(view: &mut AppView, state: &State, filter: &StreamFilterView) {
    view.addons = state.addons.iter().map(addon).collect();
    view.addons_loading = u32::try_from(state.addons_loading.len()).unwrap_or(u32::MAX);
    view.unavailable_addons = state
        .installed
        .iter()
        .filter(|t| {
            !state.addons_loading.contains(t) && !state.addons.iter().any(|a| &a.transport == *t)
        })
        .map(ToString::to_string)
        .collect();
    view.addon_lists = state
        .addon_lists
        .iter()
        .flatten()
        .map(|l| addon_list(l, &state.installed))
        .collect();
    view.install = state.install.as_ref().map(|install| match install {
        Loadable::Loading => InstallView::Installing,
        Loadable::Ready(name) => InstallView::Installed { name: name.clone() },
        Loadable::Failed(problem) => InstallView::Failed {
            problem: problem_view(problem),
        },
    });
    view.board = state.board.iter().map(row).collect();
    view.search_query.clone_from(&state.search_query);
    view.search = state.search.iter().map(row).collect();
    view.continue_watching = continue_watching(&state.library, state.settings.watched_at)
        .into_iter()
        .map(library_item)
        .collect();
    view.favorites = shelf(state, LibraryItem::is_favorite, |i| {
        i.favorited.unwrap_or(0)
    });
    view.recent = shelf(state, LibraryItem::was_played, |i| i.updated_ms);
    view.detail = state
        .detail
        .as_ref()
        .map(|d| detail(d, &state.library, filter, state.settings.p2p_enabled));
}

fn hide_controls_ms(hide: HideControls) -> u32 {
    let ms = std::time::Duration::from_secs_f64(hide.seconds()).as_millis();
    u32::try_from(ms).unwrap_or(u32::MAX)
}

fn next_notice_s(notice: NextVideoNotice) -> u32 {
    std::time::Duration::from_secs_f64(notice.seconds())
        .as_secs()
        .try_into()
        .unwrap_or(u32::MAX)
}

fn shelf(
    state: &State,
    keep: fn(&LibraryItem) -> bool,
    newest: fn(&LibraryItem) -> u64,
) -> Vec<LibraryItemView> {
    let mut items: Vec<&LibraryItem> = state.library.iter().filter(|i| keep(i)).collect();
    items.sort_by_key(|i| std::cmp::Reverse(newest(i)));
    items.into_iter().map(library_item).collect()
}

fn library_item(item: &LibraryItem) -> LibraryItemView {
    LibraryItemView {
        id: item.id.clone(),
        content_type: item.content_type.to_string(),
        name: item.name.clone(),
        poster: item.poster.as_ref().map(ToString::to_string),
        progress: item.progress(),
        favorite: item.is_favorite(),
        played: item.was_played(),
        resumable: item.stream.is_some(),
    }
}

fn addon(addon: &InstalledAddon) -> AddonView {
    AddonView {
        transport: addon.transport.to_string(),
        name: addon.manifest.name.clone(),
        version: addon.manifest.version.to_string(),
        logo: addon.manifest.logo.as_ref().map(ToString::to_string),
        configure_url: addon
            .manifest
            .behavior_hints
            .configurable
            .then(|| addon.transport.configure_url().to_string()),
    }
}

fn addon_list(list: &AddonList, installed: &[TransportUrl]) -> AddonListView {
    let target = &list.target;
    AddonListView {
        key: key(target),
        name: target.name.clone(),
        addon_name: target.addon_name.clone(),
        items: match &list.addons {
            Loadable::Loading => AddonListItems::Loading,
            Loadable::Ready(addons) => AddonListItems::Ready {
                addons: addons.iter().map(|a| listed_addon(a, installed)).collect(),
            },
            Loadable::Failed(problem) => AddonListItems::Failed {
                problem: problem_view(problem),
            },
        },
    }
}

fn listed_addon(addon: &AddonDescriptor, installed: &[TransportUrl]) -> ListedAddonView {
    let manifest = &addon.manifest;
    ListedAddonView {
        transport: addon.transport.to_string(),
        name: manifest.name.clone(),
        description: manifest.description.clone(),
        logo: manifest.logo.as_ref().map(ToString::to_string),
        installed: installed.contains(&addon.transport),
        needs_setup: manifest.behavior_hints.configuration_required,
        configure_url: addon.transport.configure_url().to_string(),
    }
}

fn key(target: &CatalogTarget) -> String {
    format!("{} {}", target.addon, target.path.to_url_path())
}

fn row(row: &Row) -> RowView {
    let target = &row.target;
    RowView {
        key: key(target),
        addon_name: target.addon_name.clone(),
        name: target.name.clone(),
        content_type: target.path.content_type.to_string(),
        items: match &row.items {
            Loadable::Loading => RowItems::Loading,
            Loadable::Ready(items) => {
                let mut seen = HashSet::new();
                RowItems::Ready {
                    items: items
                        .iter()
                        .filter(|item| seen.insert(item.id.as_str()))
                        .map(poster)
                        .collect(),
                }
            }
            Loadable::Failed(problem) => RowItems::Failed {
                problem: problem_view(problem),
            },
        },
    }
}

fn poster(meta: &MetaPreview) -> PosterView {
    PosterView {
        id: meta.id.clone(),
        content_type: meta.content_type.to_string(),
        name: meta.name.clone(),
        poster: meta.poster.as_ref().map(ToString::to_string),
        shape: meta.poster_shape,
        background: meta.background.as_ref().map(ToString::to_string),
        logo: meta.logo.as_ref().map(ToString::to_string),
        description: meta.description.clone(),
        release_info: meta.release_info.clone(),
        imdb_rating: meta.imdb_rating.clone(),
        genres: meta.genres.clone(),
    }
}

fn problem_view(problem: &Problem) -> ProblemView {
    match problem {
        Problem::AlreadyInstalled => ProblemView::AlreadyInstalled,
        Problem::ConfigurationRequired(transport) => ProblemView::ConfigurationRequired {
            configure_url: transport.configure_url().to_string(),
        },
        Problem::InvalidAddonUrl(reason) => ProblemView::InvalidAddonUrl {
            reason: reason.clone(),
        },
        Problem::NoMetaAddon => ProblemView::NoMetaAddon,
        Problem::UnsupportedFilter => ProblemView::UnsupportedFilter,
        Problem::Request(reason) => ProblemView::Request {
            reason: reason.clone(),
        },
    }
}

fn notice_view(notice: &Notice) -> NoticeView {
    match notice {
        Notice::AddonUnavailable(reason) => NoticeView::AddonUnavailable {
            reason: reason.clone(),
        },
        Notice::UnsupportedScheme(scheme) => NoticeView::UnsupportedScheme {
            scheme: scheme.clone(),
        },
        Notice::TorrentsOff => NoticeView::TorrentsOff,
        Notice::UnsupportedSource(kind) => NoticeView::UnsupportedSource { kind: *kind },
        Notice::PlaybackFailed {
            failure,
            pick_another,
        } => NoticeView::PlaybackFailed {
            failure: match failure {
                PlaybackFailure::Player(reason) => PlaybackFailureView::Player {
                    reason: reason.clone(),
                },
                PlaybackFailure::Torrent(reason) => PlaybackFailureView::Torrent {
                    reason: reason.clone(),
                },
            },
            pick_another: *pick_another,
        },
        Notice::UnexpectedEngineAddress => NoticeView::UnexpectedEngineAddress,
        Notice::SubtitleFailed(reason) => NoticeView::SubtitleFailed {
            reason: reason.clone(),
        },
        Notice::SaveFailed(reason) => NoticeView::SaveFailed {
            reason: reason.clone(),
        },
        Notice::StoreClosed => NoticeView::StoreClosed,
        Notice::InvalidLink => NoticeView::InvalidLink,
    }
}
