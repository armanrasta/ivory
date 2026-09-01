//! # Ivory Consensus
//!
//! Proof-of-Authority engine: validator set, header seals in `extra_data`.
//! Signature recovery is a stub until #28 / #16.

pub mod engine;
pub mod error;
pub mod poa;
pub mod seal;

pub use engine::ConsensusEngine;
pub use error::ConsensusError;
pub use poa::{PoAConfig, PoAConsensus};
pub use seal::{SEAL_LEN, decode_seals, encode_seals, seal_count, verify_seal};
