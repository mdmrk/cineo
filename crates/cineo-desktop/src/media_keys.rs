#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Remote {
    Play,
    Pause,
    PlayPause,
    Stop,
    Raise,
}

#[cfg(not(windows))]
pub(crate) use crate::mpris::Mpris as MediaKeys;
#[cfg(windows)]
pub(crate) use crate::smtc::Smtc as MediaKeys;
