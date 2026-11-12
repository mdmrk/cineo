#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Context;
use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "cineo-desktop", version, about = "Cineo media center")]
struct Cli {
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
    #[arg(long)]
    allow_private_network: bool,
    #[arg(long, value_name = "DIR")]
    data_dir: Option<PathBuf>,
    #[arg(long, value_name = "DIR")]
    cache_dir: Option<PathBuf>,
    #[arg(value_name = "LINK")]
    link: Option<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let installed = cineo_desktop::dirs::installed();
    let data_dir = cli
        .data_dir
        .clone()
        .or_else(|| installed.as_ref().map(|dirs| dirs.state.clone()));
    if let Some(dir) = &data_dir {
        cineo_desktop::panic_log::record_panics(dir);
        if cineo_desktop::instance::forward(dir, cli.link.as_deref()) {
            return ExitCode::SUCCESS;
        }
    }
    cineo_desktop::logging::init(cli.verbose, data_dir.as_deref());
    let left_behind = cli
        .data_dir
        .is_none()
        .then(cineo_desktop::dirs::earlier_data_dir)
        .flatten()
        .zip(data_dir.as_deref())
        .and_then(|(earlier, current)| cineo_desktop::dirs::left_behind(current, &earlier));
    let cache_dir = installed.map(|dirs| dirs.cache);
    match start(cli, data_dir, cache_dir, left_behind) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!("{err:#}");
            ExitCode::FAILURE
        }
    }
}

fn start(
    cli: Cli,
    data_dir: Option<PathBuf>,
    cache_dir: Option<PathBuf>,
    left_behind: Option<cineo_desktop::dirs::LeftBehind>,
) -> anyhow::Result<()> {
    let data_dir = data_dir.context("no home directory; pass --data-dir")?;
    let cache_dir = cli
        .cache_dir
        .or(cache_dir)
        .unwrap_or_else(|| data_dir.join("cache"));
    cineo_desktop::libmpv::install(&cache_dir);
    cineo_desktop::run(cineo_desktop::Options {
        data_dir,
        cache_dir,
        allow_private_network: cli.allow_private_network,
        link: cli.link,
        left_behind,
    })
}
