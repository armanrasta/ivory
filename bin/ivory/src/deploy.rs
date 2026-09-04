//! Deploy a YAML/WAT/WASM contract via JSON-RPC CREATE.

use std::path::Path;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use ivory_crypto::{address_from_secret, signed_tx};
use ivory_primitives::{Bytes, SecretKey, U256};
use serde_json::{Value, json};

use crate::contract::{install_contract_files, load_contract_file};

/// Compile `manifest`, copy it into `data_contracts`, and submit CREATE.
///
/// # Errors
///
/// Compile, RPC, or inclusion failures.
pub fn deploy_contract(
    manifest: &Path,
    rpc_url: &str,
    key: &SecretKey,
    data_contracts: &Path,
) -> Result<String> {
    let compiled = load_contract_file(manifest)?;
    install_contract_files(manifest, data_contracts)?;

    let sender = address_from_secret(key);
    let nonce = hex_u64(&rpc_call(
        rpc_url,
        "eth_getTransactionCount",
        json!([sender.to_hex(), "latest"]),
    )?)?;
    let gas = 21_000u64
        .saturating_add(16 * compiled.wasm.len() as u64)
        .saturating_add(50_000);
    let tx = signed_tx(
        key,
        None,
        nonce,
        U256::ZERO,
        gas,
        U256::ONE,
        Bytes::from_vec(compiled.wasm.clone()),
    );
    let hash = tx.hash();
    let raw = format!("0x{}", hex::encode(bincode::serialize(&tx)?));
    let submitted = rpc_call(rpc_url, "eth_sendRawTransaction", json!([raw]))?;
    let submitted = submitted
        .as_str()
        .context("eth_sendRawTransaction result")?
        .to_string();

    let addr = wait_create(rpc_url, &hash.to_hex())?;
    println!("deployed {}", compiled.name);
    println!("  source:   {}", compiled.source);
    println!("  schema:   {}", compiled.schema);
    println!("  tx:       {submitted}");
    println!("  address:  {addr}");
    Ok(addr)
}

fn wait_create(rpc_url: &str, tx_hash: &str) -> Result<String> {
    for _ in 0..40 {
        match rpc_call(rpc_url, "eth_getTransactionReceipt", json!([tx_hash])) {
            Ok(Value::Null) | Err(_) => {}
            Ok(rec) => {
                if let Some(addr) = rec.get("contractAddress").and_then(Value::as_str) {
                    return Ok(addr.to_string());
                }
                bail!("receipt for {tx_hash} has no contractAddress");
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
    bail!("timed out waiting for {tx_hash} (is the node producing?)");
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

fn hex_u64(v: &Value) -> Result<u64> {
    let s = v.as_str().context("expected hex qty")?;
    let s = s.trim_start_matches("0x").trim_start_matches("0X");
    if s.is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(s, 16).context("hex qty")
}
