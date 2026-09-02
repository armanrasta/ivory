//! # Ivory RPC
//!
//! JSON-RPC 2.0 over HTTP and WebSocket.

pub mod context;
pub mod error;
pub mod handler;
pub mod jsonrpc;
pub mod server;
pub mod types;
pub mod websocket;

pub use context::RpcContext;
pub use error::RpcError;
pub use handler::RpcHandler;
pub use jsonrpc::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
pub use server::{RpcState, router, serve};
