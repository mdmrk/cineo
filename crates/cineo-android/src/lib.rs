#[cfg(target_os = "android")]
mod native;
mod player;
mod shell;
mod view;

pub use player::{Playback, Players, Started};
pub use shell::{Cineo, ImageError, PlayerStatusView, StartError, StateListener};
pub use view::{
    AddonView, AppView, DetailView, InstallView, LibraryItemView, MetaStatus, NoticeView,
    PlaybackFailureView, PlaybackView, PosterShapeView, PosterView, ProblemView, RowItems, RowView,
    SourceKindView, StreamGroupView, StreamItems, StreamView, VideoView,
};

uniffi::setup_scaffolding!();
