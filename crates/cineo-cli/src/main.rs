//! `cineo` — headless shell over the Cineo core.
//!
//! It is the first end-to-end front end and the main debugging tool for
//! addon behavior (see `docs/DEBUGGING.md`). Output goes to stdout; logs go to
//! stderr and are controlled by `-v` or `RUST_LOG`.

// The CLI's job is printing.
#![allow(clippy::print_stdout)]

use std::fmt::Write as _;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use cineo_core::addon::{ContentType, ExtraValue, Manifest, ResourcePath, TransportUrl};
use cineo_core::diagnostics::Warning;
use cineo_net::{AddonClient, NetPolicy};
use clap::{Args, Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "cineo", version, about = "Cineo media center (headless shell)")]
struct Cli {
    /// Increase log verbosity (-v debug, -vv trace). `RUST_LOG` overrides.
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Allow requests to loopback and private-network addresses
    /// (self-hosted addons). Off by default; see docs/SECURITY.md.
    #[arg(long, global = true)]
    allow_private_network: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Work with addons.
    #[command(subcommand)]
    Addon(AddonCommand),
    /// Fetch one page of a catalog and list its items.
    Catalog(CatalogArgs),
}

#[derive(Debug, Subcommand)]
enum AddonCommand {
    /// Fetch, validate and summarize an addon manifest.
    Inspect {
        /// URL of the addon's manifest.json.
        url: String,
    },
}

#[derive(Debug, Args)]
struct CatalogArgs {
    /// URL of the addon's manifest.json.
    url: String,
    /// Content type, e.g. `movie` or `series`.
    #[arg(value_name = "TYPE")]
    content_type: String,
    /// Catalog id as declared in the manifest.
    id: String,
    /// Extra argument `name=value`, repeatable (e.g. `--extra search=matrix`).
    #[arg(long = "extra", value_name = "NAME=VALUE", value_parser = parse_extra)]
    extra: Vec<ExtraValue>,
}

fn parse_extra(raw: &str) -> Result<ExtraValue, String> {
    let (name, value) = raw
        .split_once('=')
        .ok_or_else(|| format!("expected NAME=VALUE, got `{raw}`"))?;
    if name.is_empty() {
        return Err("extra name must not be empty".to_owned());
    }
    Ok(ExtraValue::new(name, value))
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    init_tracing(cli.verbose);
    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            // `{:#}` prints the whole context chain on one line.
            tracing::error!("{err:#}");
            eprintln_error(&err);
            ExitCode::FAILURE
        }
    }
}

#[allow(clippy::print_stderr)]
fn eprintln_error(err: &anyhow::Error) {
    eprintln!("error: {err:#}");
}

fn init_tracing(verbose: u8) {
    let default = match verbose {
        0 => "warn",
        1 => "cineo=debug,cineo_core=debug,cineo_net=debug,info",
        _ => "trace",
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

async fn run(cli: Cli) -> Result<()> {
    let client = AddonClient::new(NetPolicy {
        allow_private_networks: cli.allow_private_network,
        ..NetPolicy::default()
    })?;
    match cli.command {
        Command::Addon(AddonCommand::Inspect { url }) => {
            let addon = TransportUrl::parse(&url).context("invalid addon URL")?;
            let manifest = client
                .fetch_manifest(&addon)
                .await
                .context("failed to load addon manifest")?;
            print!("{}", render_manifest(&manifest.value, &manifest.warnings));
        }
        Command::Catalog(args) => {
            let addon = TransportUrl::parse(&args.url).context("invalid addon URL")?;
            let content_type = ContentType::new(args.content_type.as_str())
                .context("content type must be non-empty without surrounding spaces")?;
            let manifest = client
                .fetch_manifest(&addon)
                .await
                .context("failed to load addon manifest")?
                .value;
            let path = ResourcePath::catalog(content_type, args.id).with_extra(args.extra);
            let Some(catalog) = manifest.catalog(&path.content_type, &path.id) else {
                bail!(
                    "addon does not declare catalog {}/{}",
                    path.content_type,
                    path.id
                );
            };
            catalog
                .check_extra(&path.extra)
                .context("extra arguments rejected by the catalog declaration")?;
            let response = client
                .fetch_catalog(&addon, &path)
                .await
                .context("failed to load catalog")?;
            for meta in &response.value.metas {
                let year = meta.release_info.as_deref().unwrap_or("-");
                println!(
                    "{}\t{}\t{}\t{}",
                    meta.id, meta.content_type, year, meta.name
                );
            }
            for warning in &response.warnings {
                tracing::warn!(%warning, "catalog entry problem");
            }
        }
    }
    Ok(())
}

fn render_manifest(manifest: &Manifest, warnings: &[Warning]) -> String {
    let mut out = String::new();
    // Writing to a String cannot fail.
    let _ = writeln!(
        out,
        "{} {} ({})",
        manifest.name, manifest.version, manifest.id
    );
    if let Some(description) = &manifest.description {
        let _ = writeln!(out, "  {description}");
    }
    let types: Vec<_> = manifest.types.iter().map(ContentType::as_str).collect();
    let _ = writeln!(out, "types: {}", types.join(", "));
    let _ = writeln!(out, "resources:");
    for resource in &manifest.resources {
        let types: Vec<_> = resource.types.iter().map(ContentType::as_str).collect();
        let _ = writeln!(
            out,
            "  {} [{}] ids: {:?}",
            resource.name,
            types.join(", "),
            resource.ids
        );
    }
    let _ = writeln!(out, "catalogs:");
    for catalog in &manifest.catalogs {
        let extras: Vec<String> = catalog
            .extra
            .iter()
            .map(|e| {
                if e.is_required {
                    format!("{}*", e.name)
                } else {
                    e.name.clone()
                }
            })
            .collect();
        let _ = writeln!(
            out,
            "  {}/{} \"{}\" extra: [{}]",
            catalog.content_type,
            catalog.id,
            catalog.name.as_deref().unwrap_or(""),
            extras.join(", ")
        );
    }
    let hints = &manifest.behavior_hints;
    if hints.adult || hints.p2p || hints.configurable || hints.configuration_required {
        let _ = writeln!(out, "behavior hints: {hints:?}");
    }
    if !warnings.is_empty() {
        let _ = writeln!(out, "warnings ({}):", warnings.len());
        for warning in warnings {
            let _ = writeln!(out, "  {warning}");
        }
    }
    out
}
