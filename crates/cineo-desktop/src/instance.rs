use std::path::Path;
use std::sync::OnceLock;

use fastframe_instance::{Claim, Guard, Slot};

const NAME: &str = "cineo";
const PING: &str = "ping";
const OK: &str = "ok";

static RUNNING_COPY: OnceLock<Guard> = OnceLock::new();

fn slot(data_dir: &Path) -> Slot {
    Slot::at(data_dir.join("instance"), NAME)
}

pub fn forward(data_dir: &Path, link: &str) -> bool {
    slot(data_dir).send(link.trim()).is_ok()
}

pub(crate) fn listen(data_dir: &Path, on_link: impl Fn(String) + Send + 'static) -> bool {
    let claim = slot(data_dir).claim(PING, move |request| {
        let link = request.trim();
        if !link.is_empty() && link != PING {
            on_link(link.to_owned());
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

    #[test]
    fn a_link_reaches_the_running_instance_through_a_private_socket() {
        let dir = std::env::temp_dir().join(format!("cineo-instance-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(
            !super::forward(&dir, "stremio:///board"),
            "nothing listens yet"
        );
        let (tx, rx) = channel();
        assert!(super::listen(&dir, move |link| {
            let _ = tx.send(link);
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
        assert!(super::forward(&dir, " stremio:///library \n"));
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            "stremio:///library",
            "the later launch's ping is not a link"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
