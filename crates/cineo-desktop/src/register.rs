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
    let _ = Command::new("update-desktop-database").arg(&dir).output();
    run(Command::new("xdg-mime")
        .arg("default")
        .arg(file)
        .args(SCHEMES.map(|s| format!("x-scheme-handler/{s}"))))
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
            run(Command::new("reg").arg("add").args(&args).arg("/f"))?;
        }
    }
    Ok(())
}

#[cfg(any(windows, all(unix, not(target_os = "macos"))))]
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

#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
pub(crate) fn register() -> anyhow::Result<()> {
    bail!("not supported on this platform yet")
}
