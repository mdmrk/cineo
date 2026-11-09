use std::path::Path;
use std::sync::OnceLock;

use fastframe_instance::{Claim, Guard, Slot};

const NAME: &str = "cineo";
const PING: &str = "ping";
const RAISE: &str = "raise";
const OK: &str = "ok";

static RUNNING_COPY: OnceLock<Guard> = OnceLock::new();

fn slot(data_dir: &Path) -> Slot {
    Slot::at(data_dir.join("instance"), NAME)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Request {
    Raise,
    Link(String),
}

fn parse(request: &str) -> Option<Request> {
    match request.trim() {
        "" | PING => None,
        RAISE => Some(Request::Raise),
        link => Some(Request::Link(link.to_owned())),
    }
}

pub fn forward(data_dir: &Path, link: Option<&str>) -> bool {
    slot(data_dir).send(link.map_or(RAISE, str::trim)).is_ok()
}

pub(crate) fn listen(data_dir: &Path, on_request: impl Fn(Request) + Send + 'static) -> bool {
    let claim = slot(data_dir).claim(PING, move |request| {
        if let Some(request) = parse(request) {
            on_request(request);
        }
        Some(OK.to_owned())
    });
    let Claim::First(guard) = claim else {
        return false;
    };
    let _ = RUNNING_COPY.set(guard);
    true
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::channel;
    use std::time::Duration;

    use super::Request;

    #[test]
    fn only_links_and_raise_are_requests() {
        assert_eq!(super::parse(" raise\n"), Some(Request::Raise));
        assert_eq!(
            super::parse("cineo:///board"),
            Some(Request::Link("cineo:///board".to_owned()))
        );
        assert_eq!(super::parse("ping"), None);
        assert_eq!(super::parse("  "), None);
    }

    #[test]
    fn a_link_reaches_the_running_instance_through_a_private_socket() {
        let dir = std::env::temp_dir().join(format!("cineo-instance-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(
            !super::forward(&dir, Some("stremio:///board")),
            "nothing listens yet"
        );
        assert!(
            !super::forward(&dir, None),
            "a plain launch starts when nothing runs"
        );
        let (tx, rx) = channel();
        assert!(super::listen(&dir, move |request| {
            let _ = tx.send(request);
        }));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |path: std::path::PathBuf| {
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777
            };
            assert_eq!(mode(dir.join("instance")), 0o700);
            assert_eq!(mode(dir.join("instance/instance.sock")), 0o600);
        }
        assert!(
            !super::listen(&dir, |_| {}),
            "a later launch does not become the running copy"
        );
        assert!(super::forward(&dir, Some(" stremio:///library \n")));
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            Request::Link("stremio:///library".to_owned()),
            "the later launch's ping is not a link"
        );
        assert!(super::forward(&dir, None));
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            Request::Raise,
            "a plain launch raises the running copy"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
