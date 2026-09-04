//! # Ivory Network
//!
//! libp2p gossip for headers, blocks, and transactions, plus a simple
//! missing-block / missing-header request topic for parent-hash walks.

pub mod behaviour;
pub mod codec;
pub mod error;
pub mod service;

pub use behaviour::{PROTOCOL_VERSION, TOPIC_BLOCKS, TOPIC_SYNC, TOPIC_TXS};
pub use codec::NetworkMessage;
pub use error::NetworkError;
pub use libp2p::{Multiaddr, PeerId};
pub use service::{NetworkConfig, NetworkEvent, NetworkHandle, start};
