//! JSON-RPC 2.0 request and response types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 2.0 request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// Protocol version (`"2.0"`).
    #[serde(default = "default_version")]
    pub jsonrpc: String,
    /// Method name.
    pub method: String,
    /// Positional or object params.
    #[serde(default)]
    pub params: Value,
    /// Request id.
    #[serde(default)]
    pub id: Value,
}

fn default_version() -> String {
    "2.0".to_string()
}

/// JSON-RPC 2.0 success or error envelope.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// Protocol version.
    pub jsonrpc: String,
    /// Result (mutually exclusive with `error`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error (mutually exclusive with `result`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    /// Request id.
    pub id: Value,
}

impl JsonRpcResponse {
    /// Successful result.
    #[must_use]
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Error result.
    #[must_use]
    pub fn error(id: Value, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            result: None,
            error: Some(error),
            id,
        }
    }
}

/// JSON-RPC error object.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct JsonRpcError {
    /// Numeric code.
    pub code: i64,
    /// Human-readable message.
    pub message: String,
    /// Optional data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcError {
    /// -32700
    #[must_use]
    pub fn parse_error() -> Self {
        Self {
            code: -32700,
            message: "Parse error".into(),
            data: None,
        }
    }

    /// -32600
    #[must_use]
    pub fn invalid_request() -> Self {
        Self {
            code: -32600,
            message: "Invalid Request".into(),
            data: None,
        }
    }

    /// -32601
    #[must_use]
    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Method not found: {method}"),
            data: None,
        }
    }

    /// -32602
    #[must_use]
    pub fn invalid_params(msg: &str) -> Self {
        Self {
            code: -32602,
            message: format!("Invalid params: {msg}"),
            data: None,
        }
    }

    /// -32603
    #[must_use]
    pub fn internal_error(msg: &str) -> Self {
        Self {
            code: -32603,
            message: format!("Internal error: {msg}"),
            data: None,
        }
    }

    /// Application error.
    #[must_use]
    pub fn custom(code: i64, message: String) -> Self {
        Self {
            code,
            message,
            data: None,
        }
    }
}
