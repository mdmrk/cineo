//! Spawning mpv and connecting to its IPC endpoint.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cineo_core::app::PlayRequest;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tracing::{Instrument, info, info_span, warn};

use crate::ipc::{PlayerEvent, drive_session};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PlayerError {
    #[error("mpv was not found; install mpv and make sure it is on PATH")]
    MpvNotFound,
    #[error("could not start mpv: {0}")]
    Spawn(std::io::Error),
    #[error("only http(s) streams can be played, got `{0}`")]
    UnsupportedScheme(String),
    #[error("mpv exited before accepting commands")]
    ExitedEarly,
    #[error("could not connect to mpv: {0}")]
    Connect(std::io::Error),
}

/// A running playback. Dropping it closes mpv.
#[derive(Debug)]
pub struct PlayerHandle {
    pub events: mpsc::UnboundedReceiver<PlayerEvent>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for PlayerHandle {
    fn drop(&mut self) {
        // The task owns the child with `kill_on_drop`.
        self.task.abort();
    }
}

/// Starts mpv for `request`. `mpv` is the executable (default: `mpv` on PATH).
pub async fn launch(
    mpv: Option<PathBuf>,
    request: PlayRequest,
) -> Result<PlayerHandle, PlayerError> {
    if !matches!(request.url.scheme(), "http" | "https") {
        return Err(PlayerError::UnsupportedScheme(
            request.url.scheme().to_owned(),
        ));
    }
    let endpoint = ipc_endpoint().map_err(PlayerError::Spawn)?;
    let mut child = Command::new(mpv.unwrap_or_else(|| PathBuf::from("mpv")))
        .args([
            "--no-config",
            "--ytdl=no",
            "--idle=once",
            "--force-window=yes",
            "--keep-open=no",
            "--terminal=no",
            "--hwdec=auto-safe",
        ])
        .arg(format!("--input-ipc-server={}", endpoint.display()))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => PlayerError::MpvNotFound,
            _ => PlayerError::Spawn(err),
        })?;
    info!(origin = %request.url.origin().ascii_serialization(), "mpv started");

    let stream = connect(&endpoint, &mut child).await?;
    let (tx, rx) = mpsc::unbounded_channel();
    let span = info_span!("playback", meta_id = %request.meta_id, video_id = %request.video_id);
    let task = tokio::spawn(
        async move {
            drive_session(stream, &request, tx).await;
            // Let mpv finish on its own; it quits after the file (`--idle=once`).
            if let Err(err) = child.wait().await {
                warn!(%err, "waiting for mpv failed");
            }
            cleanup(&endpoint);
        }
        .instrument(span),
    );
    Ok(PlayerHandle { events: rx, task })
}

fn unique_name() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!("mpv-{}-{nanos}", std::process::id())
}

#[cfg(unix)]
type IpcStream = tokio::net::UnixStream;
#[cfg(windows)]
type IpcStream = tokio::net::windows::named_pipe::NamedPipeClient;

/// A socket in a private (0700) per-user directory.
#[cfg(unix)]
fn ipc_endpoint() -> std::io::Result<PathBuf> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let base = std::env::var_os("XDG_RUNTIME_DIR").map_or_else(std::env::temp_dir, PathBuf::from);
    let dir = base.join(format!("cineo-{}", std::process::id()));
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    Ok(dir.join(format!("{}.sock", unique_name())))
}

#[cfg(windows)]
fn ipc_endpoint() -> std::io::Result<PathBuf> {
    Ok(PathBuf::from(format!(r"\\.\pipe\cineo-{}", unique_name())))
}

async fn connect(endpoint: &std::path::Path, child: &mut Child) -> Result<IpcStream, PlayerError> {
    let mut last_err = None;
    for _ in 0..100 {
        if let Ok(Some(_)) = child.try_wait() {
            return Err(PlayerError::ExitedEarly);
        }
        #[cfg(unix)]
        let attempt = tokio::net::UnixStream::connect(endpoint).await;
        #[cfg(windows)]
        let attempt = tokio::net::windows::named_pipe::ClientOptions::new().open(endpoint);
        match attempt {
            Ok(stream) => return Ok(stream),
            Err(err) => last_err = Some(err),
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Err(PlayerError::Connect(
        last_err.unwrap_or_else(|| std::io::Error::other("timed out")),
    ))
}

fn cleanup(endpoint: &std::path::Path) {
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(endpoint);
        if let Some(dir) = endpoint.parent() {
            // Succeeds only once the directory is empty.
            let _ = std::fs::remove_dir(dir);
        }
    }
    #[cfg(windows)]
    let _ = endpoint;
}
