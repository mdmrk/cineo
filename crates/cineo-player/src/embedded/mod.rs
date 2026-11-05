mod ffi;
mod render;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use cineo_core::addon::is_safe_header;
use cineo_core::app::{
    AudioOutput, Language, Lookahead, PlayRequest, Settings, SubtitleFont, is_engine_url,
};
use serde_json::Value as Json;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use url::Url;

pub use render::{Display, OnFrame, ProcAddress, Renderer};

use self::ffi::{Core, Event, Value};
use crate::tracker::{EndReason, Input, Tracker};
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
];

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
    ) -> Result<(Self, mpsc::UnboundedReceiver<PlayerEvent>), PlayerError> {
        let (player, _, events) = Self::start_with(request, notify, Output::Surface, &[])?;
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
        for (name, value) in style_options(&request.settings) {
            if let Err(err) = core.set_option(name, &value) {
                warn!(option = name, %err, "subtitle style option not applied");
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

fn start_option(start_ms: u64) -> String {
    format!("{}.{:03}", start_ms / 1000, start_ms % 1000)
}

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
    if settings.audio_output == AudioOutput::Stereo {
        options.push(("audio-channels", "stereo".to_owned()));
    }
    if settings.audio_passthrough {
        options.push(("audio-spdif", "ac3,eac3,dts,dts-hd,truehd".to_owned()));
    }
    options
}

fn style_options(settings: &Settings) -> Vec<(&'static str, String)> {
    let defaults = style_properties(&Settings::default());
    style_properties(settings)
        .into_iter()
        .zip(defaults)
        .filter(|(set, default)| set != default)
        .map(|(set, _)| set)
        .collect()
}

fn style_properties(settings: &Settings) -> Vec<(&'static str, String)> {
    let scale = f64::from(settings.subtitle_size.get()) / 100.0;
    let font = match settings.subtitle_font {
        SubtitleFont::Sans => "sans-serif",
        SubtitleFont::Serif => "serif",
        SubtitleFont::Mono => "monospace",
    };
    let alpha = percent_alpha(settings.subtitle_opacity.get());
    let (style, border, size) = match settings.subtitle_background.rgb() {
        Some(rgb) => ("opaque-box", argb(0xCC, rgb), "1.65"),
        None => match settings.subtitle_outline.rgb() {
            Some(rgb) => ("outline-and-shadow", argb(0xFF, rgb), "1.65"),
            None => ("outline-and-shadow", argb(0xFF, [0, 0, 0]), "0"),
        },
    };
    let bold = if settings.subtitle_bold { "yes" } else { "no" };
    let ass = if settings.keep_subtitle_styles {
        "scale"
    } else {
        "force"
    };
    vec![
        ("sub-scale", format!("{scale:.2}")),
        ("sub-font", font.to_owned()),
        ("sub-bold", bold.to_owned()),
        (
            "sub-pos",
            (100 - settings.subtitle_position.get()).to_string(),
        ),
        ("sub-color", argb(alpha, settings.subtitle_color.rgb())),
        ("sub-border-style", style.to_owned()),
        ("sub-border-color", border),
        ("sub-border-size", size.to_owned()),
        ("sub-ass-override", ass.to_owned()),
    ]
}

fn percent_alpha(percent: u32) -> u8 {
    u8::try_from(percent.min(100) * 255 / 100).unwrap_or(u8::MAX)
}

fn argb(alpha: u8, [r, g, b]: [u8; 3]) -> String {
    format!("#{alpha:02X}{r:02X}{g:02X}{b:02X}")
}

fn language_list(languages: impl Iterator<Item = Language>) -> String {
    languages
        .flat_map(Language::codes)
        .collect::<Vec<_>>()
        .join(",")
}

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
        PlayerCommand::SetPause(paused) => vec![
            "set".into(),
            "pause".into(),
            if paused { "yes" } else { "no" }.into(),
        ],
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
        PlayerCommand::SetSubtitleDelay(seconds) if seconds.is_finite() => {
            vec!["set".into(), "sub-delay".into(), format!("{seconds:.3}")]
        }
        PlayerCommand::SeekTo(_)
        | PlayerCommand::SeekBy(_)
        | PlayerCommand::SetVolume(_)
        | PlayerCommand::SetSubtitleDelay(_) => return None,
        PlayerCommand::ToggleMute => vec!["cycle".into(), "mute".into()],
        PlayerCommand::SetAudio(id) => vec!["set".into(), "aid".into(), track(id)],
        PlayerCommand::SetSubtitle(id) => vec!["set".into(), "sid".into(), track(id)],
        PlayerCommand::Stop => vec!["quit".into()],
    };
    Some(args)
}

