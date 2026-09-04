//! Two-swarm gossip and sync tests.

use std::time::Duration;

use ivory_core::{Block, BlockHeader};
use ivory_crypto::{keypair_from_byte, signed_transfer};
use ivory_network::{NetworkConfig, NetworkEvent, NetworkHandle, NetworkMessage, start};
use ivory_primitives::{Address, Bytes, H256, U256};
use libp2p::Multiaddr;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::timeout;

fn empty_block(number: u64) -> Block {
    Block {
        header: BlockHeader {
            number,
            parent_hash: H256::ZERO,
            timestamp: number + 1,
            miner: Address::zero(),
            gas_limit: 1,
            gas_used: 0,
            state_root: H256::ZERO,
            transactions_root: H256::ZERO,
            receipts_root: H256::ZERO,
            difficulty: U256::ZERO,
            extra_data: Bytes::new(),
        },
        transactions: Vec::new(),
        receipts: Vec::new(),
    }
}

async fn wait_listen(rx: &mut UnboundedReceiver<NetworkEvent>) -> Multiaddr {
    timeout(Duration::from_secs(5), async {
        loop {
            match rx.recv().await {
                Some(NetworkEvent::ListenAddr(addr)) => return addr,
                Some(_) => continue,
                None => panic!("event channel closed before listen"),
            }
        }
    })
    .await
    .expect("listen addr")
}

async fn wait_connected(rx: &mut UnboundedReceiver<NetworkEvent>) {
    timeout(Duration::from_secs(10), async {
        loop {
            match rx.recv().await {
                Some(NetworkEvent::PeerConnected(_)) => return,
                Some(_) => continue,
                None => panic!("event channel closed before connect"),
            }
        }
    })
    .await
    .expect("peer connected");
}

