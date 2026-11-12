mod events;
mod ffi;
mod options;
mod render;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use cineo_core::addon::is_safe_header;
use cineo_core::app::{Lookahead, PlayRequest, Settings, is_engine_url};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use url::Url;

pub use render::{Display, OnFrame, ProcAddress, Renderer};

use self::events::EventLoop;
use self::ffi::Core;
use self::options::{
    command_args, settings_options, start_option, style_options, style_properties, sub_add_args,
};
use crate::tracker::Tracker;
use crate::{PlayerError, PlayerEvent};

pub struct Video {
    pub get_proc_address: ProcAddress,
    pub on_frame: OnFrame,
    pub display: Option<Display>,
}

impl std::fmt::Debug for Video {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Video").finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum PlayerCommand {
    TogglePause,
    SetPause(bool),
    SeekTo(f64),
    SeekBy(f64),
    SetVolume(f64),
    ToggleMute,
    SetMute(bool),
    SetAudio(Option<i64>),
    SetSubtitle(Option<i64>),
    SetSubtitleDelay(f64),
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    Video,
    Audio,
    Subtitle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub id: i64,
    pub kind: TrackKind,
    pub title: Option<String>,
    pub lang: Option<String>,
    pub selected: bool,
    pub external_file: Option<String>,
    pub codec: Option<String>,
}