fn parse_cached_ranges(json: &str) -> Vec<(f64, f64)> {
    let Ok(state) = serde_json::from_str::<Json>(json) else {
        return Vec::new();
    };
    let Some(ranges) = state.get("seekable-ranges").and_then(Json::as_array) else {
        return Vec::new();
    };
    ranges
        .iter()
        .filter_map(|range| {
            let start = range.get("start")?.as_f64()?;
            let end = range.get("end")?.as_f64()?;
            (start.is_finite() && end.is_finite() && end > start).then_some((start.max(0.0), end))
        })
        .collect()
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
                codec: text(item, "codec"),
            })
        })
        .collect()
}

fn apply_stat(stats: &mut Stats, id: u64, value: &Value) -> bool {
    let number = match *value {
        Value::Double(x) if x.is_finite() && x >= 0.0 => Some(x),
        _ => None,
    };
    let field = match id {
        CACHE_SPEED => &mut stats.cache_speed,
        CACHE_DURATION => &mut stats.cache_s,
        WIDTH => &mut stats.width,
        HEIGHT => &mut stats.height,
        VIDEO_BITRATE => &mut stats.video_bitrate,
        FRAME_DROPS => &mut stats.dropped_frames,
        HWDEC_CURRENT => {
            let hwdec = match value {
                Value::Text(text) if !text.is_empty() => Some(text.clone()),
                _ => None,
            };
            return replace(&mut stats.hwdec, hwdec);
        }
        _ => return false,
    };
    replace(field, number)
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
                Event::FileLoaded => Some(Input::FileLoaded),
                Event::PlaybackRestart => {
                    self.update(|status| replace(&mut status.loaded, true));
                    None
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
                self.update(|s| replace(&mut s.position_s, number));
                return matches!(value, Value::Double(_)).then_some(Input::TimePos(number));
            }
            DURATION => {
                self.update(|s| replace(&mut s.duration_s, number));
                return matches!(value, Value::Double(_)).then_some(Input::Duration(number));
            }
            PAUSE => self.update(|s| replace(&mut s.paused, flag)),
            BUFFERING => self.update(|s| replace(&mut s.buffering, flag)),
            SEEKING => self.update(|s| replace(&mut s.seeking, flag)),
            VOLUME => self.update(|s| replace(&mut s.volume, number)),
            MUTE => self.update(|s| replace(&mut s.muted, flag)),
            TRACK_LIST => {
                let tracks = match value {
                    Value::Text(json) => parse_tracks(json),
                    _ => Vec::new(),
                };
                self.update(|s| replace_shared(&mut s.tracks, tracks));
            }
            CACHE_STATE => {
                let ranges = match value {
                    Value::Text(json) => parse_cached_ranges(json),
                    _ => Vec::new(),
                };
                self.update(|s| replace_shared(&mut s.cached, ranges));
            }
            CACHE_DURATION => {
                self.update(|s| apply_stat(&mut s.stats, id, value));
                return Some(Input::Buffered(number));
            }
            _ => self.update(|s| apply_stat(&mut s.stats, id, value)),
        }
        None
    }

    fn update(&self, change: impl FnOnce(&mut Status) -> bool) {
        if change(&mut self.status.lock().unwrap_or_else(PoisonError::into_inner)) {
            (self.notify)();
        }
    }
}

fn replace<T: PartialEq>(slot: &mut T, value: T) -> bool {
    let changed = *slot != value;
    *slot = value;
    changed
}

fn replace_shared<T: PartialEq>(slot: &mut Arc<[T]>, items: Vec<T>) -> bool {
    let changed = **slot != *items;
    if changed {
        *slot = items.into();
    }
    changed
}

#[cfg(test)]
mod tests;
