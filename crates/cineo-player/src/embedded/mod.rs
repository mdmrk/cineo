//! Playback inside the host window through libmpv, loaded at runtime
//! (ADR-0014).

mod ffi;
mod render;

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use cineo_core::app::{Language, PlayRequest, Settings};
use serde_json::Value as Json;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

pub use render::{OnFrame, ProcAddress, Renderer};

use self::ffi::{Core, Event, Value};
use crate::is_header_safe;
use crate::tracker::{EndReason, Input, Tracker};
use crate::{PlayerError, PlayerEvent};

/// The host window's OpenGL access, for drawing the video.
pub struct Video {
    /// Resolves OpenGL functions in the window's context.
    pub get_proc_address: ProcAddress,
    /// Schedules a repaint of the window.
    pub on_frame: OnFrame,
}

impl std::fmt::Debug for Video {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Video").finish_non_exhaustive()
    }
}

/// What the UI can ask of a running playback.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum PlayerCommand {
    TogglePause,
    SeekTo(f64),
    SeekBy(f64),
    SetVolume(f64),
    ToggleMute,
    SetAudio(Option<i64>),
    SetSubtitle(Option<i64>),
    Stop,
}

/// The kind of a media track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    Video,
    Audio,
    Subtitle,
}

/// One track of the playing file. `title` and `lang` come from the media
/// file: untrusted, show them as plain text only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub id: i64,
    pub kind: TrackKind,
    pub title: Option<String>,
    pub lang: Option<String>,
    pub selected: bool,
    /// The file an external track was loaded from (`external-filename`).
    pub external_file: Option<String>,
}

/// A snapshot of the playback for the UI.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Status {
    /// The file has loaded and its first frame is on its way.
    pub loaded: bool,
    pub position_s: f64,
    pub duration_s: f64,
    pub paused: bool,
    /// Waiting for the network.
    pub buffering: bool,
    /// Percent, 0–100.
    pub volume: f64,
    pub muted: bool,
    pub tracks: Arc<[Track]>,
}

const TIME_POS: u64 = 1;
const DURATION: u64 = 2;
const PAUSE: u64 = 3;
const BUFFERING: u64 = 4;
const VOLUME: u64 = 5;
const MUTE: u64 = 6;
const TRACK_LIST: u64 = 7;

const OBSERVED: [(u64, &str, std::ffi::c_int); 7] = [
    (TIME_POS, "time-pos", ffi::FORMAT_DOUBLE),
    (DURATION, "duration", ffi::FORMAT_DOUBLE),
    (PAUSE, "pause", ffi::FORMAT_FLAG),
    (BUFFERING, "paused-for-cache", ffi::FORMAT_FLAG),
    (VOLUME, "volume", ffi::FORMAT_DOUBLE),
    (MUTE, "mute", ffi::FORMAT_FLAG),
    (TRACK_LIST, "track-list", ffi::FORMAT_STRING),
];

// VA-API first: `auto-safe` alone loads the CUDA interop, which prints to stderr.
#[cfg(target_os = "linux")]
const HWDEC: &str = "vaapi,auto-safe";
#[cfg(not(target_os = "linux"))]
const HWDEC: &str = "auto-safe";

const OPTIONS: &[(&str, &str)] = &[
    ("config", "no"),
    ("load-scripts", "no"),
    ("ytdl", "no"),
    ("osc", "no"),
    ("terminal", "no"),
    ("input-default-bindings", "no"),
    ("input-vo-keyboard", "no"),
    ("osd-level", "0"),
    ("osd-bar", "no"),
    ("hwdec", HWDEC),
    ("vo", "libmpv"),
    ("idle", "yes"),
    ("keep-open", "no"),
    ("video-timing-offset", "0"),
    ("audio-client-name", "Cineo"),
];

/// A playback running in this process. Dropping it stops playback; mpv shuts
/// down once the last [`Renderer`] for it is dropped too.
pub struct Player {
    core: Arc<Core>,
    status: Arc<Mutex<Status>>,
    finished: Arc<AtomicBool>,
}

