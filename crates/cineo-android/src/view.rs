use cineo_core::addon::{
    AddonDescriptor, Meta, MetaPreview, PosterShape, SourceKind, Stream, TransportUrl, Video,
};
use cineo_core::app::{
    AddonList, CatalogTarget, Detail, DownloadLimit, HideControls, InstalledAddon, Language,
    LibraryItem, Loadable, NextVideoNotice, Notice, PeerLimit, PlayRequest, PlaybackFailure,
    Problem, Ranged, Resolution, Row, SeekStep, Setting, Settings, State, StreamFacts, StreamGroup,
    StreamSort, SubtitleBackground, SubtitleColor, SubtitleFont, SubtitleOutline, Tier,
    TorrentStatus, TorrentStorage, UploadLimit, WatchedAt, continue_watching, stream_matcher,
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
    pub connecting: Option<ArtView>,
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
    pub secondary_audio_language: Option<String>,
    pub secondary_subtitle_language: Option<String>,
    pub watched_at_percent: u32,
    pub watched_at_choices: Vec<u32>,
    pub allow_private_network: bool,
    pub p2p_enabled: bool,
    pub hide_controls: ChoiceView,
    pub binge_watching: ToggleView,
    pub next_video_notice: ChoiceView,
    pub subtitle_size: RangeView,
    pub subtitle_font: ChoiceView,
    pub subtitle_bold: ToggleView,
    pub subtitle_color: ChoiceView,
    pub subtitle_opacity: RangeView,
    pub subtitle_outline: ChoiceView,
    pub subtitle_background: ChoiceView,
    pub subtitle_position: RangeView,
    pub keep_subtitle_styles: ToggleView,
    pub torrent_upload: ToggleView,
    pub torrent_storage: ChoiceView,
    pub download_limit: ChoiceView,
    pub upload_limit: ChoiceView,
    pub peer_limit: ChoiceView,
    pub torrent_dht: ToggleView,
    pub torrent_port: RangeView,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ChoiceView {
    pub key: String,
    pub value: String,
    pub options: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ToggleView {
    pub key: String,
    pub on: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RangeView {
    pub key: String,
    pub value: u32,
    pub min: u32,
    pub max: u32,
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
    pub qualities: Vec<Resolution>,
    pub filter: StreamFilterView,
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

#[uniffi::remote(Enum)]
pub enum StreamSort {
    AddonOrder,
    Quality,
    Seeders,
    Size,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, uniffi::Record)]
pub struct StreamFilterView {
    pub query: String,
    pub quality: Option<Resolution>,
    pub addon: Option<u32>,
    pub sort: StreamSort,
}

#[uniffi::remote(Enum)]
pub enum Resolution {
    Uhd,
    Qhd,
    FullHd,
    Hd,
    Sd,
    Cam,
}

#[uniffi::remote(Enum)]
pub enum Tier {
    Ultra,
    High,
    Standard,
    Low,
    Cam,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct QualityView {
    pub resolution: Resolution,
    pub tier: Tier,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum StreamItems {
    Loading,
    Ready {
        streams: Vec<StreamView>,
        order: Vec<u32>,
        hidden: u32,
    },
    Failed {
        problem: ProblemView,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct StreamView {
    pub name: Option<String>,
    pub description: Option<String>,
    pub kind: SourceKind,
    pub quality: Option<QualityView>,
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

fn settings(settings: &Settings) -> SettingsView {
    let code = |language: Option<Language>| language.map(|l| l.code().to_owned());
    SettingsView {
        hardware_decoding: settings.hardware_decoding,
        seek_step_s: settings.seek_step.whole_seconds(),
        seek_steps_s: SeekStep::ALL.iter().map(|s| s.whole_seconds()).collect(),
        audio_language: code(settings.audio_language),
        subtitle_language: code(settings.subtitle_language),
        languages: Language::all().map(|l| l.code().to_owned()).collect(),
        secondary_audio_language: code(settings.secondary_audio_language),
        secondary_subtitle_language: code(settings.secondary_subtitle_language),
        watched_at_percent: settings.watched_at.percent(),
        watched_at_choices: WatchedAt::ALL.iter().map(|w| w.percent()).collect(),
        allow_private_network: settings.allow_private_network,
        p2p_enabled: settings.p2p_enabled,
        hide_controls: choice(
            settings.hide_controls,
            HideControls::ALL,
            Setting::HideControls,
        ),
        binge_watching: toggle(settings.binge_watching, Setting::BingeWatching),
        next_video_notice: choice(
            settings.next_video_notice,
            NextVideoNotice::ALL,
            Setting::NextVideoNotice,
        ),
        subtitle_size: range(settings.subtitle_size, Setting::SubtitleSize),
        subtitle_font: choice(
            settings.subtitle_font,
            SubtitleFont::ALL,
            Setting::SubtitleFont,
        ),
        subtitle_bold: toggle(settings.subtitle_bold, Setting::SubtitleBold),
        subtitle_color: choice(
            settings.subtitle_color,
            SubtitleColor::ALL,
            Setting::SubtitleColor,
        ),
        subtitle_opacity: range(settings.subtitle_opacity, Setting::SubtitleOpacity),
        subtitle_outline: choice(
            settings.subtitle_outline,
            SubtitleOutline::ALL,
            Setting::SubtitleOutline,
        ),
        subtitle_background: choice(
            settings.subtitle_background,
            SubtitleBackground::ALL,
            Setting::SubtitleBackground,
        ),
        subtitle_position: range(settings.subtitle_position, Setting::SubtitlePosition),
        keep_subtitle_styles: toggle(settings.keep_subtitle_styles, Setting::KeepSubtitleStyles),
        torrent_upload: toggle(settings.torrent_upload, Setting::TorrentUpload),
        torrent_storage: choice(
            settings.torrent_storage,
            TorrentStorage::ALL,
            Setting::TorrentStorage,
        ),
        download_limit: choice(
            settings.download_limit,
            DownloadLimit::ALL,
            Setting::DownloadLimit,
        ),
        upload_limit: choice(
            settings.upload_limit,
            UploadLimit::ALL,
            Setting::UploadLimit,
        ),
        peer_limit: choice(settings.peer_limit, PeerLimit::ALL, Setting::PeerLimit),
        torrent_dht: toggle(settings.torrent_dht, Setting::TorrentDht),
        torrent_port: range(settings.torrent_port, Setting::TorrentPort),
    }
}

fn choice<T: Copy>(current: T, all: &[T], wrap: fn(T) -> Setting) -> ChoiceView {
    ChoiceView {
        key: wrap(current).key().to_owned(),
        value: wrap(current).value(),
        options: all.iter().map(|&option| wrap(option).value()).collect(),
    }
}

fn toggle(on: bool, wrap: fn(bool) -> Setting) -> ToggleView {
    ToggleView {
        key: wrap(on).key().to_owned(),
        on,
    }
}

fn range<const MIN: u32, const MAX: u32>(
    current: Ranged<MIN, MAX>,
    wrap: fn(Ranged<MIN, MAX>) -> Setting,
) -> RangeView {
    RangeView {
        key: wrap(current).key().to_owned(),
        value: current.get(),
        min: MIN,
        max: MAX,
    }
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

fn detail(
    detail: &Detail,
    library: &[LibraryItem],
    filter: &StreamFilterView,
    p2p_enabled: bool,
) -> DetailView {
    let meta = detail.meta.ready();
    let facts: Vec<Option<Vec<StreamFacts>>> = detail
        .streams
        .iter()
        .map(|g| {
            g.streams
                .ready()
                .map(|s| s.iter().map(StreamFacts::of).collect())
        })
        .collect();
    let preview = meta.map(|m| &m.preview).or(detail.preview.as_ref());
    let text = |pick: fn(&MetaPreview) -> Option<&String>| preview.and_then(pick).cloned();
    let url = |pick: fn(&MetaPreview) -> Option<&url::Url>| {
        preview.and_then(pick).map(ToString::to_string)
    };
    DetailView {
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
        streams: detail
            .streams
            .iter()
            .zip(&facts)
            .enumerate()
            .map(|(index, (group, facts))| {
                stream_group(index, group, facts.as_deref(), filter, p2p_enabled)
            })
            .collect(),
        qualities: qualities(&detail.streams, &facts, p2p_enabled),
        filter: filter.clone(),
    }
}

fn qualities(
    groups: &[StreamGroup],
    facts: &[Option<Vec<StreamFacts>>],
    p2p_enabled: bool,
) -> Vec<Resolution> {
    let mut shown: Vec<Resolution> = groups
        .iter()
        .zip(facts)
        .filter_map(|(group, facts)| group.streams.ready().zip(facts.as_ref()))
        .flat_map(|(streams, facts)| streams.iter().zip(facts))
        .filter(|(stream, _)| p2p_enabled || !stream.source.is_p2p())
        .filter_map(|(_, facts)| facts.quality.map(|q| q.resolution))
        .collect();
    shown.sort_unstable();
    shown.dedup();
    shown
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

fn stream_group(
    index: usize,
    group: &StreamGroup,
    facts: Option<&[StreamFacts]>,
    filter: &StreamFilterView,
    p2p_enabled: bool,
) -> StreamGroupView {
    StreamGroupView {
        addon_name: group.addon_name.clone(),
        items: match (&group.streams, facts) {
            (Loadable::Loading, _) | (Loadable::Ready(_), None) => StreamItems::Loading,
            (Loadable::Ready(streams), Some(facts)) => {
                let matches = stream_matcher(&filter.query, filter.quality);
                let other_addon = filter.addon.is_some_and(|a| a as usize != index);
                let mut hidden = 0;
                let order = filter
                    .sort
                    .order(facts)
                    .into_iter()
                    .filter(|&i| {
                        if !p2p_enabled && streams[i].source.is_p2p() {
                            hidden += 1;
                            return false;
                        }
                        !other_addon && matches(&facts[i])
                    })
                    .filter_map(|i| u32::try_from(i).ok())
                    .collect();
                StreamItems::Ready {
                    streams: streams.iter().zip(facts).map(stream).collect(),
                    order,
                    hidden,
                }
            }
            (Loadable::Failed(problem), _) => StreamItems::Failed {
                problem: problem_view(problem),
            },
        },
    }
}

fn stream((stream, facts): (&Stream, &StreamFacts)) -> StreamView {
    StreamView {
        name: stream.name.clone(),
        description: stream.description.clone(),
        kind: stream.source.kind(),
        quality: facts.quality.map(|q| QualityView {
            resolution: q.resolution,
            tier: q.tier(),
            flags: q.flags.iter().map(|f| (*f).to_owned()).collect(),
        }),
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
