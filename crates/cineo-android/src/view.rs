use cineo_core::addon::{MetaPreview, PosterShape, SourceKind};
use cineo_core::app::{InstalledAddon, Loadable, Notice, PlaybackFailure, Problem, Row, State};

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AppView {
    pub addons: Vec<AddonView>,
    pub addons_loading: u32,
    pub install: Option<InstallView>,
    pub board: Vec<RowView>,
    pub notice: Option<NoticeView>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AddonView {
    pub transport: String,
    pub name: String,
    pub version: String,
    pub logo: Option<String>,
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
    ConfigurationRequired { transport: String },
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
    pub shape: PosterShapeView,
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

pub(crate) fn view(state: &State) -> AppView {
    AppView {
        addons: state.addons.iter().map(addon).collect(),
        addons_loading: u32::try_from(state.addons_loading.len()).unwrap_or(u32::MAX),
        install: state.install.as_ref().map(|install| match install {
            Loadable::Loading => InstallView::Installing,
            Loadable::Ready(name) => InstallView::Installed { name: name.clone() },
            Loadable::Failed(problem) => InstallView::Failed {
                problem: problem_view(problem),
            },
        }),
        board: state.board.iter().map(row).collect(),
        notice: state.notice.as_ref().map(notice_view),
    }
}

fn addon(addon: &InstalledAddon) -> AddonView {
    AddonView {
        transport: addon.transport.to_string(),
        name: addon.manifest.name.clone(),
        version: addon.manifest.version.to_string(),
        logo: addon.manifest.logo.as_ref().map(ToString::to_string),
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
    }
}

fn problem_view(problem: &Problem) -> ProblemView {
    match problem {
        Problem::AlreadyInstalled => ProblemView::AlreadyInstalled,
        Problem::ConfigurationRequired(transport) => ProblemView::ConfigurationRequired {
            transport: transport.to_string(),
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
