//! ivory-dev CLI.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use ivory_dev::{
    ChainTarget, find_project, load_dev_config, new_project, resolve_key_path, resolve_rpc,
};
use ivory_node::{deploy_contract, load_secret_key};
use serde_json::{Value, json};

#[derive(Parser)]
#[command(name = "ivory-dev", about = "Ivory project toolchain")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scaffold a contract project (not a server data-dir).
    New {
        /// Directory to create.
        dir: PathBuf,
    },
    /// Compile a YAML/WAT/WASM file and CREATE it on a server.
    Deploy {
        /// Manifest or source. Defaults to `contracts/tracker.yaml` in the project.
        path: Option<PathBuf>,
        /// JSON-RPC URL (overrides --chain and ivory.toml).
        #[arg(long)]
        rpc: Option<String>,
        /// `local` or `public` (`IVORY_PUBLIC_RPC`).
        #[arg(long)]
        chain: Option<String>,
        /// Ed25519 secret hex file.
        #[arg(long)]
        key: Option<PathBuf>,
        /// Server data-dir (uses `validator.key` and copies catalog into `contracts/`).
        #[arg(long)]
        data_dir: Option<PathBuf>,
    },
    /// Ping `eth_chainId` and `ivory_nodeInfo`.
    Status {
        #[arg(long)]
        rpc: Option<String>,
        #[arg(long)]
        chain: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::New { dir } => {
            new_project(&dir)?;
            println!("Created {}", dir.display());
            println!("  ivory.toml");
            println!("  contracts/tracker.yaml");
        }
        Commands::Deploy {
            path,
            rpc,
            chain,
            key,
            data_dir,
        } => {
            let cwd = std::env::current_dir()?;
            let project = find_project(&cwd).unwrap_or(cwd);
            let cfg = load_dev_config(&project)?;
            let chain = chain.as_deref().map(ChainTarget::parse).transpose()?;
            let rpc_url = resolve_rpc(rpc.as_deref(), chain, &cfg)?;
            let key_path = resolve_key_path(key.as_deref(), data_dir.as_deref(), &project, &cfg)?;
            let sk = load_secret_key(&key_path)
                .with_context(|| format!("load key {}", key_path.display()))?;
            let manifest = path.unwrap_or_else(|| project.join("contracts/tracker.yaml"));
            if !manifest.exists() {
                bail!("no contract file at {}", manifest.display());
            }
            let catalog = data_dir
                .as_ref()
                .map(|d| ivory_node::DataPaths::new(d.clone()).contracts)
                .unwrap_or_else(|| project.join("contracts"));
            std::fs::create_dir_all(&catalog)?;
            deploy_contract(&manifest, &rpc_url, &sk, &catalog)?;
        }
        Commands::Status { rpc, chain } => {
            let cwd = std::env::current_dir()?;
            let project = find_project(&cwd).unwrap_or(cwd);
            let cfg = load_dev_config(&project)?;
            let chain = chain.as_deref().map(ChainTarget::parse).transpose()?;
            let rpc_url = resolve_rpc(rpc.as_deref(), chain, &cfg)?;
            let chain_id = rpc_call(&rpc_url, "eth_chainId", json!([]))?;
            println!("rpc      {rpc_url}");
            println!("chainId  {chain_id}");
            match rpc_call(&rpc_url, "ivory_nodeInfo", json!([])) {
                Ok(info) => {
                    println!("role     {}", info.get("role").unwrap_or(&Value::Null));
                    println!(
                        "head     {}",
                        info.get("headNumber").unwrap_or(&Value::Null)
                    );
                    println!("peers    {}", info.get("peers").unwrap_or(&Value::Null));
                }
                Err(e) => println!("ivory_nodeInfo unavailable: {e}"),
            }
        }
    }
    Ok(())
}

fn rpc_call(url: &str, method: &str, params: Value) -> Result<Value> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    });
    let resp = ureq::post(url)
        .set("content-type", "application/json")
        .send_json(&body)
        .with_context(|| format!("POST {url} {method}"))?;
    let payload: Value = resp.into_json().context("rpc json")?;
    if let Some(err) = payload.get("error").filter(|e| !e.is_null()) {
        bail!("{method}: {err}");
    }
    payload.get("result").cloned().context("rpc result")
}
