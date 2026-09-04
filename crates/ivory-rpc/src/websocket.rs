//! WebSocket JSON-RPC (same methods as HTTP) plus `eth_subscribe`.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tracing::{debug, error};

use hex;
use ivory_primitives::Address;

use crate::context::RpcEvent;
use crate::jsonrpc::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use crate::server::{RpcState, dispatch};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SubKind {
    NewHeads,
    NewPendingTx,
    Logs { address: Option<Address> },
}

/// Handle a WebSocket connection.
pub async fn handle_socket(socket: WebSocket, state: Arc<RpcState>) {
    let (mut sender, mut receiver) = socket.split();
    let mut events = state.handler.context().subscribe_events();
    let mut subs: HashMap<String, SubKind> = HashMap::new();
    let mut next_id: u64 = 1;
    debug!("New WebSocket connection");

    loop {
        tokio::select! {
            msg = receiver.next() => {
                let Some(msg) = msg else {
                    break;
                };
                let msg = match msg {
                    Ok(m) => m,
                    Err(e) => {
                        error!("WebSocket error: {e}");
                        break;
                    }
                };
                match msg {
                    Message::Text(text) => {
                        let response = handle_ws_message(&state, &text, &mut subs, &mut next_id);
                        if sender.send(Message::Text(response)).await.is_err() {
                            break;
                        }
                    }
                    Message::Binary(data) => {
                        if let Ok(text) = String::from_utf8(data) {
                            let response = handle_ws_message(&state, &text, &mut subs, &mut next_id);
                            if sender.send(Message::Text(response)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Message::Ping(data) => {
                        if sender.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Message::Pong(_) => {}
                    Message::Close(_) => break,
                }
            }
            ev = events.recv() => {
                match ev {
                    Ok(event) => {
                        let notes = notifications_for(&subs, &event);
                        let mut failed = false;
                        for note in notes {
                            if sender.send(Message::Text(note)).await.is_err() {
                                failed = true;
                                break;
                            }
                        }
                        if failed {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

fn handle_ws_message(
    state: &RpcState,
    text: &str,
    subs: &mut HashMap<String, SubKind>,
    next_id: &mut u64,
) -> String {
    let request: JsonRpcRequest = match serde_json::from_str(text) {
        Ok(r) => r,
        Err(_) => {
            return serde_json::to_string(&JsonRpcResponse::error(
                Value::Null,
                JsonRpcError::parse_error(),
            ))
            .unwrap_or_default();
        }
    };

    let response = match request.method.as_str() {
        "eth_subscribe" => subscribe(request.id, &request.params, subs, next_id),
        "eth_unsubscribe" => unsubscribe(request.id, &request.params, subs),
        _ => dispatch(state, request),
    };
    serde_json::to_string(&response).unwrap_or_default()
}

fn subscribe(
    id: Value,
    params: &Value,
    subs: &mut HashMap<String, SubKind>,
    next_id: &mut u64,
) -> JsonRpcResponse {
    let Some(arr) = params.as_array() else {
        return JsonRpcResponse::error(id, JsonRpcError::invalid_params("expected array"));
    };
    let Some(topic) = arr.first().and_then(Value::as_str) else {
        return JsonRpcResponse::error(id, JsonRpcError::invalid_params("missing topic"));
    };
    let kind = match topic {
        "newHeads" => SubKind::NewHeads,
        "newPendingTransactions" => SubKind::NewPendingTx,
        "logs" => {
            let address = arr.get(1).and_then(|v| {
                v.get("address")
                    .and_then(Value::as_str)
                    .and_then(|s| Address::from_hex(s).ok())
            });
            SubKind::Logs { address }
        }
        other => {
            return JsonRpcResponse::error(
                id,
                JsonRpcError::invalid_params(&format!("unknown subscription {other}")),
            );
        }
    };
    let sub_id = format!("0x{next_id:x}");
    *next_id = next_id.saturating_add(1);
    subs.insert(sub_id.clone(), kind);
    JsonRpcResponse::success(id, Value::String(sub_id))
}

fn unsubscribe(id: Value, params: &Value, subs: &mut HashMap<String, SubKind>) -> JsonRpcResponse {
    let Some(arr) = params.as_array() else {
        return JsonRpcResponse::error(id, JsonRpcError::invalid_params("expected array"));
    };
    let Some(sub_id) = arr.first().and_then(Value::as_str) else {
        return JsonRpcResponse::error(id, JsonRpcError::invalid_params("missing subscription id"));
    };
    let removed = subs.remove(sub_id).is_some();
    JsonRpcResponse::success(id, Value::Bool(removed))
}

fn notifications_for(subs: &HashMap<String, SubKind>, event: &RpcEvent) -> Vec<String> {
    let mut out = Vec::new();
    for (sub_id, kind) in subs {
        let result = match (kind, event) {
            (SubKind::NewHeads, RpcEvent::NewHead { .. }) => Some(head_json(event)),
            (SubKind::NewPendingTx, RpcEvent::NewPendingTx { hash }) => {
                Some(Value::String(hash.to_hex()))
            }
            (
                SubKind::Logs { address },
                RpcEvent::NewLogs {
                    logs,
                    block_number,
                    block_hash,
                },
            ) => Some(logs_json(logs, *address, *block_number, *block_hash)),
            _ => None,
        };
        if let Some(result) = result {
            let note = json!({
                "jsonrpc": "2.0",
                "method": "eth_subscription",
                "params": {
                    "subscription": sub_id,
                    "result": result,
                }
            });
            if let Ok(s) = serde_json::to_string(&note) {
                out.push(s);
            }
        }
    }
    out
}

fn head_json(event: &RpcEvent) -> Value {
    let RpcEvent::NewHead {
        number,
        hash,
        parent_hash,
        miner,
        timestamp,
        state_root,
        gas_limit,
        gas_used,
    } = event
    else {
        return Value::Null;
    };
    json!({
        "number": encode_qty(*number),
        "hash": hash.to_hex(),
        "parentHash": parent_hash.to_hex(),
        "miner": miner.to_hex(),
        "timestamp": encode_qty(*timestamp),
        "stateRoot": state_root.to_hex(),
        "gasLimit": encode_qty(*gas_limit),
        "gasUsed": encode_qty(*gas_used),
    })
}

fn logs_json(
    logs: &[ivory_core::Log],
    filter: Option<Address>,
    block_number: u64,
    block_hash: ivory_primitives::H256,
) -> Value {
    let items: Vec<Value> = logs
        .iter()
        .filter(|l| filter.is_none_or(|a| l.address == a))
        .map(|l| {
            json!({
                "address": l.address.to_hex(),
                "topics": l.topics.iter().map(|t| t.to_hex()).collect::<Vec<_>>(),
                "data": format!("0x{}", hex::encode(l.data.as_slice())),
                "blockNumber": encode_qty(block_number),
                "blockHash": block_hash.to_hex(),
            })
        })
        .collect();
    Value::Array(items)
}

fn encode_qty(n: u64) -> String {
    if n == 0 {
        "0x0".into()
    } else {
        format!("0x{n:x}")
    }
}
