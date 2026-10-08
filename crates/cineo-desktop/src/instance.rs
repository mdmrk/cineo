use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;

const MAX_LINK: u64 = 8 * 1024;

pub fn forward(data_dir: &Path, link: &str) -> bool {
    let Ok(mut stream) = connect(data_dir) else {
        return false;
    };
    stream
        .write_all(format!("{}\n", link.trim()).as_bytes())
        .and_then(|()| stream.flush())
        .is_ok()
}

fn read_link(stream: impl Read) -> Option<String> {
    let mut line = String::new();
    BufReader::new(stream.take(MAX_LINK))
        .read_line(&mut line)
        .ok()?;
    let link = line.trim();
    (!link.is_empty()).then(|| link.to_owned())
}

#[cfg(unix)]
fn socket_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("cineo.sock")
}

#[cfg(unix)]
fn connect(data_dir: &Path) -> std::io::Result<std::os::unix::net::UnixStream> {
    std::os::unix::net::UnixStream::connect(socket_path(data_dir))
}

#[cfg(unix)]
pub(crate) fn listen(
    data_dir: &Path,
    _runtime: &tokio::runtime::Handle,
    on_link: impl Fn(String) + Send + 'static,
) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;

    let path = socket_path(data_dir);
    if connect(data_dir).is_ok() {
        return Err(std::io::Error::from(std::io::ErrorKind::AddrInUse));
    }
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    std::thread::Builder::new()
        .name("cineo-links".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                if let Some(link) = read_link(stream) {
                    on_link(link);
                }
            }
        })?;
    Ok(())
}

#[cfg(windows)]
fn pipe_name(data_dir: &Path) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    data_dir.hash(&mut hasher);
    format!(r"\\.\pipe\cineo-{:016x}", hasher.finish())
}

#[cfg(windows)]
fn connect(data_dir: &Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .write(true)
        .open(pipe_name(data_dir))
}

#[cfg(windows)]
pub(crate) fn listen(
    data_dir: &Path,
    runtime: &tokio::runtime::Handle,
    on_link: impl Fn(String) + Send + 'static,
) -> std::io::Result<()> {
    use tokio::io::AsyncReadExt;
    use tokio::net::windows::named_pipe::ServerOptions;

    let name = pipe_name(data_dir);
    let _guard = runtime.enter();
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(&name)?;
    runtime.spawn(async move {
        loop {
            if server.connect().await.is_err() {
                break;
            }
            let Ok(next) = ServerOptions::new()
                .reject_remote_clients(true)
                .create(&name)
            else {
                break;
            };
            let mut client = std::mem::replace(&mut server, next);
            let mut bytes = Vec::new();
            let mut limited = (&mut client).take(MAX_LINK);
            if limited.read_to_end(&mut bytes).await.is_ok()
                && let Some(link) = read_link(bytes.as_slice())
            {
                on_link(link);
            }
        }
    });
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn connect(_: &Path) -> std::io::Result<std::fs::File> {
    Err(std::io::ErrorKind::Unsupported.into())
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn listen(
    _: &Path,
    _: &tokio::runtime::Handle,
    _: impl Fn(String) + Send + 'static,
) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}

#[cfg(all(test, unix))]
mod tests {
    use std::sync::mpsc::channel;
    use std::time::Duration;

    #[test]
    fn a_link_reaches_the_running_instance_through_a_private_socket() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("cineo-instance-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert!(
            !super::forward(&dir, "stremio:///board"),
            "nothing listens yet"
        );
        let (tx, rx) = channel();
        super::listen(&dir, runtime.handle(), move |link| {
            let _ = tx.send(link);
        })
        .unwrap();
        let mode = std::fs::metadata(dir.join("cineo.sock"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        assert!(super::forward(&dir, " stremio:///library \n"));
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            "stremio:///library"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