impl Track {
    pub fn label(&self) -> Option<String> {
        let parts: Vec<&str> = [self.lang.as_deref(), self.title.as_deref()]
            .into_iter()
            .flatten()
            .collect();
        (!parts.is_empty()).then(|| crate::plain_text(&parts.join(" · ")))
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Status {
    pub loaded: bool,
    pub position_s: f64,
    pub duration_s: f64,
    pub paused: bool,
    pub buffering: bool,
    pub seeking: bool,
    pub volume: f64,
    pub muted: bool,
    pub tracks: Arc<[Track]>,
    pub cached: Arc<[(f64, f64)]>,
    pub stats: Stats,
}

impl Status {
    pub fn is_waiting(&self) -> bool {
        self.buffering || self.seeking
    }

    pub fn video_codec(&self) -> Option<String> {
        self.tracks
            .iter()
            .find(|t| t.kind == TrackKind::Video && t.selected)
            .and_then(|t| t.codec.as_deref())
            .map(crate::plain_text)
    }

    #[expect(clippy::cast_possible_truncation, reason = "fractions in 0..=1")]
    pub fn cached_spans(&self) -> impl Iterator<Item = (f32, f32)> + '_ {
        let duration = self.duration_s;
        let fraction = move |s: f64| (s / duration).clamp(0.0, 1.0) as f32;
        self.cached
            .iter()
            .filter(move |_| duration > 0.0)
            .map(move |&(start, end)| (fraction(start), fraction(end)))
            .filter(|(start, end)| end > start)
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Stats {
    pub cache_speed: Option<f64>,
    pub cache_s: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub video_bitrate: Option<f64>,
    pub hwdec: Option<String>,
    pub dropped_frames: Option<f64>,
}

const TIME_POS: u64 = 1;
const DURATION: u64 = 2;
const PAUSE: u64 = 3;
const BUFFERING: u64 = 4;
const VOLUME: u64 = 5;
const MUTE: u64 = 6;
const TRACK_LIST: u64 = 7;
const CACHE_SPEED: u64 = 8;
const CACHE_DURATION: u64 = 9;
const WIDTH: u64 = 10;
const HEIGHT: u64 = 11;
const VIDEO_BITRATE: u64 = 12;
const HWDEC_CURRENT: u64 = 13;
const FRAME_DROPS: u64 = 14;
const SEEKING: u64 = 15;
const CACHE_STATE: u64 = 16;

const OBSERVED: [(u64, &str, std::ffi::c_int); 16] = [
    (TIME_POS, "time-pos", ffi::FORMAT_DOUBLE),
    (DURATION, "duration", ffi::FORMAT_DOUBLE),
    (PAUSE, "pause", ffi::FORMAT_FLAG),
    (BUFFERING, "paused-for-cache", ffi::FORMAT_FLAG),
    (VOLUME, "volume", ffi::FORMAT_DOUBLE),
    (MUTE, "mute", ffi::FORMAT_FLAG),
    (TRACK_LIST, "track-list", ffi::FORMAT_STRING),
    (CACHE_SPEED, "cache-speed", ffi::FORMAT_DOUBLE),
    (CACHE_DURATION, "demuxer-cache-duration", ffi::FORMAT_DOUBLE),
    (WIDTH, "width", ffi::FORMAT_DOUBLE),
    (HEIGHT, "height", ffi::FORMAT_DOUBLE),
    (VIDEO_BITRATE, "video-bitrate", ffi::FORMAT_DOUBLE),
    (HWDEC_CURRENT, "hwdec-current", ffi::FORMAT_STRING),
    (FRAME_DROPS, "frame-drop-count", ffi::FORMAT_DOUBLE),
    (SEEKING, "seeking", ffi::FORMAT_FLAG),
    (CACHE_STATE, "demuxer-cache-state", ffi::FORMAT_STRING),
];

#[cfg(all(not(windows), not(target_os = "android")))]
const HWDEC: &str = "vaapi,auto-safe";
#[cfg(windows)]
const HWDEC: &str = "auto-safe";
#[cfg(target_os = "android")]
const HWDEC: &str = "mediacodec";

const SCRIPT_OPTIONS: &[&str] = &["load-scripts", "ytdl", "osc"];

const SURFACE_OPTIONS: &[(&str, &str)] = &[
    ("vo", "null"),
    ("force-window", "no"),
    ("gpu-context", "android"),
    ("opengl-es", "yes"),
    ("profile", "fast"),
    ("vd-lavc-o", "ndk_codec=1"),
    ("vf", "format=fmt=yuv420p"),
];

const SURFACE_SUBTITLE_FONT: &str = "Inter";

fn surface_font_options(fonts_dir: &str) -> [(&str, &str); 2] {
    [
        ("sub-fonts-dir", fonts_dir),
        ("sub-font", SURFACE_SUBTITLE_FONT),
    ]
}

#[derive(Debug)]
enum Output {
    Render(Video),
    Surface,
    #[cfg(test)]
    None,
}

impl Output {
    fn options(&self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Surface => SURFACE_OPTIONS,
            _ => &[],
        }
    }

    fn into_video(self) -> Option<Video> {
        match self {
            Self::Render(video) => Some(video),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Set {
    Option,
    Property,
}

fn attach_steps(wid: i64) -> [(Set, &'static str, String); 3] {
    [
        (Set::Option, "wid", wid.to_string()),
        (Set::Option, "force-window", "yes".into()),
        (Set::Property, "vo", "gpu".into()),
    ]
}

fn detach_steps() -> [(Set, &'static str, String); 3] {
    [
        (Set::Property, "vo", "null".into()),
        (Set::Option, "force-window", "no".into()),
        (Set::Option, "wid", "0".into()),
    ]
}

#[cfg(target_os = "android")]
pub fn set_java_vm(vm: std::ptr::NonNull<std::ffi::c_void>) -> Result<(), PlayerError> {
    ffi::set_java_vm(vm).map_err(PlayerError::LibmpvUnavailable)
}

fn load(core: &Core, url: &Url) -> Result<(), ffi::MpvError> {
    core.command(&["loadfile", url.as_str(), "replace"])?;
    info!(origin = %url.origin().ascii_serialization(), "libmpv playback started");
    Ok(())
}

pub fn use_bundled_libmpv(path: PathBuf) {
    ffi::set_bundled(path);
}

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
    ("cache-secs", "120"),
    ("demuxer-max-bytes", "512MiB"),
];

pub struct Player {
    core: Arc<Core>,
    pending: Mutex<Option<Url>>,
    status: Arc<Mutex<Status>>,
    finished: Arc<AtomicBool>,
    events_thread: Option<std::thread::JoinHandle<()>>,
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
        if let Some(thread) = self.events_thread.take()
            && thread.join().is_err()
        {
            error!("the mpv event thread panicked");
        }
    }
}

impl Player {
    pub fn start(
        request: &PlayRequest,
        notify: Arc<dyn Fn() + Send + Sync>,
        video: Video,
    ) -> Result<(Self, Renderer, mpsc::UnboundedReceiver<PlayerEvent>), PlayerError> {
        let (player, renderer, events) =
            Self::start_with(request, notify, Output::Render(video), &[])?;
        let renderer = renderer.ok_or_else(|| PlayerError::Render("no renderer".into()))?;
        Ok((player, renderer, events))
    }

    pub fn start_on_surface(
        request: &PlayRequest,
        notify: Arc<dyn Fn() + Send + Sync>,
        fonts_dir: &Path,
    ) -> Result<(Self, mpsc::UnboundedReceiver<PlayerEvent>), PlayerError> {
        let fonts_dir = fonts_dir
            .to_str()
            .ok_or_else(|| PlayerError::Libmpv("the fonts folder is not a UTF-8 path".into()))?;
        let (player, _, events) = Self::start_with(
            request,
            notify,
            Output::Surface,
            &surface_font_options(fonts_dir),
        )?;
        Ok((player, events))
    }

    pub fn attach_surface(&self, wid: i64) -> Result<(), PlayerError> {
        self.apply(&attach_steps(wid))?;
        let pending = self
            .pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(url) = pending {
            load(&self.core, &url).map_err(|err| PlayerError::Libmpv(err.message))?;
        }
        Ok(())
    }

    pub fn detach_surface(&self) -> Result<(), PlayerError> {
        self.apply(&detach_steps())
    }

    pub fn resize_surface(&self, width: u32, height: u32) -> Result<(), PlayerError> {
        self.apply(&[(
            Set::Property,
            "android-surface-size",
            format!("{width}x{height}"),
        )])
    }

    fn apply(&self, steps: &[(Set, &str, String)]) -> Result<(), PlayerError> {
        for (set, name, value) in steps {
            let result = match set {
                Set::Option => self.core.set_option(name, value),
                Set::Property => self.core.set_property(name, value),
            };
            result.map_err(|err| PlayerError::Libmpv(format!("{name}: {err}")))?;
        }
        Ok(())
    }

    fn start_with(
        request: &PlayRequest,
        notify: Arc<dyn Fn() + Send + Sync>,
        output: Output,
        extra_options: &[(&str, &str)],
    ) -> Result<(Self, Option<Renderer>, mpsc::UnboundedReceiver<PlayerEvent>), PlayerError> {
        if !matches!(request.url.scheme(), "http" | "https") {
            return Err(PlayerError::UnsupportedScheme(
                request.url.scheme().to_owned(),
            ));
        }
        let lib = ffi::lib().map_err(PlayerError::LibmpvUnavailable)?;
        let fail = |err: ffi::MpvError| PlayerError::Libmpv(err.message);
        let core = Arc::new(Core::create(lib).map_err(fail)?);
        let mut preferences = settings_options(&request.settings);
        if request.start_ms > 0 {
            preferences.push(("start", start_option(request.start_ms)));
        }
        if is_engine_url(&request.url) {
            preferences.push(("demuxer-hysteresis-secs", Lookahead::RESUME_S.to_string()));
            preferences.push(("network-timeout", "0".to_owned()));
        }
        let preferences = preferences
            .iter()
            .map(|(name, value)| (*name, value.as_str()));
        for (name, value) in style_options(&request.settings) {
            if let Err(err) = core.set_option(name, &value) {
                warn!(option = name, %err, "subtitle style option not applied");
            }
        }
        for (name, value) in OPTIONS
            .iter()
            .copied()
            .chain(preferences)
            .chain(output.options().iter().copied())
            .chain(extra_options.iter().copied())
        {
            match core.set_option(name, value) {
                Err(err) if SCRIPT_OPTIONS.contains(&name) && err.is_option_not_found() => {
                    debug!(option = name, "libmpv was built without scripting");
                }
                result => {
                    result.map_err(|err| PlayerError::Libmpv(format!("option {name}: {err}")))?;
                }
            }
        }
        core.initialize().map_err(fail)?;
        core.request_log_messages("warn").map_err(fail)?;
        for (id, name, format) in OBSERVED {
            core.observe(id, name, format).map_err(fail)?;
        }
        let waits_for_surface = matches!(output, Output::Surface);
        let renderer = output
            .into_video()
            .map(|video| {
                Renderer::new(
                    Arc::clone(&core),
                    video.get_proc_address,
                    video.on_frame,
                    video.display,
                )
            })
            .transpose()?;
        for (name, value) in &request.headers {
            if is_safe_header(name, value) {
                core.command(&[
                    "change-list",
                    "http-header-fields",
                    "append",
                    &format!("{name}: {value}"),
                ])
                .map_err(fail)?;
            }
        }
        core.set_property("force-media-title", &request.title)
            .map_err(fail)?;
        if waits_for_surface {
            info!("libmpv waits for a surface");
        } else {
            load(&core, &request.url).map_err(fail)?;
        }

        let status = Arc::new(Mutex::new(Status::default()));
        let finished = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::unbounded_channel();
        let thread = EventLoop {
            core: Arc::clone(&core),
            status: Arc::clone(&status),
            finished: Arc::clone(&finished),
            notify,
            events: tx,
            tracker: Tracker::new(),
        };
        let joins_on_drop = renderer.is_none();
        let events_thread = std::thread::Builder::new()
            .name("mpv-events".into())
            .spawn(move || thread.run())
            .map_err(|err| PlayerError::Libmpv(format!("cannot start the event thread: {err}")))?;
        Ok((
            Self {
                core,
                pending: Mutex::new(waits_for_surface.then(|| request.url.clone())),
                status,
                finished,
                events_thread: joins_on_drop.then_some(events_thread),
            },
            renderer,
            rx,
        ))
    }

    pub fn status(&self) -> Status {
        self.status
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::SeqCst)
    }

    pub fn add_subtitle(&self, path: &Path, title: &str, lang: &str) -> Result<(), PlayerError> {
        let args = sub_add_args(path, title, lang).ok_or_else(|| {
            PlayerError::Libmpv("subtitle path is not an absolute UTF-8 path".into())
        })?;
        debug!("mpv sub-add");
        self.core
            .command_async(&args)
            .map_err(|err| PlayerError::Libmpv(err.message))
    }

    pub fn set_subtitle_style(&self, settings: &Settings) {
        for (name, value) in style_properties(settings) {
            if let Err(err) = self.core.set_property_async(name, &value) {
                warn!(property = name, %err, "subtitle style not applied");
            }
        }
    }

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

#[cfg(test)]
mod tests;
