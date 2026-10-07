//! The JSON IPC conversation with mpv, independent of how the byte stream
//! was obtained (socket, named pipe, or an in-memory pipe in tests).

use std::time::{Duration, Instant};

use cineo_core::app::PlayRequest;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;
use tracing::{debug, warn};

/// What the player reports back to the shell.
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEvent {
    /// Playback position, throttled to at most one per `PROGRESS_INTERVAL`.
    Progress { time_ms: u64, duration_ms: u64 },
    /// The file played to the end.
    Ended { time_ms: u64, duration_ms: u64 },
    /// mpv could not play the file.
    Failed(String),
    /// The player window was closed or the connection ended.
    Closed { time_ms: u64, duration_ms: u64 },
}

const PROGRESS_INTERVAL: Duration = Duration::from_secs(5);
const TIME_POS: u64 = 1;
const DURATION: u64 = 2;

/// The commands that start playback of `request`, in order. Resuming is a
/// `seek` sent after `file-loaded`: setting the `start` property before
/// `loadfile` is ignored by mpv 0.41 (VERIFIED 2026-10-06), and `loadfile`'s
/// options argument moved between mpv versions.
pub(crate) fn start_commands(request: &PlayRequest) -> Vec<Value> {
    let mut commands = Vec::new();
    let headers: Vec<String> = request
        .headers
        .iter()
        .filter(|(name, value)| is_header_safe(name, value))
        .map(|(name, value)| format!("{name}: {value}"))
        .collect();
    if !headers.is_empty() {
        commands.push(json!(["set_property", "http-header-fields", headers]));
    }
    // A property, not the `--title` option: avoids option parsing and
    // property expansion of addon-provided text.
    commands.push(json!(["set_property", "force-media-title", request.title]));
    commands.push(json!(["observe_property", TIME_POS, "time-pos"]));
    commands.push(json!(["observe_property", DURATION, "duration"]));
    commands.push(json!(["loadfile", request.url.as_str(), "replace"]));
    commands
}

fn is_header_safe(name: &str, value: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
        && !value.bytes().any(|b| matches!(b, b'\r' | b'\n' | 0))
}

/// Sends the start commands over `stream` and forwards events until mpv
/// closes the connection or playback ends.
pub async fn drive_session<S>(
    stream: S,
    request: &PlayRequest,
    events: mpsc::UnboundedSender<PlayerEvent>,
) where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (reader, mut writer) = tokio::io::split(stream);
    for (i, command) in start_commands(request).into_iter().enumerate() {
        let line = json!({ "command": command, "request_id": i }).to_string() + "\n";
        debug!(command = %command, "mpv ipc send");
        if let Err(err) = writer.write_all(line.as_bytes()).await {
            let _ = events.send(PlayerEvent::Failed(format!("mpv IPC write failed: {err}")));
            return;
        }
    }

    let mut lines = BufReader::new(reader).lines();
    let mut time_ms = 0_u64;
    let mut duration_ms = 0_u64;
    let mut last_report: Option<Instant> = None;
    let mut started = false;
    loop {
        let Ok(Some(line)) = lines.next_line().await else {
            break;
        };
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            warn!("mpv ipc: unparsable line");
            continue;
        };
        match message.get("event").and_then(Value::as_str) {
            Some("property-change") => {
                let seconds = message.get("data").and_then(Value::as_f64);
                match (message.get("id").and_then(Value::as_u64), seconds) {
                    (Some(TIME_POS), Some(s)) => time_ms = seconds_to_ms(s),
                    (Some(DURATION), Some(s)) => duration_ms = seconds_to_ms(s),
                    _ => continue,
                }
                if last_report.is_none_or(|t| t.elapsed() >= PROGRESS_INTERVAL) && time_ms > 0 {
                    last_report = Some(Instant::now());
                    let _ = events.send(PlayerEvent::Progress {
                        time_ms,
                        duration_ms,
                    });
                }
            }
            Some("file-loaded") => {
                started = true;
                if request.start_ms > 0 {
                    #[expect(
                        clippy::cast_precision_loss,
                        reason = "milliseconds of a video fit in f64"
                    )]
                    let seconds = request.start_ms as f64 / 1000.0;
                    let line =
                        json!({ "command": ["seek", seconds, "absolute"] }).to_string() + "\n";
                    if let Err(err) = writer.write_all(line.as_bytes()).await {
                        warn!(%err, "mpv ipc: resume seek failed");
                    }
                }
            }
            Some("end-file") => {
                let reason = message.get("reason").and_then(Value::as_str).unwrap_or("");
                debug!(reason, "mpv end-file");
                match reason {
                    "eof" => {
                        let _ = events.send(PlayerEvent::Ended {
                            time_ms,
                            duration_ms,
                        });
                        return;
                    }
                    "error" => {
                        let detail = message
                            .get("file_error")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown error");
                        let _ = events.send(PlayerEvent::Failed(format!(
                            "mpv could not play the stream: {detail}"
                        )));
                        return;
                    }
                    // `stop`/`quit`/`redirect`: the user closed it or mpv replaced the file.
                    _ if started => break,
                    _ => {}
                }
            }
            _ => {
                if let Some(error) = message.get("error").and_then(Value::as_str)
                    && error != "success"
                {
                    warn!(error, "mpv ipc command failed");
                }
            }
        }
    }
    let _ = events.send(PlayerEvent::Closed {
        time_ms,
        duration_ms,
    });
}

fn seconds_to_ms(seconds: f64) -> u64 {
    if seconds.is_finite() && seconds > 0.0 {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "positive, finite, sub-2^53"
        )]
        let ms = (seconds * 1000.0) as u64;
        ms
    } else {
        0
    }
}