impl std::fmt::Debug for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Player")
            .field("finished", &self.is_finished())
            .finish_non_exhaustive()
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        if !self.is_finished() {
            let _ = self.core.command_async(&["quit"]);
        }
    }
}

impl Player {
    /// Starts playing `request` with its video drawn by the returned
    /// [`Renderer`]. Events arrive on the returned channel. `notify` is
    /// called (from another thread) whenever [`Self::status`] changes.
    pub fn start(
        request: &PlayRequest,
        notify: Arc<dyn Fn() + Send + Sync>,
        video: Video,
    ) -> Result<(Self, Renderer, mpsc::UnboundedReceiver<PlayerEvent>), PlayerError> {
        let (player, renderer, events) = Self::start_with(request, notify, Some(video), &[])?;
        let renderer = renderer.ok_or_else(|| PlayerError::Render("no renderer".into()))?;
        Ok((player, renderer, events))
    }

    fn start_with(
        request: &PlayRequest,
        notify: Arc<dyn Fn() + Send + Sync>,
        video: Option<Video>,
        extra_options: &[(&str, &str)],
    ) -> Result<(Self, Option<Renderer>, mpsc::UnboundedReceiver<PlayerEvent>), PlayerError> {
        if !matches!(request.url.scheme(), "http" | "https") {
            return Err(PlayerError::UnsupportedScheme(
                request.url.scheme().to_owned(),
            ));
        }
        let lib = ffi::lib().map_err(PlayerError::LibmpvUnavailable)?;
        let fail = |err: ffi::MpvError| PlayerError::Libmpv(err.0);
        let core = Arc::new(Core::create(lib).map_err(fail)?);
        let preferences = settings_options(&request.settings);
        let preferences = preferences
            .iter()
            .map(|(name, value)| (*name, value.as_str()));
        for (name, value) in OPTIONS
            .iter()
            .copied()
            .chain(preferences)
            .chain(extra_options.iter().copied())
        {
            core.set_option(name, value)
                .map_err(|err| PlayerError::Libmpv(format!("option {name}: {err}")))?;
        }
        core.initialize().map_err(fail)?;
        core.request_log_messages("warn").map_err(fail)?;
        for (id, name, format) in OBSERVED {
            core.observe(id, name, format).map_err(fail)?;
        }
        // The render context must exist before a file loads, or mpv plays audio only.
        let renderer = video
            .map(|video| Renderer::new(Arc::clone(&core), video.get_proc_address, video.on_frame))
            .transpose()?;
        for (name, value) in &request.headers {
            if is_header_safe(name, value) {
                // `append` adds exactly one item: no list parsing of the value.
                core.command(&[
                    "change-list",
                    "http-header-fields",
                    "append",
                    &format!("{name}: {value}"),
                ])
                .map_err(fail)?;
            }
        }
        // A property, not an option: addon text is never parsed as one.
        core.set_property("force-media-title", &request.title)
            .map_err(fail)?;
        core.command(&["loadfile", request.url.as_str(), "replace"])
            .map_err(fail)?;
        info!(origin = %request.url.origin().ascii_serialization(), "libmpv playback started");

        let status = Arc::new(Mutex::new(Status::default()));
        let finished = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::unbounded_channel();
        let thread = EventLoop {
            core: Arc::clone(&core),
            status: Arc::clone(&status),
            finished: Arc::clone(&finished),
            notify,
            events: tx,
            tracker: Tracker::new(request.start_ms),
        };
        std::thread::Builder::new()
            .name("mpv-events".into())
            .spawn(move || thread.run())
            .map_err(|err| PlayerError::Libmpv(format!("cannot start the event thread: {err}")))?;
        Ok((
            Self {
                core,
                status,
                finished,
            },
            renderer,
            rx,
        ))
    }

