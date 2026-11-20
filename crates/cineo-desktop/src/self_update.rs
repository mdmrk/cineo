use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use aws_lc_rs::digest::{SHA256, digest};
use cineo_core::app::{UpdateFailure, UpdateRequest, UpdateTarget};
use cineo_net::{AddonClient, NetPolicy};
use minisign_verify::{PublicKey, Signature};
use semver::Version;
use tracing::{info, warn};

pub const PUBLIC_KEY: &str = "RWQPKOI7lXW1zBfoRntGmEe8QJf5TdJTDUZ4NEDFJtrSNdNmxrdBpHln";
const AFTER_UPDATE: &str = "--after-update";
const LICENSES: &str = "cineo-licenses.zip";
const INSTALLED_NAMES: [&str; 2] = if cfg!(windows) {
    ["cineo.exe", "cineo-desktop.exe"]
} else {
    ["cineo", "cineo-desktop"]
};
const TARGET: Option<UpdateTarget> = if cfg!(windows) {
    Some(UpdateTarget::Windows)
} else if cfg!(target_os = "linux") {
    Some(UpdateTarget::Linux)
} else {
    None
};
const MAX_DOWNLOAD: usize = 256 * 1024 * 1024;

pub(crate) fn policy() -> NetPolicy {
    NetPolicy {
        max_body_bytes: MAX_DOWNLOAD,
        timeout: Duration::from_secs(600),
        ..NetPolicy::default()
    }
}

pub(crate) fn target(exe: &Path) -> Option<UpdateTarget> {
    TARGET.filter(|_| can_update(exe))
}

fn can_update(exe: &Path) -> bool {
    if exe
        .file_name()
        .is_none_or(|name| !INSTALLED_NAMES.iter().any(|installed| name == *installed))
    {
        return false;
    }
    let Some(dir) = exe.parent() else {
        return false;
    };
    let probe = dir.join(".cineo-update-probe");
    let writable = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .is_ok();
    let _ = std::fs::remove_file(&probe);
    writable
}

fn verify_sums(
    key: &str,
    version: &Version,
    sums: &[u8],
    signature: &[u8],
) -> Result<(), UpdateFailure> {
    let key = PublicKey::from_base64(key).map_err(|_| UpdateFailure::Signature)?;
    let signature = std::str::from_utf8(signature)
        .ok()
        .and_then(|text| Signature::decode(text).ok())
        .ok_or(UpdateFailure::Signature)?;
    key.verify(sums, &signature, false)
        .map_err(|_| UpdateFailure::Signature)?;
    if signature.trusted_comment() == format!("cineo v{version} SHA256SUMS") {
        Ok(())
    } else {
        Err(UpdateFailure::Signature)
    }
}

fn check_file(sums: &[u8], name: &str, bytes: &[u8]) -> Result<(), UpdateFailure> {
    let expected = std::str::from_utf8(sums)
        .ok()
        .and_then(|text| {
            text.lines().find_map(|line| {
                let (hash, file) = line.split_once(char::is_whitespace)?;
                (file.trim_start().trim_start_matches('*') == name)
                    .then(|| hash.to_ascii_lowercase())
            })
        })
        .ok_or(UpdateFailure::Checksum)?;
    let actual: String = digest(&SHA256, bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if actual == expected {
        Ok(())
    } else {
        Err(UpdateFailure::Checksum)
    }
}

fn beside(exe: &Path, suffix: &str) -> PathBuf {
    let mut name = exe.file_name().map_or_else(OsString::new, OsString::from);
    name.push(suffix);
    exe.with_file_name(name)
}

fn swap(exe: &Path, new: &[u8]) -> Result<(), UpdateFailure> {
    let staged = beside(exe, ".new");
    let old = beside(exe, ".old");
    let _ = std::fs::remove_file(&old);
    let _ = std::fs::remove_file(&staged);
    write_executable(&staged, new).map_err(|_| UpdateFailure::Install)?;
    if std::fs::rename(exe, &old).is_err() {
        let _ = std::fs::remove_file(&staged);
        return Err(UpdateFailure::Install);
    }
    if std::fs::rename(&staged, exe).is_err() {
        let _ = std::fs::rename(&old, exe);
        let _ = std::fs::remove_file(&staged);
        return Err(UpdateFailure::Install);
    }
    Ok(())
}

fn write_executable(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o755);
    options.open(path)?.write_all(bytes)
}

