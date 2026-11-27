use cineo_core::app::RemoteState;
use cineo_core::remote::{Choice, Command, NowPlaying, Status, Track};
use cineo_net::remote::RemoteError;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RemotePeerView {
    pub name: String,
    pub address: String,
    pub through_sync: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ComputerChoiceView {
    Paired,
    Synced { device: u64 },
}

impl From<ComputerChoiceView> for Choice {
    fn from(choice: ComputerChoiceView) -> Self {
        match choice {
            ComputerChoiceView::Paired => Self::Paired,
            ComputerChoiceView::Synced { device } => Self::Synced(device),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ComputerView {
    pub name: String,
    pub address: String,
    pub through_sync: bool,
    pub active: bool,
    pub choice: ComputerChoiceView,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RemoteLinkView {
    pub code: String,
    pub address: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum RemoteCommandView {
    Pause { paused: bool },
    SeekTo { seconds: f64 },
    SeekBy { seconds: f64 },
    Volume { percent: f64 },
    Mute { muted: bool },
    Audio { id: Option<i64> },
    Subtitle { id: Option<i64> },
    SubtitleDelay { seconds: f64 },
    Speed { speed: f64 },
    Stop,
}

impl From<RemoteCommandView> for Command {
    fn from(command: RemoteCommandView) -> Self {
        match command {
            RemoteCommandView::Pause { paused } => Self::Pause(paused),
            RemoteCommandView::SeekTo { seconds } => Self::SeekTo(seconds),
            RemoteCommandView::SeekBy { seconds } => Self::SeekBy(seconds),
            RemoteCommandView::Volume { percent } => Self::Volume(percent),
            RemoteCommandView::Mute { muted } => Self::Mute(muted),
            RemoteCommandView::Audio { id } => Self::Audio(id),
            RemoteCommandView::Subtitle { id } => Self::Subtitle(id),
            RemoteCommandView::SubtitleDelay { seconds } => Self::SubtitleDelay(seconds),
            RemoteCommandView::Speed { speed } => Self::Speed(speed),
            RemoteCommandView::Stop => Self::Stop,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RemoteTrackView {
    pub id: i64,
    pub number: u32,
    pub label: Option<String>,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct NowPlayingView {
    pub title: String,
    pub loading: bool,
    pub paused: bool,
    pub position_s: f64,
    pub duration_s: f64,
    pub volume: f64,
    pub muted: bool,
    pub speed: f64,
    pub subtitle_delay_s: f64,
    pub audio: Vec<RemoteTrackView>,
    pub subtitles: Vec<RemoteTrackView>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RemoteStatusView {
    pub playing: Option<NowPlayingView>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum RemoteFailure {
    #[error("no computer is paired")]
    NotPaired,
    #[error("the computer could not be reached")]
    Unreachable,
    #[error("the computer has a new code")]
    WrongKey,
    #[error("the clocks differ")]
    Clock,
    #[error("the computer answered HTTP {status}")]
    Rejected { status: u16 },
    #[error("the computer's answer was unreadable")]
    Unreadable,
    #[error("not a valid command")]
    Invalid,
}

impl From<RemoteError> for RemoteFailure {
    fn from(err: RemoteError) -> Self {
        match err {
            RemoteError::Fetch(_) | RemoteError::NotLan | RemoteError::Io(_) => Self::Unreachable,
            RemoteError::WrongKey => Self::WrongKey,
            RemoteError::Clock => Self::Clock,
            RemoteError::Rejected(status) => Self::Rejected { status },
            _ => Self::Unreadable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum RemoteInputError {
    #[error("not an address on the local network")]
    Address,
    #[error("a code has 32 letters and digits")]
    CodeLength,
    #[error("the code has a typo")]
    CodeTypo,
    #[error("this is not a code")]
    NotACode,
}

fn shown_name(computer: &cineo_core::remote::Computer) -> String {
    if computer.name.is_empty() {
        computer.peer.address.ip().to_string()
    } else {
        computer.name.clone()
    }
}

pub(crate) fn peer(remote: &RemoteState) -> Option<RemotePeerView> {
    remote.computer().map(|computer| RemotePeerView {
        name: shown_name(computer),
        address: computer.peer.address.to_string(),
        through_sync: remote.active() != Some(Choice::Paired),
    })
}

pub(crate) fn computers(remote: &RemoteState) -> Vec<ComputerView> {
    let active = remote.active();
    let view = |computer: &cineo_core::remote::Computer, choice: ComputerChoiceView| ComputerView {
        name: shown_name(computer),
        address: computer.peer.address.to_string(),
        through_sync: choice != ComputerChoiceView::Paired,
        active: active == Some(choice.into()),
        choice,
    };
    remote
        .peer
        .iter()
        .map(|computer| view(computer, ComputerChoiceView::Paired))
        .chain(remote.synced.iter().map(|(device, computer)| {
            view(computer, ComputerChoiceView::Synced { device: *device })
        }))
        .collect()
}

fn tracks(tracks: &[Track]) -> Vec<RemoteTrackView> {
    tracks
        .iter()
        .zip(1..)
        .map(|(track, number)| RemoteTrackView {
            id: track.id,
            number,
            label: track.label.clone(),
            selected: track.selected,
        })
        .collect()
}

fn now_playing(playing: &NowPlaying) -> NowPlayingView {
    NowPlayingView {
        title: playing.title.clone(),
        loading: playing.loading,
        paused: playing.paused,
        position_s: playing.position_s,
        duration_s: playing.duration_s,
        volume: playing.volume,
        muted: playing.muted,
        speed: playing.speed,
        subtitle_delay_s: playing.subtitle_delay_s,
        audio: tracks(&playing.audio),
        subtitles: tracks(&playing.subtitles),
    }
}

pub(crate) fn status(status: &Status) -> RemoteStatusView {
    RemoteStatusView {
        playing: status.playing.as_ref().map(now_playing),
    }
}
