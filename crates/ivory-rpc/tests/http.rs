//! HTTP JSON-RPC integration tests.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use ivory_chain::BlockStore;
use ivory_consensus::{ConsensusEngine, PoAConsensus};
use ivory_core::{Block, BlockHeader, empty_list_roots};
use ivory_crypto::keypair_from_byte;
use ivory_primitives::{Bytes, H256, U256};
use ivory_rpc::{
    JsonRpcRequest, JsonRpcResponse, RpcContext, RpcEvent, RpcHandler, RpcHttpConfig, router,
    router_with_config,
};
use ivory_state::StateDB;
use ivory_txpool::TransactionPool;
use serde_json::json;
use tokio::net::TcpListener;

fn genesis() -> Block {
    let sk = keypair_from_byte(9).0;
    let miner = keypair_from_byte(9).2;
    let poa = PoAConsensus::from_secret(&sk).unwrap();
    let (tx_root, rx_root) = empty_list_roots();
    let mut header = BlockHeader {
        number: 0,
        parent_hash: H256::ZERO,
        timestamp: 1,
        miner,
        gas_limit: 1,
        gas_used: 0,
        state_root: H256::ZERO,
        transactions_root: tx_root,
        receipts_root: rx_root,
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
            method: "eth_foo".into(),
            params: json!([]),
            id: json!(2),
        },
    )
    .await;
    assert_eq!(resp.error.as_ref().unwrap().code, -32601);
}

#[tokio::test]
async fn http_serves_panel() {
    let (addr, _h) = spawn_server().await;
    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (mut reader, mut writer) = stream.into_split();
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    writer
        .write_all(b"GET /ui HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).await.unwrap();
    let text = String::from_utf8_lossy(&buf);
    assert!(text.contains("200"));
    assert!(text.contains("text/html"));
    assert!(text.contains("ivory-ui-theme"));
    assert!(text.contains("ivory_nodeInfo"));
}

async fn http_get(addr: SocketAddr, path: &str, auth: Option<&str>) -> (u16, String) {
    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (mut reader, mut writer) = stream.into_split();
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let auth_line = auth
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    let req =
        format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n{auth_line}Connection: close\r\n\r\n");
    writer.write_all(req.as_bytes()).await.unwrap();
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).await.unwrap();
    let text = String::from_utf8_lossy(&buf);
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    (status, text.into_owned())
}

#[tokio::test]
async fn http_livez_readyz() {
    let (addr, _h) = spawn_server().await;
    let (status, body) = http_get(addr, "/livez", None).await;
    assert_eq!(status, 200);
    assert!(body.contains("ok"));
    let (status, body) = http_get(addr, "/readyz", None).await;
    assert_eq!(status, 200);
    assert!(body.contains("ready"));
}

#[tokio::test]
async fn http_metrics_has_head_after_genesis() {
    let (addr, _h) = spawn_server().await;
    let (status, body) = http_get(addr, "/metrics", None).await;
    assert_eq!(status, 200);
    assert!(body.contains("ivory_head_number"), "{body}");
}

async fn spawn_token_server(token: &str) -> (SocketAddr, tokio::task::JoinHandle<()>) {
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
    let app = router_with_config(
        handler,
        RpcHttpConfig {
            token: Some(token.into()),
            cors: String::new(),
        },
    );
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (addr, handle)
}

#[tokio::test]
async fn rpc_token_rejects_without_bearer() {
    let (addr, _h) = spawn_token_server("s3cret").await;
    let (status, _) = http_get(addr, "/ui", None).await;
    assert_eq!(status, 401);
    let (status, _) = http_get(addr, "/livez", None).await;
    assert_eq!(status, 200);
}

#[tokio::test]
async fn rpc_token_accepts_bearer() {
    let (addr, _h) = spawn_token_server("s3cret").await;
    let (status, body) = http_get(addr, "/ui", Some("s3cret")).await;
    assert_eq!(status, 200);
    assert!(body.contains("text/html"));
}

async fn post_status(addr: SocketAddr, auth: Option<&str>) -> u16 {
    let payload = serde_json::to_vec(&JsonRpcRequest {
        jsonrpc: "2.0".into(),
        method: "eth_chainId".into(),
        params: json!([]),
        id: json!(1),
    })
    .unwrap();
    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (mut reader, mut writer) = stream.into_split();
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let auth_line = auth
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    let req = format!(
        "POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\n{auth_line}Content-Length: {}\r\nConnection: close\r\n\r\n",
        payload.len()
    );
    writer.write_all(req.as_bytes()).await.unwrap();
    writer.write_all(&payload).await.unwrap();
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).await.unwrap();
    let text = String::from_utf8_lossy(&buf);
    text.split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

#[tokio::test]
async fn rpc_token_rejects_post_without_bearer() {
    let (addr, _h) = spawn_token_server("s3cret").await;
    assert_eq!(post_status(addr, None).await, 401);
}

#[tokio::test]
async fn rpc_token_accepts_post_with_bearer() {
    let (addr, _h) = spawn_token_server("s3cret").await;
    assert_eq!(post_status(addr, Some("s3cret")).await, 200);
}

async fn spawn_with_handler(handler: RpcHandler) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = router(handler);
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (addr, handle)
}

#[tokio::test]
async fn http_subscribe_is_rejected() {
    let (addr, _h) = spawn_server().await;
    let resp = post_rpc(
        addr,
        JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "eth_subscribe".into(),
            params: json!(["newHeads"]),
            id: json!(1),
        },
    )
    .await;
    assert!(resp.error.is_some());
    assert!(
        resp.error
            .as_ref()
            .unwrap()
            .message
            .to_lowercase()
            .contains("websocket")
    );
}

