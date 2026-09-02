//! # Ivory Consensus
//!
//! Proof-of-Authority engine: validator set and Ed25519 header seals in
//! `extra_data`. Transaction signatures are verified in the tx pool.

pub mod engine;
pub mod error;
pub mod poa;
pub mod seal;

pub use engine::ConsensusEngine;
pub use error::ConsensusError;
pub use poa::{PoAConfig, PoAConsensus, Validator};
pub use seal::{
    SEAL_LEN, decode_seals, encode_seals, encode_signatures, seal_count, seal_hash, verify_seal,
};
