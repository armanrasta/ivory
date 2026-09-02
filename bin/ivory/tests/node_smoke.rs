//! Node init and produce-via-RPC smoke test.

use std::time::Duration;

use ivory_crypto::signed_transfer;
use ivory_node::{init_datadir, load_datadir, run_node};
use ivory_primitives::U256;
use ivory_txpool::TxOrigin;
use serde_json::json;
use tokio::sync::watch;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn init_writes_files() {
    let dir = tempfile::tempdir().unwrap();
    let paths = init_datadir(dir.path()).unwrap();
    assert!(paths.config.exists());
    assert!(paths.genesis.exists());
    assert!(paths.validator_key.exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn run_produces_signed_transfer() {
    let dir = tempfile::tempdir().unwrap();
    init_datadir(dir.path()).unwrap();
    let (mut cfg, genesis, key, _) = load_datadir(dir.path()).unwrap();
    cfg.rpc_addr = "127.0.0.1:0".into();
    cfg.block_interval_ms = 80;
    cfg.p2p_listen = "/ip4/127.0.0.1/tcp/0".into();

    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let node = run_node(cfg, genesis, key.clone(), shutdown_rx)
        .await
        .unwrap();

    let to = ivory_crypto::keypair_from_byte(2).2;
    let tx = signed_transfer(&key, to, 0, U256::from(1u64), 21_000);
    node.pool.add_transaction(tx, TxOrigin::Local).unwrap();

    let mut ok = false;
    for _ in 0..80 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let Some(head) = node.store.head_block() else {
            continue;
        };
        for n in 1..=head.header.number {
            if node
                .store
                .get_block_by_number(n)
                .is_some_and(|b| !b.transactions.is_empty())
            {
                ok = true;
                break;
            }
        }
        if ok {
            break;
        }
    }
    assert!(ok, "expected a produced block containing the transfer");
    let n = node.handler.handle("eth_blockNumber", json!([])).unwrap();
    assert_ne!(n, json!("0x0"));
}
