//! `cineo-desktop` — the Cineo desktop app.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Context;
use clap::Parser;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "cineo-desktop", version, about = "Cineo media center")]
struct Cli {
    /// Increase log verbosity (-v debug, -vv trace). `RUST_LOG` overrides.
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
    /// Allow addons and images on loopback and private-network addresses
    /// (self-hosted addons). Off by default; see docs/SECURITY.md.
    #[arg(long)]
    allow_private_network: bool,
    /// Directory holding the database (default: the platform data directory).
    #[arg(long, value_name = "DIR")]
    data_dir: Option<PathBuf>,
    /// Path to the mpv executable (default: `mpv` on PATH).
    #[arg(long, value_name = "PATH")]
    mpv: Option<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let default = match cli.verbose {
        0 => "warn",
        1 => {
            "cineo_desktop=debug,cineo_core=debug,cineo_net=debug,cineo_player_mpv=debug,cineo_store=debug,info"
        }
        _ => "trace",
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
    match start(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!("{err:#}");
            ExitCode::FAILURE
        }
    }
}

fn start(cli: Cli) -> anyhow::Result<()> {
    let data_dir = cli
        .data_dir
        .or_else(cineo_store::default_data_dir)
        .context("the platform reports no home directory; pass --data-dir")?;
    cineo_desktop::run(cineo_desktop::Options {
        data_dir,
        allow_private_network: cli.allow_private_network,
        mpv: cli.mpv,
    })
}
