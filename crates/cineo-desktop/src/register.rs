#[cfg(any(windows, all(unix, not(target_os = "macos"))))]
use std::process::Command;

#[cfg(any(windows, all(unix, not(target_os = "macos"))))]
use anyhow::Context;
use anyhow::bail;

#[cfg(any(windows, all(unix, not(target_os = "macos"))))]
const SCHEMES: [&str; 2] = ["stremio", "cineo"];

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn register() -> anyhow::Result<()> {
    let exe = std::env::current_exe().context("cannot find the Cineo executable")?;
    let data = std::env::var_os("XDG_DATA_HOME")
        .filter(|d| !d.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".local/share")))
        .context("no home directory")?;
    let dir = data.join("applications");
    std::fs::create_dir_all(&dir)?;
    let mimes: String = SCHEMES
        .iter()
        .map(|s| format!("x-scheme-handler/{s};"))
        .collect();
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Cineo\nExec={} %u\nTerminal=false\nNoDisplay=true\nMimeType={mimes}\n",
        desktop_quote(&exe.to_string_lossy())
    );
    let file = "cineo-links.desktop";
    std::fs::write(dir.join(file), entry)?;
    let _ = Command::new("update-desktop-database").arg(&dir).status();
    let status = Command::new("xdg-mime")
        .arg("default")
        .arg(file)
        .args(SCHEMES.map(|s| format!("x-scheme-handler/{s}")))
        .status()
        .context("cannot run xdg-mime")?;
    if !status.success() {
        bail!("xdg-mime failed ({status})");
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
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
            let status = Command::new("reg")
                .arg("add")
                .args(&args)
                .arg("/f")
                .status()
                .context("cannot run reg.exe")?;
            if !status.success() {
                bail!("reg.exe failed ({status})");
            }
        }
    }
    Ok(())
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
pub(crate) fn register() -> anyhow::Result<()> {
    bail!("not supported on this platform yet")
}
