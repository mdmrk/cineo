use cineo_core::addon::{
    AddonDescriptor, Meta, MetaPreview, PosterShape, SourceKind, Stream, TransportUrl, Video,
};
use cineo_core::app::{
    AddonList, Detail, InstalledAddon, Language, LibraryItem, Loadable, Notice, PlaybackFailure,
    Problem, Row, SeekStep, Setting, Settings, State, StreamGroup, TorrentStatus, WatchedAt,
    continue_watching,
};

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
    pub connecting: Option<String>,
    pub torrent: Option<TorrentView>,
    pub p2p_prompt: bool,
    pub settings: SettingsView,
    pub notice: Option<NoticeView>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SettingsView {
    pub hardware_decoding: bool,
    pub seek_step_s: u32,
    pub seek_steps_s: Vec<u32>,
    pub audio_language: Option<String>,
    pub subtitle_language: Option<String>,
    pub languages: Vec<String>,
    pub watched_at_percent: u32,
    pub watched_at_choices: Vec<u32>,
    pub allow_private_network: bool,
    pub p2p_enabled: bool,
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

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DetailView {
    pub content_type: String,
    pub id: String,
    pub name: Option<String>,
    pub favorite: bool,
    pub poster: Option<String>,
    pub background: Option<String>,
    pub logo: Option<String>,
    pub description: Option<String>,
    pub release_info: Option<String>,
    pub runtime: Option<String>,
    pub genres: Vec<String>,
    pub imdb_rating: Option<String>,
    pub meta: MetaStatus,
    pub videos: Vec<VideoView>,
    pub selected_video: Option<String>,
    pub streams: Vec<StreamGroupView>,
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

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum MetaStatus {
    Loading,
    Ready,
    Failed { problem: ProblemView },
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct VideoView {
    pub id: String,
    pub title: String,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub released: Option<String>,
    pub thumbnail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct StreamGroupView {
    pub addon_name: String,
    pub items: StreamItems,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum StreamItems {
    Loading,
    Ready { streams: Vec<StreamView> },
    Failed { problem: ProblemView },
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct StreamView {
    pub name: Option<String>,
    pub description: Option<String>,
    pub kind: SourceKindView,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PlaybackView {
    pub title: String,
    pub seek_step_s: u32,
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
    pub version: String,
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
    ConfigurationRequired {
        transport: String,
        configure_url: String,
    },
    InvalidAddonUrl {
        reason: String,
    },
    NoMetaAddon,
    UnsupportedFilter,
    Request {
        reason: String,
    },
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
    pub shape: PosterShapeView,
    pub background: Option<String>,
    pub logo: Option<String>,
    pub description: Option<String>,
    pub release_info: Option<String>,
    pub imdb_rating: Option<String>,
    pub genres: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PosterShapeView {
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
        kind: SourceKindView,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SourceKindView {
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

pub(crate) fn view(state: &State, playing: Option<&str>) -> AppView {
    AppView {
        addons: state.addons.iter().map(addon).collect(),
        addons_loading: u32::try_from(state.addons_loading.len()).unwrap_or(u32::MAX),
        unavailable_addons: state
            .installed
            .iter()
            .filter(|t| {
                !state.addons_loading.contains(t)
                    && !state.addons.iter().any(|a| &a.transport == *t)
            })
            .map(ToString::to_string)
            .collect(),
        addon_lists: state
            .addon_lists
            .iter()
            .flatten()
            .map(|l| addon_list(l, &state.installed))
            .collect(),
        install: state.install.as_ref().map(|install| match install {
            Loadable::Loading => InstallView::Installing,
            Loadable::Ready(name) => InstallView::Installed { name: name.clone() },
            Loadable::Failed(problem) => InstallView::Failed {
                problem: problem_view(problem),
            },
        }),
        board: state.board.iter().map(row).collect(),
        search_query: state.search_query.clone(),
        search: state.search.iter().map(row).collect(),
        continue_watching: continue_watching(&state.library, state.settings.watched_at)
            .into_iter()
            .map(library_item)
            .collect(),
        favorites: shelf(state, LibraryItem::is_favorite, |i| {
            i.favorited.unwrap_or(0)
        }),
        recent: shelf(state, LibraryItem::was_played, |i| i.updated_ms),
        detail: state.detail.as_ref().map(|d| detail(d, &state.library)),
        playback: playing.map(|title| PlaybackView {
            title: title.to_owned(),
            seek_step_s: number(Setting::SeekStep(state.settings.seek_step)),
        }),
        connecting: state
            .torrent
            .as_ref()
            .and_then(|t| t.connecting())
            .map(|request| request.title.clone()),
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
    }
}

fn settings(settings: &Settings) -> SettingsView {
    let code = |language: Option<Language>| language.map(|l| l.code().to_owned());
    SettingsView {
        hardware_decoding: settings.hardware_decoding,
        seek_step_s: number(Setting::SeekStep(settings.seek_step)),
        seek_steps_s: SeekStep::ALL
            .iter()
            .map(|s| number(Setting::SeekStep(*s)))
            .collect(),
        audio_language: code(settings.audio_language),
        subtitle_language: code(settings.subtitle_language),
        languages: Language::all().map(|l| l.code().to_owned()).collect(),
        watched_at_percent: number(Setting::WatchedAt(settings.watched_at)),
        watched_at_choices: WatchedAt::ALL
            .iter()
            .map(|w| number(Setting::WatchedAt(*w)))
            .collect(),
        allow_private_network: settings.allow_private_network,
        p2p_enabled: settings.p2p_enabled,
    }
}

fn number(setting: Setting) -> u32 {
    setting.value().parse().unwrap_or_default()
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

fn detail(detail: &Detail, library: &[LibraryItem]) -> DetailView {
    let meta = detail.meta.ready();
    let preview = meta.map(|m| &m.preview).or(detail.preview.as_ref());
    let text = |pick: fn(&MetaPreview) -> Option<&String>| preview.and_then(pick).cloned();
    let url = |pick: fn(&MetaPreview) -> Option<&url::Url>| {
        preview.and_then(pick).map(ToString::to_string)
    };
    DetailView {
        content_type: detail.content_type.to_string(),
        id: detail.id.clone(),
        name: preview.map(|p| p.name.clone()),
        favorite: library.iter().any(|i| i.id == detail.id && i.is_favorite()),
        poster: url(|p| p.poster.as_ref()),
        background: url(|p| p.background.as_ref()),
        logo: url(|p| p.logo.as_ref()),
        description: text(|p| p.description.as_ref()),
        release_info: text(|p| p.release_info.as_ref()),
        runtime: meta.and_then(|m: &Meta| m.runtime.clone()),
        genres: preview.map(|p| p.genres.clone()).unwrap_or_default(),
        imdb_rating: text(|p| p.imdb_rating.as_ref()),
        meta: match &detail.meta {
            Loadable::Loading => MetaStatus::Loading,
            Loadable::Ready(_) => MetaStatus::Ready,
            Loadable::Failed(problem) => MetaStatus::Failed {
                problem: problem_view(problem),
            },
        },
        videos: meta
            .map(|m| m.videos.iter().map(video).collect())
            .unwrap_or_default(),
        selected_video: detail.selected_video.clone(),
        streams: detail.streams.iter().map(stream_group).collect(),
    }
}

fn video(video: &Video) -> VideoView {
    VideoView {
        id: video.id.clone(),
        title: video.title.clone(),
        season: video.season,
        episode: video.episode,
        released: video.released.as_deref().map(day),
        thumbnail: video.thumbnail.as_ref().map(ToString::to_string),
    }
}

fn day(released: &str) -> String {
    let date = released.split('T').next().unwrap_or(released);
    let is_day = date.len() == 10
        && date.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        });
    if is_day { date } else { released }.to_owned()
}

fn stream_group(group: &StreamGroup) -> StreamGroupView {
    StreamGroupView {
        addon_name: group.addon_name.clone(),
        items: match &group.streams {
            Loadable::Loading => StreamItems::Loading,
            Loadable::Ready(streams) => StreamItems::Ready {
                streams: streams.iter().map(stream).collect(),
            },
            Loadable::Failed(problem) => StreamItems::Failed {
                problem: problem_view(problem),
            },
        },
    }
}

fn stream(stream: &Stream) -> StreamView {
    StreamView {
        name: stream.name.clone(),
        description: stream.description.clone(),
        kind: source_kind(stream.source.kind()),
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
        key: format!("{} {}", target.addon, target.path.to_url_path()),
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
        version: manifest.version.to_string(),
        description: manifest.description.clone(),
        logo: manifest.logo.as_ref().map(ToString::to_string),
        installed: installed.contains(&addon.transport),
        needs_setup: manifest.behavior_hints.configuration_required,
        configure_url: addon.transport.configure_url().to_string(),
    }
}

fn row(row: &Row) -> RowView {
    let target = &row.target;
    RowView {
        key: format!("{} {}", target.addon, target.path.to_url_path()),
        addon_name: target.addon_name.clone(),
        name: target.name.clone(),
        content_type: target.path.content_type.to_string(),
        items: match &row.items {
            Loadable::Loading => RowItems::Loading,
            Loadable::Ready(items) => RowItems::Ready {
                items: items.iter().map(poster).collect(),
            },
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
        shape: match meta.poster_shape {
            PosterShape::Poster => PosterShapeView::Poster,
            PosterShape::Square => PosterShapeView::Square,
            PosterShape::Landscape => PosterShapeView::Landscape,
        },
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
            transport: transport.to_string(),
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
        Notice::UnsupportedSource(kind) => NoticeView::UnsupportedSource {
            kind: source_kind(*kind),
        },
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

fn source_kind(kind: SourceKind) -> SourceKindView {
    match kind {
        SourceKind::Http => SourceKindView::Http,
        SourceKind::OtherUrl => SourceKindView::OtherUrl,
        SourceKind::YouTube => SourceKindView::YouTube,
        SourceKind::Torrent => SourceKindView::Torrent,
        SourceKind::External => SourceKindView::External,
        SourceKind::Archive => SourceKindView::Archive,
        SourceKind::Usenet => SourceKindView::Usenet,
    }
}

#[cfg(test)]
mod tests {
    use super::day;

    #[test]
    fn release_times_show_only_their_day() {
        assert_eq!(day("2025-07-14T04:00:00.000Z"), "2025-07-14");
        assert_eq!(day("2025-07-14"), "2025-07-14");
        assert_eq!(day("July 2025"), "July 2025");
        assert_eq!(day("2025T"), "2025T");
    }
}
