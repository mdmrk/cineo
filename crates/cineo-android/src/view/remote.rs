use cineo_core::remote::{Command, NowPlaying, Peer, Status, Track};
use cineo_net::remote::RemoteError;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RemotePeerView {
    pub address: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RemoteLinkView {
    pub code: String,
    pub address: String,
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

pub(crate) fn peer(peer: &Peer) -> RemotePeerView {
    RemotePeerView {
        address: peer.address.to_string(),
    }
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