    /// The latest playback state.
    pub fn status(&self) -> Status {
        self.status
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The playback is over and mpv has shut down (or is shutting down).
    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::SeqCst)
    }

    /// Loads a subtitle file Cineo wrote and selects it. `title` and `lang`
    /// are shown in the track list.
    pub fn add_subtitle(&self, path: &Path, title: &str, lang: &str) -> Result<(), PlayerError> {
        let args = sub_add_args(path, title, lang).ok_or_else(|| {
            PlayerError::Libmpv("subtitle path is not an absolute UTF-8 path".into())
        })?;
        debug!("mpv sub-add");
        self.core
            .command_async(&args)
            .map_err(|err| PlayerError::Libmpv(err.0))
    }

    /// Sends `command` without waiting for it.
    pub fn send(&self, command: PlayerCommand) {
        let Some(args) = command_args(command) else {
            return;
        };
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        debug!(?command, "mpv command");
        if let Err(err) = self.core.command_async(&args) {
            warn!(%err, ?command, "mpv command failed");
        }
    }
}

/// mpv's preferred subtitle languages: every tag the language goes by.
/// mpv options from the user's settings; they come after [`OPTIONS`].
fn settings_options(settings: &Settings) -> Vec<(&'static str, String)> {
    let mut options = Vec::new();
    if !settings.hardware_decoding {
        options.push(("hwdec", "no".to_owned()));
    }
    let alang = language_list(settings.audio_languages());
    if !alang.is_empty() {
        options.push(("alang", alang));
    }
    let slang = language_list(settings.subtitle_languages());
    if !slang.is_empty() {
        options.push(("slang", slang));
    }
    if settings.remember_volume {
        options.push(("volume", settings.volume.get().to_string()));
    }
    options
}

/// Every tag of every language, in order of preference.
fn language_list(languages: impl Iterator<Item = Language>) -> String {
    languages
        .flat_map(|l| l.codes())
        .collect::<Vec<_>>()
        .join(",")
}

/// `sub-add` with `cached`: adding the same file again selects the loaded
/// track. Only absolute local paths, so a value is never read as a protocol.
fn sub_add_args<'a>(path: &'a Path, title: &'a str, lang: &'a str) -> Option<[&'a str; 5]> {
    if !path.is_absolute() {
        return None;
    }
    Some(["sub-add", path.to_str()?, "cached", title, lang])
}

fn command_args(command: PlayerCommand) -> Option<Vec<String>> {
    let track = |id: Option<i64>| id.map_or_else(|| "no".to_owned(), |id| id.to_string());
    let args = match command {
        PlayerCommand::TogglePause => vec!["cycle".into(), "pause".into()],
        PlayerCommand::SeekTo(seconds) if seconds.is_finite() => vec![
            "seek".into(),
            format!("{:.3}", seconds.max(0.0)),
            "absolute".into(),
        ],
        PlayerCommand::SeekBy(seconds) if seconds.is_finite() => {
            vec!["seek".into(), format!("{seconds:.3}"), "relative".into()]
        }
        PlayerCommand::SetVolume(percent) if percent.is_finite() => vec![
            "set".into(),
            "volume".into(),
            format!("{:.0}", percent.clamp(0.0, 100.0)),
        ],
        PlayerCommand::SeekTo(_) | PlayerCommand::SeekBy(_) | PlayerCommand::SetVolume(_) => {
            return None;
        }
        PlayerCommand::ToggleMute => vec!["cycle".into(), "mute".into()],
        PlayerCommand::SetAudio(id) => vec!["set".into(), "aid".into(), track(id)],
        PlayerCommand::SetSubtitle(id) => vec!["set".into(), "sid".into(), track(id)],
        // `quit`, not `stop`: mpv ignores a stop before the file has loaded.
        PlayerCommand::Stop => vec!["quit".into()],
    };
    Some(args)
}

