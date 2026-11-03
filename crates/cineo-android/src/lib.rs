mod shell;
#[cfg(target_os = "android")]
mod tls;
mod view;

pub use shell::{Cineo, ImageError, StartError, StateListener};
pub use view::{
    AddonView, AppView, InstallView, NoticeView, PlaybackFailureView, PosterShapeView, PosterView,
    ProblemView, RowItems, RowView, SourceKindView,
};

uniffi::setup_scaffolding!();
