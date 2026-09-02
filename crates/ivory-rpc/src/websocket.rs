//! WebSocket JSON-RPC (same methods as HTTP; subscriptions are stubs).

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use tracing::{debug, error};

use crate::jsonrpc::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use crate::server::{RpcState, dispatch};

/// Handle a WebSocket connection.
pub async fn handle_socket(socket: WebSocket, state: Arc<RpcState>) {
    let (mut sender, mut receiver) = socket.split();
    debug!("New WebSocket connection");

    while let Some(msg) = receiver.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                error!("WebSocket error: {e}");
                break;
            }
        };

        match msg {
            Message::Text(text) => {
                let response = handle_ws_message(&state, &text);
                if sender.send(Message::Text(response)).await.is_err() {
                    break;
                }
            }
            Message::Binary(data) => {
                if let Ok(text) = String::from_utf8(data) {
                    let response = handle_ws_message(&state, &text);
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
            Message::Close(_) => {
                break;
            }
        }
    }
}

fn handle_ws_message(state: &RpcState, text: &str) -> String {
    let request: JsonRpcRequest = match serde_json::from_str(text) {
        Ok(r) => r,
        Err(_) => {
            return serde_json::to_string(&JsonRpcResponse::error(
                serde_json::Value::Null,
                JsonRpcError::parse_error(),
            ))
            .unwrap_or_default();
        }
    };

    let response = match request.method.as_str() {
        "eth_subscribe" => JsonRpcResponse::success(request.id.clone(), serde_json::json!("0x1")),
        "eth_unsubscribe" => JsonRpcResponse::success(request.id.clone(), serde_json::json!(true)),
        _ => dispatch(state, request),
    };
    serde_json::to_string(&response).unwrap_or_default()
}
