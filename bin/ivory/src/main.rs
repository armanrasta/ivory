//! Ivory server (node): init and run. Contract deploy lives in `ivory-dev`.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use ivory_node::{InitOpts, ServerRole, init_datadir_with, load_datadir, run_node};
use tokio::sync::watch;

#[derive(Parser)]
#[command(name = "ivory", about = "Ivory server: permissioned ledger node")]
struct Cli {
    /// Data directory (config, genesis, validator key).
    #[arg(long, global = true, default_value = "./ivory-data")]
    data_dir: PathBuf,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Write genesis, config, and a validator key.
    Init {
        /// `master` (may produce) or `slave` (follow only). Aliases: validator, follower.
        #[arg(long, default_value = "master")]
        role: String,
        /// Bootstrap multiaddrs (join another server or hosted P2P).
        #[arg(long)]
        bootstrap: Vec<String>,
    },
    /// Run JSON-RPC, P2P, and (if master and authorized) block production.
    Run,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    match cli.command {
        Commands::Init { role, bootstrap } => {
            let opts = InitOpts {
                role: ServerRole::parse(&role)?,
                bootstrap,
            };
            let paths = init_datadir_with(&cli.data_dir, opts)?;
            println!("Initialized {} ({role} server)", paths.root.display());
            println!("  config:        {}", paths.config.display());
            println!("  genesis:       {}", paths.genesis.display());
            println!("  validator key: {}", paths.validator_key.display());
            println!("  chain db:      {}", paths.chain.display());
            println!("  contracts:     {}", paths.contracts.display());
        }
        Commands::Run => {
            let (cfg, genesis, key, paths) = load_datadir(&cli.data_dir)?;
            let (shutdown_tx, shutdown_rx) = watch::channel(false);
            let _node = run_node(cfg, genesis, key, shutdown_rx, &paths).await?;
            tracing::info!("Ivory server running");
            tokio::signal::ctrl_c().await?;
            let _ = shutdown_tx.send(true);
        }
    }
    Ok(())
}
