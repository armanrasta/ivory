//! HTTP JSON-RPC integration tests.

use std::net::SocketAddr;
use std::sync::Arc;

use ivory_chain::BlockStore;
use ivory_consensus::{ConsensusEngine, PoAConsensus};
use ivory_core::{Block, BlockHeader};
use ivory_crypto::keypair_from_byte;
use ivory_primitives::{Bytes, H256, U256};
use ivory_rpc::{JsonRpcRequest, JsonRpcResponse, RpcContext, RpcHandler, router};
use ivory_state::StateDB;
use ivory_txpool::TransactionPool;
use serde_json::json;
use tokio::net::TcpListener;

fn genesis() -> Block {
    let sk = keypair_from_byte(9).0;
    let miner = keypair_from_byte(9).2;
    let poa = PoAConsensus::from_secret(&sk).unwrap();
    let mut header = BlockHeader {
        number: 0,
        parent_hash: H256::ZERO,
        timestamp: 1,
        miner,
        gas_limit: 1,
        gas_used: 0,
        state_root: H256::ZERO,
        transactions_root: H256::ZERO,
        receipts_root: H256::ZERO,
        difficulty: U256::ZERO,
        extra_data: Bytes::new(),
    };
    poa.seal_header(&mut header, &miner, &sk).unwrap();
    Block {
        header,
        transactions: Vec::new(),
        receipts: Vec::new(),
    }
}

async fn spawn_server() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let store = Arc::new(BlockStore::new(
        PoAConsensus::from_secret(&keypair_from_byte(9).0).unwrap(),
    ));
    store.insert_genesis(genesis()).unwrap();
    let handler = RpcHandler::new(RpcContext::new(
        store,
        Arc::new(TransactionPool::new()),
        StateDB::new(),
        1,
    ));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = router(handler);
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (addr, handle)
}

async fn post_rpc(addr: SocketAddr, body: JsonRpcRequest) -> JsonRpcResponse {
    let payload = serde_json::to_vec(&body).unwrap();
    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (mut reader, mut writer) = stream.into_split();
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let req = format!(
        "POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        payload.len()
    );
    writer.write_all(req.as_bytes()).await.unwrap();
    writer.write_all(&payload).await.unwrap();
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).await.unwrap();
    let text = String::from_utf8_lossy(&buf);
    let json_start = text.find('{').expect("json body");
    serde_json::from_str(&text[json_start..]).unwrap()
}

#[tokio::test]
async fn http_chain_id() {
    let (addr, _h) = spawn_server().await;
    let resp = post_rpc(
        addr,
        JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "eth_chainId".into(),
            params: json!([]),
            id: json!(1),
        },
    )
    .await;
    assert_eq!(resp.result, Some(json!("0x1")));
    assert!(resp.error.is_none());
}

#[tokio::test]
async fn http_malformed_body() {
    let (addr, _h) = spawn_server().await;
    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (mut reader, mut writer) = stream.into_split();
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let payload = b"{not-json";
    let req = format!(
        "POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        payload.len()
    );
    writer.write_all(req.as_bytes()).await.unwrap();
    writer.write_all(payload).await.unwrap();
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).await.unwrap();
    let text = String::from_utf8_lossy(&buf);
    assert!(text.contains("-32700") || text.contains("Parse error"));
}

#[tokio::test]
async fn http_method_not_found() {
    let (addr, _h) = spawn_server().await;
    let resp = post_rpc(
        addr,
        JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "eth_call".into(),
            params: json!([]),
            id: json!(2),
        },
    )
    .await;
    assert_eq!(resp.error.as_ref().unwrap().code, -32601);
}
