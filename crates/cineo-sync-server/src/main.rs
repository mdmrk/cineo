#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
mod native_main {
    use std::net::SocketAddr;
    use std::path::PathBuf;
    use std::sync::Arc;

    use anyhow::Context as _;
    use cineo_sync_server::{ServeOptions, SqliteDb, serve};
    use clap::Parser;
    use tokio::net::TcpListener;
    use tracing::info;
    use tracing_subscriber::EnvFilter;

    #[derive(Debug, Parser)]
    #[command(
        version,
        about = "Cineo sync server: stores end-to-end encrypted records it cannot read"
    )]
    struct Args {
        #[arg(long, default_value = "127.0.0.1:8787")]
        listen: SocketAddr,
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        trust_forwarded_for: bool,
    }

    #[tokio::main]
    pub(crate) async fn main() -> anyhow::Result<()> {
        tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
            )
            .init();
        let args = Args::parse();
        let db = SqliteDb::open(&args.database)
            .with_context(|| format!("opening {}", args.database.display()))?;
        let listener = TcpListener::bind(args.listen)
            .await
            .with_context(|| format!("listening on {}", args.listen))?;
        info!(address = %listener.local_addr()?, "serving sync");
        serve(
            listener,
            Arc::new(db),
            ServeOptions {
                trust_forwarded_for: args.trust_forwarded_for,
                ..ServeOptions::default()
            },
        )
        .await;
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    native_main::main()
}
