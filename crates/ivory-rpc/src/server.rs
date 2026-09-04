//! Axum HTTP JSON-RPC server.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::Request;
use axum::extract::State;
use axum::extract::ws::WebSocketUpgrade;
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::handler::RpcHandler;
use crate::jsonrpc::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use crate::websocket;

/// Read-only explorer (`GET /ui`).
const PANEL_HTML: &str = include_str!("../../../web/panel.html");

/// HTTP extras: optional bearer token and CORS allowlist.
#[derive(Clone, Debug, Default)]
pub struct RpcHttpConfig {
    /// If set, `POST /`, WebSocket, and `/ui` require `Authorization: Bearer`.
    pub token: Option<String>,
    /// `*` = permissive; empty = no CORS layer; otherwise comma-separated origins.
    pub cors: String,
}

impl RpcHttpConfig {
    /// Read `IVORY_RPC_TOKEN` / `IVORY_RPC_TOKEN_FILE` and `IVORY_CORS`.
    #[must_use]
    pub fn from_env() -> Self {
        let token = read_token();
        let cors = std::env::var("IVORY_CORS").unwrap_or_default();
        Self { token, cors }
    }
}

fn read_token() -> Option<String> {
    if let Ok(path) = std::env::var("IVORY_RPC_TOKEN_FILE") {
        let path = path.trim();
        if let Some(t) = file_token(path) {
            return Some(t);
        }
    }
    std::env::var("IVORY_RPC_TOKEN")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn file_token(path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    let t = std::fs::read_to_string(path).ok()?.trim().to_string();
    (!t.is_empty()).then_some(t)
}

/// Shared server state.
pub struct RpcState {
    /// Method dispatcher.
    pub handler: RpcHandler,
    token: Option<String>,
}

/// Build the HTTP / WS router (no token, no CORS).
pub fn router(handler: RpcHandler) -> Router {
    router_with_config(handler, RpcHttpConfig::default())
}

/// Build the HTTP / WS router with token and CORS settings.
pub fn router_with_config(handler: RpcHandler, cfg: RpcHttpConfig) -> Router {
    let state = Arc::new(RpcState {
        handler,
        token: cfg.token.clone(),
    });
    let mut app = Router::new()
        .route("/", post(http_jsonrpc).get(ws_upgrade))
        .route("/ui", get(panel_ui))
        .route("/ui/", get(panel_ui))
        .route("/livez", get(livez))
        .route("/readyz", get(readyz))
        .layer(middleware::from_fn_with_state(Arc::clone(&state), auth_mw))
        .with_state(state);
    if let Some(cors) = cors_layer(&cfg.cors) {
        app = app.layer(cors);
    }
    app
}

fn cors_layer(spec: &str) -> Option<CorsLayer> {
    let spec = spec.trim();
    if spec.is_empty() {
        return None;
    }
    if spec == "*" {
        return Some(CorsLayer::permissive());
    }
    let origins: Vec<HeaderValue> = spec
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    if origins.is_empty() {
        return None;
    }
    Some(
        CorsLayer::new()
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
            .allow_origin(AllowOrigin::list(origins)),
    )
}

async fn livez() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn readyz(State(state): State<Arc<RpcState>>) -> impl IntoResponse {
    match state.handler.handle("eth_blockNumber", json!([])) {
        Ok(_) => (StatusCode::OK, "ready").into_response(),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "not ready").into_response(),
    }
}

async fn auth_mw(
    State(state): State<Arc<RpcState>>,
    request: Request,
    next: Next,
) -> impl IntoResponse {
    let path = request.uri().path();
    if path == "/livez" || path == "/readyz" {
        return next.run(request).await;
    }
    if let Some(expected) = &state.token {
        let ok = request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| {
                v.strip_prefix("Bearer ")
                    .is_some_and(|got| got == expected.as_str())
            });
        if !ok {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }
    next.run(request).await
}

async fn panel_ui() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        PANEL_HTML,
    )
}

/// Bind and serve JSON-RPC.
///
/// # Errors
///
/// Returns IO errors from bind / serve.
pub async fn serve(handler: RpcHandler, addr: SocketAddr) -> Result<(), std::io::Error> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        router_with_config(handler, RpcHttpConfig::from_env()),
    )
    .await
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