#[tokio::test]
async fn ws_subscribe_new_heads_and_unsubscribe() {
    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;

    let store = Arc::new(BlockStore::new(
        PoAConsensus::from_secret(&keypair_from_byte(9).0).unwrap(),
    ));
    let genesis = genesis();
    store.insert_genesis(genesis.clone()).unwrap();
    let ctx = RpcContext::new(store, Arc::new(TransactionPool::new()), StateDB::new(), 1);
    let events = ctx.events.clone();
    let (addr, _h) = spawn_with_handler(RpcHandler::new(ctx)).await;
    let url = format!("ws://{addr}/");
    let (mut ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();

    ws.send(Message::Text(
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_subscribe",
            "params": ["newHeads"]
        })
        .to_string(),
    ))
    .await
    .unwrap();
    let reply = ws.next().await.unwrap().unwrap();
    let text = reply.into_text().unwrap();
    let sub: serde_json::Value = serde_json::from_str(&text).unwrap();
    let sub_id = sub["result"].as_str().unwrap().to_string();
    assert!(sub_id.starts_with("0x"));

    events.send(RpcEvent::new_head(&genesis)).unwrap();
    let note = tokio::time::timeout(Duration::from_secs(2), ws.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let note: serde_json::Value = serde_json::from_str(&note.into_text().unwrap()).unwrap();
    assert_eq!(note["method"], "eth_subscription");
    assert_eq!(note["params"]["subscription"], sub_id);
    assert_eq!(note["params"]["result"]["hash"], genesis.hash().to_hex());

    ws.send(Message::Text(
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "eth_unsubscribe",
            "params": [sub_id]
        })
        .to_string(),
    ))
    .await
    .unwrap();
    let unsub = ws.next().await.unwrap().unwrap();
    let unsub: serde_json::Value = serde_json::from_str(&unsub.into_text().unwrap()).unwrap();
    assert_eq!(unsub["result"], true);

    events.send(RpcEvent::new_head(&genesis)).unwrap();
    let extra = tokio::time::timeout(Duration::from_millis(200), ws.next()).await;
    assert!(extra.is_err(), "unsubscribed socket must not be notified");
}

#[tokio::test]
async fn ws_subscribe_pending_tx() {
    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;

    let ctx = RpcContext::new(
        Arc::new(BlockStore::new(
            PoAConsensus::from_secret(&keypair_from_byte(9).0).unwrap(),
        )),
        Arc::new(TransactionPool::new()),
        StateDB::new(),
        1,
    );
    let events = ctx.events.clone();
    let (addr, _h) = spawn_with_handler(RpcHandler::new(ctx)).await;
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/"))
        .await
        .unwrap();
    ws.send(Message::Text(
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_subscribe",
            "params": ["newPendingTransactions"]
        })
        .to_string(),
    ))
    .await
    .unwrap();
    let _ = ws.next().await;
    let hash = H256::from_bytes([0xab; 32]);
    events.send(RpcEvent::NewPendingTx { hash }).unwrap();
    let note = tokio::time::timeout(Duration::from_secs(2), ws.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let note: serde_json::Value = serde_json::from_str(&note.into_text().unwrap()).unwrap();
    assert_eq!(note["params"]["result"], hash.to_hex());
}

#[tokio::test]
async fn ws_subscribe_logs_filters_address() {
    use futures::{SinkExt, StreamExt};
    use ivory_core::Log;
    use tokio_tungstenite::tungstenite::Message;

    let addr = keypair_from_byte(3).2;
    let other = keypair_from_byte(4).2;
    let ctx = RpcContext::new(
        Arc::new(BlockStore::new(
            PoAConsensus::from_secret(&keypair_from_byte(9).0).unwrap(),
        )),
        Arc::new(TransactionPool::new()),
        StateDB::new(),
        1,
    );
    let events = ctx.events.clone();
    let (bind, _h) = spawn_with_handler(RpcHandler::new(ctx)).await;
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{bind}/"))
        .await
        .unwrap();
    ws.send(Message::Text(
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_subscribe",
            "params": ["logs", {"address": addr.to_hex()}]
        })
        .to_string(),
    ))
    .await
    .unwrap();
    let _ = ws.next().await;
    events
        .send(RpcEvent::NewLogs {
            logs: vec![
                Log {
                    address: other,
                    topics: Vec::new(),
                    data: Bytes::new(),
                },
                Log {
                    address: addr,
                    topics: Vec::new(),
                    data: Bytes::from_vec(vec![0x2a]),
                },
            ],
            block_number: 1,
            block_hash: H256::from_bytes([0x11; 32]),
        })
        .unwrap();
    let note = tokio::time::timeout(Duration::from_secs(2), ws.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let note: serde_json::Value = serde_json::from_str(&note.into_text().unwrap()).unwrap();
    let logs = note["params"]["result"].as_array().unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0]["address"], addr.to_hex());
}

#[tokio::test]
async fn ws_unknown_subscribe_topic_is_invalid_params() {
    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;

    let ctx = RpcContext::new(
        Arc::new(BlockStore::new(
            PoAConsensus::from_secret(&keypair_from_byte(9).0).unwrap(),
        )),
        Arc::new(TransactionPool::new()),
        StateDB::new(),
        1,
    );
    let (addr, _h) = spawn_with_handler(RpcHandler::new(ctx)).await;
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/"))
        .await
        .unwrap();
    ws.send(Message::Text(
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_subscribe",
            "params": ["syncing"]
        })
        .to_string(),
    ))
    .await
    .unwrap();
    let reply = ws.next().await.unwrap().unwrap();
    let body: serde_json::Value = serde_json::from_str(&reply.into_text().unwrap()).unwrap();
    assert_eq!(body["error"]["code"], -32602);
}
