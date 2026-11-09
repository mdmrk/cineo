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
    let data_dir = cli.data_dir.clone().or_else(cineo_store::default_data_dir);
    if let Some(dir) = &data_dir {
        cineo_desktop::panic_log::record_panics(dir);
        if let Some(link) = &cli.link
            && cineo_desktop::instance::forward(dir, link)
        {
            return ExitCode::SUCCESS;
        }
    }
    cineo_desktop::logging::init(cli.verbose, data_dir.as_deref());
    match start(cli, data_dir) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!("{err:#}");
            ExitCode::FAILURE
        }
    }
}

fn start(cli: Cli, data_dir: Option<PathBuf>) -> anyhow::Result<()> {
    let data_dir = data_dir.context("no home directory; pass --data-dir")?;
    let cache_dir = cli
        .cache_dir
        .or_else(cineo_store::default_cache_dir)
        .unwrap_or_else(|| data_dir.join("cache"));
    cineo_desktop::libmpv::install(&cache_dir);
    cineo_desktop::run(cineo_desktop::Options {
        data_dir,
        cache_dir,
        allow_private_network: cli.allow_private_network,
        link: cli.link,
    })
}
