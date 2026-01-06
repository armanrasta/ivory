// crates/ivory-rpc/src/websocket.rs

//! WebSocket subscription handling

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, warn};

use crate::server::RpcState;
use crate::jsonrpc::{JsonRpcRequest, JsonRpcResponse, JsonRpcError};

/// Subscription types
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SubscriptionType {
    /// New block headers
    NewHeads,
    /// New pending transactions
    NewPendingTransactions,
    /// Logs matching filter
    Logs { filter: LogFilter },
    /// Syncing status
    Syncing,
}

/// Log filter for subscriptions
#[derive(Debug, Clone, Deserialize)]
pub struct LogFilter {
    pub address: Option<Vec<String>>,
    pub topics: Option<Vec<Option<Vec<String>>>>,
}

/// Handle WebSocket connection
pub async fn handle_socket(socket: WebSocket, state: Arc<RpcState>) {
    let (mut sender, mut receiver) = socket.split();
    
    debug!("New WebSocket connection");
    
    while let Some(msg) = receiver.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
        };
        
        match msg {
            Message::Text(text) => {
                let response = handle_ws_message(&state, &text).await;
                if let Err(e) = sender.send(Message::Text(response)).await {
                    error!("Failed to send response: {}", e);
                    break;
                }
            }
            Message::Binary(data) => {
                // Handle binary as text
                if let Ok(text) = String::from_utf8(data) {
                    let response = handle_ws_message(&state, &text).await;
                    if let Err(e) = sender.send(Message::Text(response)).await {
                        error!("Failed to send response: {}", e);
                        break;
                    }
                }
            }
            Message::Ping(data) => {
                if let Err(e) = sender.send(Message::Pong(data)).await {
                    error!("Failed to send pong: {}", e);
                    break;
                }
            }
            Message::Pong(_) => {}
            Message::Close(_) => {
                debug!("WebSocket connection closed");
                break;
            }
        }
    }
    
    debug!("WebSocket connection ended");
}

/// Handle WebSocket message
async fn handle_ws_message(state: &RpcState, text: &str) -> String {
    // Parse JSON-RPC request
    let request: JsonRpcRequest = match serde_json::from_str(text) {
        Ok(r) => r,
        Err(e) => {
            let response = JsonRpcResponse::error(
                serde_json::Value::Null,
                JsonRpcError::parse_error(),
            );
            return serde_json::to_string(&response).unwrap_or_default();
        }
    };
    
    // Handle subscription methods
    let response = match request.method.as_str() {
        "eth_subscribe" => handle_subscribe(&request).await,
        "eth_unsubscribe" => handle_unsubscribe(&request).await,
        _ => {
            // Regular RPC call
            match state.handler.handle(&request.method, request.params.clone()).await {
                Ok(result) => JsonRpcResponse::success(request.id, result),
                Err(e) => JsonRpcResponse::error(request.id, e.into()),
            }
        }
    };
    
    serde_json::to_string(&response).unwrap_or_default()
}

/// Handle subscription request
async fn handle_subscribe(request: &JsonRpcRequest) -> JsonRpcResponse {
    // TODO: Implement subscriptions
    // 1. Parse subscription type
    // 2. Register subscription
    // 3. Return subscription ID
    
    let sub_id = format!("0x{:x}", rand::random::<u64>());
    JsonRpcResponse::success(request.id.clone(), serde_json::json!(sub_id))
}

/// Handle unsubscribe request
async fn handle_unsubscribe(request: &JsonRpcRequest) -> JsonRpcResponse {
    // TODO: Implement unsubscribe
    // 1. Parse subscription ID
    // 2. Remove subscription
    // 3. Return success
    
    JsonRpcResponse::success(request.id.clone(), serde_json::json!(true))
}