pub fn remove_leftover(exe: &Path) {
    let old = beside(exe, ".old");
    if old.exists() {
        match std::fs::remove_file(&old) {
            Ok(()) => info!(file = %old.display(), "removed the executable an update replaced"),
            Err(err) => warn!(%err, file = %old.display(), "cannot remove the replaced executable"),
        }
    }
}

#[cfg(windows)]
fn system_tar() -> PathBuf {
    std::env::var_os("SystemRoot")
        .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from)
        .join(r"System32\tar.exe")
}

#[cfg(windows)]
fn replace_licenses(dir: &Path, zip: &[u8]) -> Result<(), UpdateFailure> {
    use std::os::windows::process::CommandExt as _;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let licenses = dir.join("licenses");
    let staging = dir.join(".licenses-update");
    let archive = dir.join(".licenses-update.zip");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|_| UpdateFailure::Install)?;
    std::fs::write(&archive, zip).map_err(|_| UpdateFailure::Install)?;
    let extracted = std::process::Command::new(system_tar())
        .arg("-xf")
        .arg(&archive)
        .arg("-C")
        .arg(&staging)
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .is_ok_and(|status| status.success());
    let _ = std::fs::remove_file(&archive);
    let unpacked = staging.join("cineo-licenses");
    let replaced = extracted
        && unpacked.is_dir()
        && std::fs::remove_dir_all(&licenses).is_ok()
        && std::fs::rename(&unpacked, &licenses).is_ok();
    let _ = std::fs::remove_dir_all(&staging);
    if replaced {
        Ok(())
    } else {
        Err(UpdateFailure::Install)
    }
}

#[cfg(not(windows))]
fn replace_licenses(_dir: &Path, _zip: &[u8]) -> Result<(), UpdateFailure> {
    Err(UpdateFailure::Install)
}

#[derive(Default)]
struct Throttle(Option<u64>);

impl Throttle {
    fn due(&mut self, received: u64, total: Option<u64>) -> bool {
        let step = match total {
            Some(total) if total > 0 => received.saturating_mul(100) / total,
            _ => received / (1024 * 1024),
        };
        self.0.replace(step) != Some(step)
    }
}

async fn download(
    client: &AddonClient,
    url: &url::Url,
    progress: impl FnMut(u64, Option<u64>) + Send,
) -> Result<Vec<u8>, UpdateFailure> {
    client
        .fetch_release_file(url, progress)
        .await
        .map_err(|err| {
            warn!(%err, %url, "update download failed");
            UpdateFailure::Download
        })
}

pub(crate) async fn install(
    client: &AddonClient,
    request: &UpdateRequest,
    exe: PathBuf,
    progress: impl Fn(u64, Option<u64>) + Send + Sync,
) -> Result<(), UpdateFailure> {
    let sums = download(client, &request.sums, |_, _| {}).await?;
    let signature = download(client, &request.signature, |_, _| {}).await?;
    verify_sums(PUBLIC_KEY, &request.version, &sums, &signature)?;
    let mut throttle = Throttle::default();
    let executable = download(client, &request.executable, |received, total| {
        if throttle.due(received, total) {
            progress(received, total);
        }
    })
    .await?;
    check_file(&sums, request.target.executable(), &executable)?;
    let dir = exe.parent().map_or_else(PathBuf::new, Path::to_path_buf);
    let licenses = if request.target == UpdateTarget::Windows && dir.join("licenses").is_dir() {
        let zip = download(client, &request.licenses, |_, _| {}).await?;
        check_file(&sums, LICENSES, &zip)?;
        Some(zip)
    } else {
        None
    };
    tokio::task::spawn_blocking(move || {
        if let Some(zip) = licenses {
            replace_licenses(&dir, &zip)?;
        }
        swap(&exe, &executable)
    })
    .await
    .map_err(|_| UpdateFailure::Install)??;
    info!(version = %request.version, "update installed; it runs after a restart");
    Ok(())
}

