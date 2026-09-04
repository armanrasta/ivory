//! Ivory Chain node.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use ivory_node::{init_datadir, load_datadir, run_node};
use tokio::sync::watch;

#[derive(Parser)]
#[command(name = "ivory", about = "Ivory Chain node")]
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
    Init,
    /// Run JSON-RPC, P2P, and (if validator) block production.
    Run,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    match cli.command {
        Commands::Init => {
            let paths = init_datadir(&cli.data_dir)?;
            println!("Initialized {}", paths.root.display());
            println!("  config:        {}", paths.config.display());
            println!("  genesis:       {}", paths.genesis.display());
            println!("  validator key: {}", paths.validator_key.display());
            println!("  chain db:      {}", paths.chain.display());
        }
        Commands::Run => {
            let (cfg, genesis, key, paths) = load_datadir(&cli.data_dir)?;
            let (shutdown_tx, shutdown_rx) = watch::channel(false);
            let _node = run_node(cfg, genesis, key, shutdown_rx, &paths).await?;
            tracing::info!("Ivory node running");
            tokio::signal::ctrl_c().await?;
            let _ = shutdown_tx.send(true);
        }
    }
    Ok(())
}
