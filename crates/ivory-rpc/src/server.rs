//! Axum HTTP JSON-RPC server.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::State;
use axum::extract::ws::WebSocketUpgrade;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::Value;
use tower_http::cors::CorsLayer;

use crate::handler::RpcHandler;
use crate::jsonrpc::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use crate::websocket;

/// Shared server state.
pub struct RpcState {
    /// Method dispatcher.
    pub handler: RpcHandler,
}

/// Build the HTTP / WS router.
pub fn router(handler: RpcHandler) -> Router {
    let state = Arc::new(RpcState { handler });
    Router::new()
        .route("/", post(http_jsonrpc).get(ws_upgrade))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Bind and serve JSON-RPC.
///
/// # Errors
///
/// Returns IO errors from bind / serve.
pub async fn serve(handler: RpcHandler, addr: SocketAddr) -> Result<(), std::io::Error> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(handler)).await
}

async fn http_jsonrpc(
    State(state): State<Arc<RpcState>>,
    body: Result<Json<Value>, axum::extract::rejection::JsonRejection>,
) -> impl IntoResponse {
    let Json(value) = match body {
        Ok(j) => j,
        Err(_) => {
            return (
                StatusCode::OK,
                Json(JsonRpcResponse::error(
                    Value::Null,
                    JsonRpcError::parse_error(),
                )),
            );
        }
    };
    let request: JsonRpcRequest = match serde_json::from_value(value) {
        Ok(r) => r,
        Err(_) => {
            return (
                StatusCode::OK,
                Json(JsonRpcResponse::error(
                    Value::Null,
                    JsonRpcError::invalid_request(),
                )),
            );
        }
    };
    let response = dispatch(&state, request);
    (StatusCode::OK, Json(response))
}

pub(crate) fn dispatch(state: &RpcState, request: JsonRpcRequest) -> JsonRpcResponse {
    match state.handler.handle(&request.method, request.params) {
        Ok(result) => JsonRpcResponse::success(request.id, result),
        Err(e) => JsonRpcResponse::error(request.id, e.into()),
    }
}

async fn ws_upgrade(ws: WebSocketUpgrade, State(state): State<Arc<RpcState>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| websocket::handle_socket(socket, state))
}
