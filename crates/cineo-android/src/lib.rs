#[cfg(target_os = "android")]
mod native;
mod player;
mod shell;
mod view;

pub use player::{Playback, Players, Started};
pub use shell::{
    AddonSubtitleView, CachedSpanView, Cineo, DecoderView, ImageError, PlayerStatsView,
    PlayerStatusView, SettingChange, StartError, StateListener, TrackView, VideoSizeView,
};
pub use view::{
    AddonListItems, AddonListView, AddonView, AppView, ArtView, ChoiceView, DetailView,
    InstallView, LibraryItemView, ListedAddonView, MetaStatus, NoticeView, PlaybackFailureView,
    PlaybackView, PosterView, ProblemView, QualityView, RangeView, RowItems, RowView, SettingsView,
    StreamFilterView, StreamGroupView, StreamItems, StreamView, SubtitlePreviewView, ToggleView,
    TorrentView, UpdateView, VideoView,
};

#[uniffi::export]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

#[uniffi::export]
pub fn resolution_label(resolution: cineo_core::app::Resolution) -> String {
    resolution.label().to_owned()
}

uniffi::setup_scaffolding!();
