#[cfg(target_os = "android")]
mod native;
mod player;
mod shell;
mod view;

pub use player::{Playback, Players, Started};
pub use shell::{Cineo, ImageError, PlayerStatusView, SettingChange, StartError, StateListener};
pub use view::{
    AddonListItems, AddonListView, AddonView, AppView, DetailView, InstallView, LibraryItemView,
    ListedAddonView, MetaStatus, NoticeView, PlaybackFailureView, PlaybackView, PosterShapeView,
    PosterView, ProblemView, RowItems, RowView, SettingsView, SourceKindView, StreamGroupView,
    StreamItems, StreamView, TorrentView, VideoView,
};

uniffi::setup_scaffolding!();