pub(crate) fn restart(exe: &Path) -> std::io::Result<()> {
    let args: Vec<OsString> = std::env::args_os()
        .skip(1)
        .filter(|arg| arg != AFTER_UPDATE)
        .collect();
    std::process::Command::new(exe)
        .args(args)
        .arg(AFTER_UPDATE)
        .spawn()
        .map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/update");

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(Path::new(FIXTURES).join(name)).unwrap()
    }

    fn test_key() -> String {
        String::from_utf8(fixture("test.pub"))
            .unwrap()
            .lines()
            .nth(1)
            .unwrap()
            .to_owned()
    }

    #[test]
    fn checksums_signed_for_the_offered_version_verify() {
        let version = Version::new(999, 1, 0);
        let sums = fixture("SHA256SUMS");
        assert_eq!(
            verify_sums(&test_key(), &version, &sums, &fixture("SHA256SUMS.minisig")),
            Ok(())
        );
    }

    #[test]
    fn checksums_signed_for_another_version_or_key_or_changed_are_refused() {
        let version = Version::new(999, 1, 0);
        let sums = fixture("SHA256SUMS");
        let signature = fixture("SHA256SUMS.minisig");
        let refused = Err(UpdateFailure::Signature);
        assert_eq!(
            verify_sums(&test_key(), &version, &sums, &fixture("older.minisig")),
            refused
        );
        assert_eq!(
            verify_sums(&test_key(), &version, &sums, &fixture("other-key.minisig")),
            refused
        );
        let mut changed = sums.clone();
        changed[0] ^= 1;
        assert_eq!(
            verify_sums(&test_key(), &version, &changed, &signature),
            refused
        );
        assert_eq!(
            verify_sums(PUBLIC_KEY, &version, &sums, &signature),
            refused
        );
        assert_eq!(
            verify_sums(&test_key(), &version, &sums, b"not a signature"),
            refused
        );
    }

    #[test]
    fn a_file_must_match_its_line_in_the_checksums() {
        let sums = fixture("SHA256SUMS");
        assert_eq!(
            check_file(&sums, UpdateTarget::Windows.executable(), b"new executable"),
            Ok(())
        );
        assert_eq!(check_file(&sums, LICENSES, b"licenses zip"), Ok(()));
        assert_eq!(
            check_file(
                &sums,
                UpdateTarget::Windows.executable(),
                b"another executable"
            ),
            Err(UpdateFailure::Checksum)
        );
        assert_eq!(
            check_file(&sums, "cineo-x86_64-linux", b"new executable"),
            Err(UpdateFailure::Checksum)
        );
    }

    #[test]
    fn the_new_executable_takes_the_old_ones_place() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join(INSTALLED_NAMES[0]);
        std::fs::write(&exe, b"old").unwrap();
        assert_eq!(swap(&exe, b"new"), Ok(()));
        assert_eq!(std::fs::read(&exe).unwrap(), b"new");
        assert_eq!(std::fs::read(beside(&exe, ".old")).unwrap(), b"old");
        assert!(!beside(&exe, ".new").exists());
        remove_leftover(&exe);
        assert!(!beside(&exe, ".old").exists());
    }

    #[test]
    fn progress_is_reported_once_per_percent_or_per_mebibyte() {
        let mut throttle = Throttle::default();
        let reported: Vec<u64> = [0, 1, 99, 100, 150, 199, 200, 10_000]
            .into_iter()
            .filter(|&received| throttle.due(received, Some(10_000)))
            .collect();
        assert_eq!(reported, [0, 100, 200, 10_000]);
        let mut throttle = Throttle::default();
        const MIB: u64 = 1024 * 1024;
        let reported: Vec<u64> = [1, MIB - 1, MIB, MIB + 5, 3 * MIB]
            .into_iter()
            .filter(|&received| throttle.due(received, None))
            .collect();
        assert_eq!(reported, [1, MIB, 3 * MIB]);
    }

    #[test]
    fn a_failed_swap_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join(INSTALLED_NAMES[0]);
        assert_eq!(swap(&exe, b"new"), Err(UpdateFailure::Install));
        assert!(!exe.exists());
        assert!(!beside(&exe, ".new").exists());
    }

    #[test]
    fn only_a_writable_installed_cineo_updates_itself() {
        let dir = tempfile::tempdir().unwrap();
        assert!(can_update(&dir.path().join(INSTALLED_NAMES[0])));
        assert!(can_update(&dir.path().join(INSTALLED_NAMES[1])));
        assert!(!can_update(&dir.path().join("cineo-x86_64-windows.exe")));
        assert!(!can_update(
            &dir.path().join("missing").join(INSTALLED_NAMES[0])
        ));
        assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn the_new_executable_can_run() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join(INSTALLED_NAMES[0]);
        std::fs::write(&exe, b"old").unwrap();
        std::fs::write(beside(&exe, ".new"), b"stale").unwrap();
        assert_eq!(swap(&exe, b"new"), Ok(()));
        let mode = std::fs::metadata(&exe).unwrap().permissions().mode();
        assert_eq!(mode & 0o100, 0o100, "mode {mode:o}");
        assert_eq!(std::fs::read(&exe).unwrap(), b"new");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_running_executable_can_be_swapped_on_linux() {
        if std::env::var_os("CINEO_SWAP_CHILD").is_some() {
            std::thread::sleep(Duration::from_secs(5));
            return;
        }
        let me = std::env::current_exe().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join(INSTALLED_NAMES[0]);
        std::fs::copy(&me, &exe).unwrap();
        let mut running = std::process::Command::new(&exe)
            .args([
                "--exact",
                "self_update::tests::a_running_executable_can_be_swapped_on_linux",
            ])
            .env("CINEO_SWAP_CHILD", "1")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        let swapped = swap(&exe, &std::fs::read(&me).unwrap());
        let still_running = running.try_wait().unwrap().is_none();
        let _ = running.kill();
        let _ = running.wait();
        assert_eq!(swapped, Ok(()));
        assert!(still_running);
        assert!(
            std::process::Command::new(&exe)
                .arg("--list")
                .stdout(std::process::Stdio::null())
                .status()
                .unwrap()
                .success()
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_running_executable_can_be_swapped() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join(INSTALLED_NAMES[0]);
        let system = std::env::var_os("SystemRoot")
            .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
        std::fs::copy(system.join(r"System32\ping.exe"), &exe).unwrap();
        let mut running = std::process::Command::new(&exe)
            .args(["-n", "5", "127.0.0.1"])
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        assert!(
            std::fs::write(&exe, b"new").is_err(),
            "a running executable cannot be overwritten"
        );
        let swapped = swap(&exe, b"new");
        let _ = running.kill();
        let _ = running.wait();
        assert_eq!(swapped, Ok(()));
        assert_eq!(std::fs::read(&exe).unwrap(), b"new");
    }

    #[cfg(windows)]
    #[test]
    fn the_license_texts_come_from_the_zip() {
        let dir = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        std::fs::create_dir(source.path().join("cineo-licenses")).unwrap();
        std::fs::write(source.path().join("cineo-licenses").join("NEW.txt"), b"new").unwrap();
        let zip = source.path().join("licenses.zip");
        let status = std::process::Command::new(system_tar())
            .arg("-a")
            .arg("-cf")
            .arg(&zip)
            .arg("-C")
            .arg(source.path())
            .arg("cineo-licenses")
            .status()
            .unwrap();
        assert!(status.success());
        std::fs::create_dir(dir.path().join("licenses")).unwrap();
        std::fs::write(dir.path().join("licenses").join("OLD.txt"), b"old").unwrap();
        assert_eq!(
            replace_licenses(dir.path(), &std::fs::read(&zip).unwrap()),
            Ok(())
        );
        assert!(dir.path().join("licenses").join("NEW.txt").is_file());
        assert!(!dir.path().join("licenses").join("OLD.txt").exists());
        let mut left: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        left.sort();
        assert_eq!(left, ["licenses"]);
    }
}
