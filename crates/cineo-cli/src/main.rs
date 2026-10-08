//! `cineo` — headless shell over the Cineo core.

#![allow(clippy::print_stdout, reason = "the CLI's job is printing")]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use cineo_core::addon::{ContentType, ExtraValue, Manifest, ResourcePath, TransportUrl};
use cineo_core::app::{LibraryItem, continue_watching};
use cineo_core::diagnostics::Warning;
use cineo_net::{AddonClient, NetPolicy};
use cineo_store::{DB_FILE, Report, SCHEMA_VERSION, Store};
use clap::{Args, Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "cineo", version, about = "Cineo media center (headless shell)")]
struct Cli {
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    #[arg(long, global = true)]
    allow_private_network: bool,

    #[arg(long, global = true, value_name = "DIR")]
    data_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(subcommand)]
    Addon(AddonCommand),
    Catalog(CatalogArgs),
    Library {
        #[arg(long)]
        all: bool,
    },
    Doctor,
}

#[derive(Debug, Subcommand)]
enum AddonCommand {
    Inspect { url: String },
    Add { url: String },
    Remove { url: String },
    List,
}

#[derive(Debug, Args)]
struct CatalogArgs {
    url: String,
    #[arg(value_name = "TYPE")]
    content_type: String,
    id: String,
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
    let data_dir = cli.data_dir.or_else(cineo_store::default_data_dir);
    let data_dir = || {
        data_dir
            .as_deref()
            .context("the platform reports no home directory; pass --data-dir")
    };
    let client = || {
        AddonClient::new(NetPolicy {
            allow_private_networks: cli.allow_private_network,
            ..NetPolicy::default()
        })
    };
    match cli.command {
        Command::Addon(AddonCommand::Inspect { url }) => {
            let addon = TransportUrl::parse(&url).context("invalid addon URL")?;
            let manifest = client()?
                .fetch_manifest(&addon)
                .await
                .context("failed to load addon manifest")?;
            print!("{}", render_manifest(&manifest.value, &manifest.warnings));
        }
        Command::Addon(AddonCommand::Add { url }) => {
            let addon = TransportUrl::parse(&url).context("invalid addon URL")?;
            let mut store = open_store(data_dir()?)?;
            let mut addons = store.addons()?;
            if addons.contains(&addon) {
                bail!("this addon is already installed");
            }
            let manifest = client()?
                .fetch_manifest(&addon)
                .await
                .context("failed to load addon manifest")?;
            for warning in &manifest.warnings {
                tracing::warn!(%warning, "manifest problem");
            }
            addons.push(addon);
            store.save_addons(&addons)?;
            let manifest = manifest.value;
            println!(
                "installed {} {} ({})",
                manifest.name, manifest.version, manifest.id
            );
        }
        Command::Addon(AddonCommand::Remove { url }) => {
            let addon = TransportUrl::parse(&url).context("invalid addon URL")?;
            let mut store = open_store(data_dir()?)?;
            let mut addons = store.addons()?;
            let before = addons.len();
            addons.retain(|a| a != &addon);
            if addons.len() == before {
                bail!("this addon is not installed");
            }
            store.save_addons(&addons)?;
            println!("removed");
        }
        Command::Addon(AddonCommand::List) => {
            for (position, addon) in (1..).zip(open_store(data_dir()?)?.addons()?) {
                println!("{position}\t{addon}");
            }
        }
        Command::Library { all } => {
            let store = open_store(data_dir()?)?;
            let items = store.library()?;
            let shown = if all {
                items.iter().collect()
            } else {
                continue_watching(&items, store.settings()?.watched_at)
            };
            for item in shown {
                println!("{}", render_library_item(item));
            }
        }
        Command::Doctor => {
            let report = cineo_store::diagnose(&data_dir()?.join(DB_FILE));
            print!("{}", render_report(&report));
            if !report.is_healthy() {
                bail!("the database has problems (see above)");
            }
        }
        Command::Catalog(args) => {
            let addon = TransportUrl::parse(&args.url).context("invalid addon URL")?;
            let content_type = ContentType::new(args.content_type.as_str())
                .context("content type must be non-empty without surrounding spaces")?;
            let client = client()?;
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

fn open_store(dir: &Path) -> Result<Store> {
    Store::open_in(dir).with_context(|| format!("cannot open the database in {}", dir.display()))
}

fn render_library_item(item: &LibraryItem) -> String {
    format!(
        "{}\t{}\t{}/{}\t{}",
        item.id,
        item.video_id,
        clock(item.time_offset_ms),
        clock(item.duration_ms),
        item.name
    )
}

fn clock(ms: u64) -> String {
    let s = ms / 1000;
    format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

fn render_report(report: &Report) -> String {
    let mut out = String::new();
    let shown = |value: Option<i64>| value.map_or_else(|| "-".to_owned(), |v| v.to_string());
    let _ = writeln!(out, "database: {}", report.path.display());
    if !report.exists {
        let _ = writeln!(out, "status: not created yet (created on first use)");
        return out;
    }
    let _ = writeln!(
        out,
        "schema version: {} (supported: {SCHEMA_VERSION})",
        shown(report.schema_version)
    );
    let _ = writeln!(out, "addons: {}", shown(report.addons));
    let _ = writeln!(out, "library items: {}", shown(report.library_items));
    if !report.integrity.is_empty() {
        let _ = writeln!(out, "integrity: {}", report.integrity.join("; "));
    }
    if let Some(error) = &report.error {
        let _ = writeln!(out, "error: {error}");
    }
    out
}

fn render_manifest(manifest: &Manifest, warnings: &[Warning]) -> String {
    let mut out = String::new();
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
