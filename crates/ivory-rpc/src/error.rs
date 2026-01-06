// crates/ivory-rpc/src/error.rs

//! RPC errors

use thiserror::Error;
use crate::jsonrpc::JsonRpcError;

/// RPC errors
#[derive(Debug, Error)]
pub enum RpcError {
    /// Method not found
    #[error("method not found: {0}")]
    MethodNotFound(String),
    
    /// Invalid parameters
    #[error("invalid params: {0}")]
    InvalidParams(String),
    
    /// Internal error
    #[error("internal error: {0}")]
    Internal(String),
    
    /// Server error
    #[error("server error: {0}")]
    Server(String),
    
    /// Transaction not found
    #[error("transaction not found")]
    TransactionNotFound,
    
    /// Block not found
    #[error("block not found")]
    BlockNotFound,
    
    /// Account not found
    #[error("account not found")]
    AccountNotFound,
}

impl From<RpcError> for JsonRpcError {
    fn from(err: RpcError) -> Self {
        match err {
            RpcError::MethodNotFound(m) => JsonRpcError::method_not_found(&m),
            RpcError::InvalidParams(m) => JsonRpcError::invalid_params(&m),
            RpcError::Internal(m) => JsonRpcError::internal_error(&m),
            RpcError::Server(m) => JsonRpcError::internal_error(&m),
            RpcError::TransactionNotFound => JsonRpcError::custom(-32000, "Transaction not found".to_string()),
            RpcError::BlockNotFound => JsonRpcError::custom(-32001, "Block not found".to_string()),
            RpcError::AccountNotFound => JsonRpcError::custom(-32002, "Account not found".to_string()),
        }
    }
}