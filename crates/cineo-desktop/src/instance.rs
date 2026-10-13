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

fn socket_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("cineo.sock")
}

fn connect(data_dir: &Path) -> std::io::Result<std::os::unix::net::UnixStream> {
    std::os::unix::net::UnixStream::connect(socket_path(data_dir))
}

pub(crate) fn listen(
    data_dir: &Path,
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

#[cfg(test)]
mod tests {
    use std::sync::mpsc::channel;
    use std::time::Duration;

    #[test]
    fn a_link_reaches_the_running_instance_through_a_private_socket() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("cineo-instance-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(
            !super::forward(&dir, "stremio:///board"),
            "nothing listens yet"
        );
        let (tx, rx) = channel();
        super::listen(&dir, move |link| {
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
