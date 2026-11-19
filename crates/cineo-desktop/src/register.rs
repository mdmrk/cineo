#[cfg(not(target_os = "macos"))]
use std::process::Command;

#[cfg(not(target_os = "macos"))]
use anyhow::Context;
use anyhow::bail;

#[cfg(not(target_os = "macos"))]
const SCHEMES: [&str; 2] = ["stremio", "cineo"];

#[cfg(target_os = "linux")]
pub(crate) fn register() -> anyhow::Result<()> {
    let exe = std::env::current_exe().context("cannot find the Cineo executable")?;
    let exe = link_target(&exe, std::env::var_os("APPIMAGE").as_deref());
    let data = std::env::var_os("XDG_DATA_HOME")
        .filter(|d| !d.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".local/share")))
        .context("no home directory")?;
    let dir = data.join("applications");
    let mimes: String = SCHEMES
        .iter()
        .map(|s| format!("x-scheme-handler/{s};"))
        .collect();
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Cineo\nExec={} %u\nTerminal=false\nNoDisplay=true\nMimeType={mimes}\n",
        desktop_quote(&exe.to_string_lossy())
    );
    let file = "cineo-links.desktop";
    fastframe_file::write(&dir.join(file), entry)?;
    let _ = Command::new("update-desktop-database").arg(&dir).output();
    run(Command::new("xdg-mime")
        .arg("default")
        .arg(file)
        .args(SCHEMES.map(|s| format!("x-scheme-handler/{s}"))))
}

#[cfg(target_os = "linux")]
fn link_target(exe: &std::path::Path, appimage: Option<&std::ffi::OsStr>) -> std::path::PathBuf {
    appimage
        .map(std::path::Path::new)
        .filter(|image| image.is_absolute() && image.is_file())
        .map_or_else(|| exe.to_path_buf(), std::path::Path::to_path_buf)
}

#[cfg(target_os = "linux")]
fn desktop_quote(path: &str) -> String {
    let mut out = String::from("\"");
    for c in path.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

#[cfg(windows)]
pub(crate) fn register() -> anyhow::Result<()> {
    let exe = std::env::current_exe().context("cannot find the Cineo executable")?;
    let command = format!("\"{}\" \"%1\"", exe.display());
    for scheme in SCHEMES {
        let key = format!(r"HKCU\Software\Classes\{scheme}");
        for args in [
            vec![
                key.clone(),
                "/ve".into(),
                "/d".into(),
                format!("URL:{scheme}"),
            ],
            vec![
                key.clone(),
                "/v".into(),
                "URL Protocol".into(),
                "/d".into(),
                String::new(),
            ],
            vec![
                format!(r"{key}\shell\open\command"),
                "/ve".into(),
                "/d".into(),
                command.clone(),
            ],
        ] {
            let mut reg = Command::new("reg");
            std::os::windows::process::CommandExt::creation_flags(&mut reg, CREATE_NO_WINDOW);
            run(reg.arg("add").args(&args).arg("/f"))?;
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn register() -> anyhow::Result<()> {
    bail!("links are not supported on macOS yet")
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(not(target_os = "macos"))]
fn run(command: &mut Command) -> anyhow::Result<()> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = command
        .output()
        .with_context(|| format!("cannot run {program}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("{program} failed ({}): {}", output.status, stderr.trim());
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn links_open_the_appimage_file_not_its_mount() {
        let dir = std::env::temp_dir().join(format!("cineo-register-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let image = dir.join("cineo-x86_64-linux.AppImage");
        std::fs::write(&image, b"").unwrap();
        let mounted = Path::new("/tmp/.mount_cineoX/usr/lib/cineo/cineo-desktop");
        assert_eq!(link_target(mounted, Some(image.as_os_str())), image);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn links_open_the_executable_outside_an_appimage() {
        let exe = Path::new("/home/me/.local/bin/cineo-desktop");
        assert_eq!(link_target(exe, None), PathBuf::from(exe));
        assert_eq!(link_target(exe, Some("".as_ref())), PathBuf::from(exe));
        assert_eq!(
            link_target(exe, Some("relative.AppImage".as_ref())),
            PathBuf::from(exe)
        );
        assert_eq!(
            link_target(exe, Some("/nowhere/cineo.AppImage".as_ref())),
            PathBuf::from(exe)
        );
    }
}
