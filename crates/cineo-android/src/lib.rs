#[cfg(target_os = "android")]
mod native;
mod player;
mod shell;
mod view;

pub use player::{Playback, Players, Started};
pub use shell::{Cineo, ImageError, PlayerStatusView, SettingChange, StartError, StateListener};
pub use view::{
    AddonListItems, AddonListView, AddonView, AppView, ArtView, DetailView, InstallView,
    LibraryItemView, ListedAddonView, MetaStatus, NoticeView, PlaybackFailureView, PlaybackView,
    PosterView, ProblemView, QualityView, RowItems, RowView, SettingsView, StreamFilterView,
    StreamGroupView, StreamItems, StreamView, TorrentView, VideoView,
};

#[uniffi::export]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

uniffi::setup_scaffolding!();
