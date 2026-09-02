//! Network errors.

use thiserror::Error;

/// Failures from swarm setup, publish, or codec.
#[derive(Debug, Error)]
pub enum NetworkError {
    /// libp2p transport or behaviour failed to build.
    #[error("swarm: {0}")]
    Swarm(String),
    /// Gossip publish failed.
    #[error("publish: {0}")]
    Publish(String),
    /// Command channel closed (swarm task exited).
    #[error("network service stopped")]
    Stopped,
    /// Message could not be decoded.
    #[error("invalid network message")]
    InvalidMessage,
    /// Listen / dial multiaddr is invalid.
    #[error("invalid address: {0}")]
    InvalidAddress(String),
}