async fn pair() -> (
    NetworkHandle,
    UnboundedReceiver<NetworkEvent>,
    NetworkHandle,
    UnboundedReceiver<NetworkEvent>,
) {
    let (a, mut ra) = start(NetworkConfig::default()).await.unwrap();
    let addr_a = wait_listen(&mut ra).await;
    let (b, mut rb) = start(NetworkConfig::default()).await.unwrap();
    let _addr_b = wait_listen(&mut rb).await;
    b.dial(addr_a).unwrap();
    wait_connected(&mut ra).await;
    wait_connected(&mut rb).await;
    (a, ra, b, rb)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn start_emits_listen_addr() {
    let (handle, mut rx) = start(NetworkConfig::default()).await.unwrap();
    let addr = wait_listen(&mut rx).await;
    assert!(addr.to_string().contains("/tcp/"));
    assert_ne!(handle.peer_id().to_string().len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_peers_connect() {
    let (a, _, b, _) = pair().await;
    assert_ne!(a.peer_id(), b.peer_id());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gossip_transaction() {
    let (a, _, _b, mut rb) = pair().await;
    let (sk, _, _) = keypair_from_byte(1);
    let to = keypair_from_byte(2).2;
    let tx = signed_transfer(&sk, to, 0, U256::from(1u64), 21_000);
    let hash = tx.hash();
    tokio::time::sleep(Duration::from_millis(400)).await;
    a.broadcast_transaction(tx).unwrap();
    let got = timeout(Duration::from_secs(8), async {
        loop {
            match rb.recv().await {
                Some(NetworkEvent::TxReceived(t)) => return t,
                Some(_) => continue,
                None => panic!("closed"),
            }
        }
    })
    .await
    .expect("tx gossip");
    assert_eq!(got.hash(), hash);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gossip_block() {
    let (a, _, _b, mut rb) = pair().await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    let block = empty_block(3);
    let hash = block.hash();
    a.broadcast_block(block).unwrap();
    let got = timeout(Duration::from_secs(8), async {
        loop {
            match rb.recv().await {
                Some(NetworkEvent::BlockReceived(b)) => return b,
                Some(_) => continue,
                None => panic!("closed"),
            }
        }
    })
    .await
    .expect("block gossip");
    assert_eq!(got.hash(), hash);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gossip_header_then_block() {
    let (a, _, _b, mut rb) = pair().await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    let block = empty_block(3);
    let hash = block.hash();
    a.broadcast_block(block).unwrap();
    let (got_header, got_block) = timeout(Duration::from_secs(8), async {
        let mut header = None;
        let mut body = None;
        loop {
            match rb.recv().await {
                Some(NetworkEvent::HeaderReceived(h)) if h.hash() == hash => header = Some(h),
                Some(NetworkEvent::BlockReceived(b)) if b.hash() == hash => body = Some(b),
                Some(_) => continue,
                None => panic!("closed"),
            }
            if let (Some(h), Some(b)) = (header.clone(), body.clone()) {
                return (h, b);
            }
        }
    })
    .await
    .expect("header then block");
    assert_eq!(got_header.hash(), hash);
    assert_eq!(got_block.hash(), hash);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sync_get_header_request() {
    let (a, _, _b, mut rb) = pair().await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    let hash = H256::from_bytes([43u8; 32]);
    a.request_header(hash).unwrap();
    let got = timeout(Duration::from_secs(8), async {
        loop {
            match rb.recv().await {
                Some(NetworkEvent::HeaderRequest(h)) => return h,
                Some(_) => continue,
                None => panic!("closed"),
            }
        }
    })
    .await
    .expect("get-header");
    assert_eq!(got, hash);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sync_get_block_request() {
    let (a, _, _b, mut rb) = pair().await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    let hash = H256::from_bytes([42u8; 32]);
    a.request_block(hash).unwrap();
    let got = timeout(Duration::from_secs(8), async {
        loop {
            match rb.recv().await {
                Some(NetworkEvent::BlockRequest(h)) => return h,
                Some(_) => continue,
                None => panic!("closed"),
            }
        }
    })
    .await
    .expect("get-block");
    assert_eq!(got, hash);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn peer_disconnect() {
    let (a, mut ra, b, _) = pair().await;
    let remote = b.peer_id();
    drop(b);
    timeout(Duration::from_secs(8), async {
        loop {
            match ra.recv().await {
                Some(NetworkEvent::PeerDisconnected(id)) if id == remote => return,
                Some(_) => continue,
                None => panic!("closed"),
            }
        }
    })
    .await
    .expect("disconnect");
    a.broadcast_block(empty_block(0)).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unique_peer_ids() {
    let (a, mut ra) = start(NetworkConfig::default()).await.unwrap();
    let (b, mut rb) = start(NetworkConfig::default()).await.unwrap();
    let _ = wait_listen(&mut ra).await;
    let _ = wait_listen(&mut rb).await;
    assert_ne!(a.peer_id(), b.peer_id());
}

#[tokio::test]
async fn codec_used_by_integration_path() {
    let msg = NetworkMessage::GetBlock(H256::ZERO);
    assert!(NetworkMessage::decode(&msg.encode().unwrap()).is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn broadcast_without_peers_does_not_fail() {
    let (handle, mut rx) = start(NetworkConfig::default()).await.unwrap();
    let _ = wait_listen(&mut rx).await;
    handle.broadcast_block(empty_block(1)).unwrap();
    handle
        .broadcast_transaction(signed_transfer(
            &keypair_from_byte(1).0,
            keypair_from_byte(2).2,
            0,
            U256::from(1u64),
            21_000,
        ))
        .unwrap();
    handle.request_block(H256::ZERO).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bootstrap_dials_listen_addr() {
    let (a, mut ra) = start(NetworkConfig::default()).await.unwrap();
    let addr_a = wait_listen(&mut ra).await;
    let (b, mut rb) = start(NetworkConfig {
        listen: "/ip4/127.0.0.1/tcp/0".parse().unwrap(),
        bootstrap: vec![addr_a],
        allowlist: Vec::new(),
    })
    .await
    .unwrap();
    let _ = wait_listen(&mut rb).await;
    wait_connected(&mut ra).await;
    wait_connected(&mut rb).await;
    assert_ne!(a.peer_id(), b.peer_id());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn allowlist_accepts_listed_peer_and_denies_third() {
    let (b, mut rb) = start(NetworkConfig::default()).await.unwrap();
    let _ = wait_listen(&mut rb).await;
    let (a, mut ra) = start(NetworkConfig {
        listen: "/ip4/127.0.0.1/tcp/0".parse().unwrap(),
        bootstrap: Vec::new(),
        allowlist: vec![b.peer_id()],
    })
    .await
    .unwrap();
    let addr_a = wait_listen(&mut ra).await;
    b.dial(addr_a.clone()).unwrap();
    timeout(Duration::from_secs(10), async {
        loop {
            if a.peer_count() == 1 && b.peer_count() == 1 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("listed peers connected");

    let (c, mut rc) = start(NetworkConfig::default()).await.unwrap();
    let _ = wait_listen(&mut rc).await;
    c.dial(addr_a).unwrap();
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert_eq!(a.peer_count(), 1);
    let _ = c;
}
