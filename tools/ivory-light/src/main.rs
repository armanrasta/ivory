//! ivory-light CLI: walk headers from a full-node RPC.

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use ivory_core::BlockHeader;
use ivory_light::{header_from_rpc, verify_header_link, verify_rpc_hash};
use serde_json::{Value, json};

#[derive(Parser)]
#[command(name = "ivory-light", about = "Follow Ivory headers without bodies")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Walk `ivory_getHeaderByNumber` from `--from` through `--to`.
    Follow {
        /// JSON-RPC URL.
        #[arg(long, default_value = "http://127.0.0.1:8545")]
        rpc: String,
        /// First height (decimal or 0x-hex).
        #[arg(long, default_value = "0")]
        from: String,
        /// Last height, or `latest`.
        #[arg(long, default_value = "latest")]
        to: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Follow { rpc, from, to } => {
            let rpc = rpc.trim_end_matches('/').to_string();
            let start = parse_height(&from)?;
            let end = if to.eq_ignore_ascii_case("latest") {
                let n = rpc_call(&rpc, "eth_blockNumber", json!([]))?;
                parse_rpc_qty(&n)?
            } else {
                parse_height(&to)?
            };
            if end < start {
                bail!("--to {end} is below --from {start}");
            }
            let mut prev: Option<BlockHeader> = None;
            for n in start..=end {
                let raw = rpc_call(&rpc, "ivory_getHeaderByNumber", json!([encode_qty(n)]))?;
                let header = header_from_rpc(&raw).context("decode header")?;
                verify_rpc_hash(&header, &raw).context("header hash")?;
                if n == 0 && header.extra_data.is_empty() {
                    bail!("genesis extra_data seal is empty");
                }
                if let Some(p) = &prev {
                    verify_header_link(p, &header).with_context(|| format!("link at {n}"))?;
                } else if header.number != start {
                    bail!("first header number {} != --from {start}", header.number);
                }
                println!("{} {}", encode_qty(header.number), header.hash().to_hex());
                prev = Some(header);
            }
        }
    }
    Ok(())
}

fn parse_height(s: &str) -> Result<u64> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).context("height hex")
    } else {
        s.parse::<u64>().context("height decimal")
    }
}

fn parse_rpc_qty(v: &Value) -> Result<u64> {
    let s = v.as_str().context("qty string")?;
    let stripped = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    u64::from_str_radix(stripped, 16).context("qty hex")
}

fn encode_qty(n: u64) -> String {
    if n == 0 {
        "0x0".into()
    } else {
        format!("0x{n:x}")
    }
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
