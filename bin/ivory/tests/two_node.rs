//! Two-node process smoke: producer A, follower B, shared genesis.

use std::time::Duration;

use ivory_crypto::signed_transfer;
use ivory_node::{init_datadir, load_datadir, run_node};
use ivory_primitives::U256;
use serde_json::json;
use tokio::sync::watch;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn follower_imports_transfer_and_matches_balance() {
    let dir_a = tempfile::tempdir().unwrap();
    let dir_b = tempfile::tempdir().unwrap();
    init_datadir(dir_a.path()).unwrap();
    init_datadir(dir_b.path()).unwrap();
    std::fs::copy(
        dir_a.path().join("genesis.json"),
        dir_b.path().join("genesis.json"),
    )
    .unwrap();

    let (mut cfg_a, genesis_a, key_a, paths_a) = load_datadir(dir_a.path()).unwrap();
    cfg_a.rpc_addr = "127.0.0.1:0".into();
    cfg_a.p2p_listen = "/ip4/127.0.0.1/tcp/0".into();
    cfg_a.block_interval_ms = 80;

    let (_sh_a, rx_a) = watch::channel(false);
    let node_a = run_node(cfg_a, genesis_a, key_a.clone(), rx_a, &paths_a)
        .await
        .unwrap();

    let (mut cfg_b, genesis_b, key_b, paths_b) = load_datadir(dir_b.path()).unwrap();
    cfg_b.rpc_addr = "127.0.0.1:0".into();
    cfg_b.p2p_listen = "/ip4/127.0.0.1/tcp/0".into();
    cfg_b.block_interval_ms = 5_000;
    cfg_b.bootstrap = vec![node_a.p2p_addr.to_string()];

    let (_sh_b, rx_b) = watch::channel(false);
    let node_b = run_node(cfg_b, genesis_b, key_b, rx_b, &paths_b)
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(600)).await;

    let to = ivory_crypto::keypair_from_byte(2).2;
    let tx = signed_transfer(&key_a, to, 0, U256::from(1u64), 21_000);
    let raw = format!("0x{}", hex::encode(bincode::serialize(&tx).unwrap()));
    node_a
        .handler
        .handle("eth_sendRawTransaction", json!([raw]))
        .unwrap();

    let mut ok = false;
    for _ in 0..80 {
        tokio::time::sleep(Duration::from_millis(80)).await;
        let Some(head_b) = node_b.store.head_block() else {
            continue;
        };
        if head_b.header.number == 0 {
            continue;
        }
        let bal = node_b
            .handler
            .handle("eth_getBalance", json!([to.to_hex(), "latest"]))
            .unwrap();
        if bal == json!(U256::from(1u64).to_hex()) {
            ok = true;
            break;
        }
    }
    assert!(
        ok,
        "follower should import the produced transfer and credit the recipient"
    );
    let head_a = node_a.store.head_block().unwrap();
    let head_b = node_b.store.head_block().unwrap();
    assert_eq!(head_a.hash(), head_b.hash());
}
