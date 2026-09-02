//! # Ivory Network
//!
//! libp2p gossip for blocks and transactions, plus a simple missing-block
//! request topic for parent-hash walks.

pub mod behaviour;
pub mod codec;
pub mod error;
pub mod service;

pub use behaviour::{PROTOCOL_VERSION, TOPIC_BLOCKS, TOPIC_SYNC, TOPIC_TXS};
pub use codec::NetworkMessage;
pub use error::NetworkError;
pub use service::{NetworkConfig, NetworkEvent, NetworkHandle, start};
