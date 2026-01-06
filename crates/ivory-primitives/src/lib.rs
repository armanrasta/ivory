// crates/ivory-primitives/src/lib.rs

//! Ivory Chain primitive types

pub mod hash;
pub mod address;
pub mod uint;
pub mod signature;
pub mod error;

pub use hash::{H256, H160, H512};
pub use address::Address;
pub use uint::U256;
pub use signature::{Signature, PublicKey, SecretKey};
pub use error::PrimitiveError;

/// Block number type
pub type BlockNumber = u64;
/// Transaction index
pub type TxIndex = u32;
/// Nonce
pub type Nonce = u64;
/// Gas
pub type Gas = u64;
/// Timestamp (milliseconds)
pub type Timestamp = i64;
/// Chain ID
pub type ChainId = u64;
/// Bytes alias
pub type Bytes = Vec<u8>;