fn parse_tracks(json: &str) -> Vec<Track> {
    let Ok(Json::Array(items)) = serde_json::from_str::<Json>(json) else {
        return Vec::new();
    };
    let text = |item: &Json, key: &str| {
        item.get(key)
            .and_then(Json::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(str::to_owned)
    };
    items
        .iter()
        .filter_map(|item| {
            let kind = match item.get("type")?.as_str()? {
                "video" => TrackKind::Video,
                "audio" => TrackKind::Audio,
                "sub" => TrackKind::Subtitle,
                _ => return None,
            };
            Some(Track {
                id: item.get("id")?.as_i64()?,
                kind,
                title: text(item, "title"),
                lang: text(item, "lang"),
                selected: item.get("selected").and_then(Json::as_bool) == Some(true),
                external_file: text(item, "external-filename"),
            })
        })
        .collect()
}

struct EventLoop {
    core: Arc<Core>,
    status: Arc<Mutex<Status>>,
    finished: Arc<AtomicBool>,
    notify: Arc<dyn Fn() + Send + Sync>,
    events: mpsc::UnboundedSender<PlayerEvent>,
    tracker: Tracker,
}

fn log_level(mpv_level: &str) -> tracing::Level {
    match mpv_level {
        "fatal" | "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    }
}

impl EventLoop {
    fn run(mut self) {
        let mut done = false;
        loop {
            let input = match self.core.wait_event(-1.0) {
                Event::Shutdown => break,
                Event::Log {
                    level,
                    prefix,
                    text,
                } => {
                    let text = text.trim_end();
                    if log_level(&level) == tracing::Level::ERROR {
                        error!(target: "libmpv", "[{prefix}] {text}");
                    } else {
                        info!(target: "libmpv", "[{prefix}] {text}");
                    }
                    None
                }
                Event::FileLoaded => {
                    self.update(|status| status.loaded = true);
                    Some(Input::FileLoaded)
                }
                Event::EndFile { eof } => Some(Input::EndFile(match eof {
                    Ok(true) => EndReason::Eof,
                    Ok(false) => EndReason::Other,
                    Err(message) => EndReason::Error(message),
                })),
                Event::Property { id, value } => self.property(id, &value),
                Event::None | Event::Other => None,
            };
            let Some(input) = input else { continue };
            if done {
                continue;
            }
            let step = self.tracker.on(input, Instant::now());
            if let Some(seconds) = step.seek_to {
                let target = format!("{seconds:.3}");
                if let Err(err) = self.core.command_async(&["seek", &target, "absolute"]) {
                    warn!(%err, "resume seek failed");
                }
            }
            if let Some(event) = step.event {
                let _ = self.events.send(event);
            }
            if step.done {
                done = true;
                self.finished.store(true, Ordering::SeqCst);
                (self.notify)();
                let _ = self.core.command_async(&["quit"]);
            }
        }
        if !done {
            let _ = self.events.send(self.tracker.closed());
        }
        self.finished.store(true, Ordering::SeqCst);
        (self.notify)();
        debug!("mpv event loop ended");
    }

    fn property(&self, id: u64, value: &Value) -> Option<Input> {
        let number = match *value {
            Value::Double(x) if x.is_finite() => x,
            _ => 0.0,
        };
        let flag = *value == Value::Flag(true);
        match id {
            TIME_POS => {
                self.update(|s| s.position_s = number);
                return matches!(value, Value::Double(_)).then_some(Input::TimePos(number));
            }
            DURATION => {
                self.update(|s| s.duration_s = number);
                return matches!(value, Value::Double(_)).then_some(Input::Duration(number));
            }
            PAUSE => self.update(|s| s.paused = flag),
            BUFFERING => self.update(|s| s.buffering = flag),
            VOLUME => self.update(|s| s.volume = number),
            MUTE => self.update(|s| s.muted = flag),
            TRACK_LIST => {
                let tracks = match value {
                    Value::Text(json) => parse_tracks(json),
                    _ => Vec::new(),
                };
                self.update(|s| s.tracks = tracks.into());
            }
            _ => {}
        }
        None
    }

    fn update(&self, change: impl FnOnce(&mut Status)) {
        change(&mut self.status.lock().unwrap_or_else(PoisonError::into_inner));
        (self.notify)();
    }
}

#[cfg(test)]
mod tests;
