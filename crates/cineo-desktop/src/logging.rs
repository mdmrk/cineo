use std::fs::{DirBuilder, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::Mutex;

use tracing::Subscriber;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, fmt};

const LOG: &str = "cineo.log";
const PREVIOUS_LOG: &str = "cineo.previous.log";

const VERBOSE: &str = "cineo_desktop=debug,cineo_core=debug,cineo_net=debug,cineo_player=debug,cineo_store=debug,cineo_stream=debug,info";
const QUIET_FILE: &str = "error,cineo_desktop=warn,cineo_core=warn,cineo_net=warn,cineo_player=warn,cineo_store=warn,cineo_stream=warn";

pub fn init(verbose: u8, data_dir: Option<&Path>) {
    let (file, failure) = match data_dir.map(open_log) {
        Some(Ok(file)) => (Some(file), None),
        Some(Err(err)) => (None, Some(err)),
        None => (None, None),
    };
    let stderr = fmt::layer()
        .with_writer(io::stderr)
        .with_filter(filter(stderr_directives(verbose)));
    tracing_subscriber::registry()
        .with(stderr)
        .with(file.map(|file| file_layer(file, verbose)))
        .init();
    if let Some(err) = failure {
        tracing::warn!(%err, "the log file could not be created");
    }
}

fn filter(directives: &str) -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(directives))
}

fn stderr_directives(verbose: u8) -> &'static str {
    match verbose {
        0 => "warn",
        1 => VERBOSE,
        _ => "trace",
    }
}

fn file_directives(verbose: u8) -> &'static str {
    match verbose {
        0 => QUIET_FILE,
        level => stderr_directives(level),
    }
}

fn file_layer<S>(file: File, verbose: u8) -> impl Layer<S>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    fmt::layer()
        .with_ansi(false)
        .with_writer(Mutex::new(file))
        .with_filter(filter(file_directives(verbose)))
}

fn open_log(data_dir: &Path) -> io::Result<File> {
    create_private_dir(data_dir)?;
    let log = data_dir.join(LOG);
    match std::fs::rename(&log, data_dir.join(PREVIOUS_LOG)) {
        Err(err) if err.kind() != io::ErrorKind::NotFound => return Err(err),
        _ => {}
    }
    let mut options = OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(log)?;
    writeln!(
        file,
        "Cineo {} on {} ({})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    )?;
    Ok(file)
}

fn create_private_dir(dir: &Path) -> io::Result<()> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test assertions")]

    use tracing_subscriber::layer::SubscriberExt;

    use super::*;

    #[test]
    fn each_run_starts_a_log_and_keeps_the_one_before() {
        let dir = tempfile::tempdir().unwrap();
        let mut first = open_log(dir.path()).unwrap();
        writeln!(first, "the first run").unwrap();
        let mut second = open_log(dir.path()).unwrap();
        writeln!(second, "the second run").unwrap();
        let read = |name| std::fs::read_to_string(dir.path().join(name)).unwrap();
        assert!(read(PREVIOUS_LOG).contains("the first run"));
        let current = read(LOG);
        assert!(current.starts_with("Cineo "), "{current}");
        assert!(current.contains("the second run"));
        assert!(!current.contains("the first run"));
    }

    #[cfg(unix)]
    #[test]
    fn the_log_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        open_log(dir.path()).unwrap();
        let mode = std::fs::metadata(dir.path().join(LOG))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn a_log_that_cannot_be_created_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let not_a_dir = dir.path().join("file");
        std::fs::write(&not_a_dir, b"").unwrap();
        assert!(open_log(&not_a_dir).is_err());
    }

    fn logged_by_default(emit: impl FnOnce()) -> String {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log");
        let subscriber =
            tracing_subscriber::registry().with(file_layer(File::create(&path).unwrap(), 0));
        tracing::subscriber::with_default(subscriber, emit);
        std::fs::read_to_string(path).unwrap()
    }

    #[test]
    fn by_default_the_file_keeps_cineo_warnings_and_nothing_about_what_is_watched() {
        let log = logged_by_default(|| {
            let span = tracing::info_span!(target: "cineo_net", "addon_request", resource = "stream/movie/tt1.json");
            let _entered = span.enter();
            tracing::info!(target: "cineo_stream", info_hash = "abc", "torrent initialized");
            tracing::warn!(target: "cineo_net", "kept warning");
        });
        assert!(log.contains("kept warning"), "{log}");
        assert!(!log.contains("tt1"), "{log}");
        assert!(!log.contains("abc"), "{log}");
    }

    #[test]
    fn by_default_other_crates_only_reach_the_file_with_errors() {
        let log = logged_by_default(|| {
            tracing::warn!(target: "librqbit", "a tracker name");
            tracing::error!(target: "librqbit", "kept error");
        });
        assert!(log.contains("kept error"), "{log}");
        assert!(!log.contains("a tracker name"), "{log}");
    }
}
