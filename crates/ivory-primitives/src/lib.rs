// crates/ivory-primitives/src/lib.rs

//! # Ivory Primitives
//!
//! Core primitive types for the Ivory Chain blockchain.
//!
//! This crate provides fundamental types used throughout Ivory Chain:
//!
//! - [`H256`], [`H160`], [`H512`] - Fixed-size hash types
//! - [`Address`] - Account addresses
//! - [`U256`], [`U128`] - Large unsigned integers
//! - [`Signature`], [`PublicKey`], [`SecretKey`] - Cryptographic types
//! - [`Bytes`] - Dynamic byte arrays
//!
//! ## Features
//!
//! - `std` (default): Enable standard library support
//! - `serde`: Enable serde serialization/deserialization
//! - `arbitrary`: Enable fuzzing support
//!
//! ## Example
//!
//! ```rust
//! use ivory_primitives::{H256, Address, U256};
//!
//! // Create a hash
//! let hash = H256::zero();
//! assert!(hash.is_zero());
//!
//! // Parse from hex
//! let hash = H256::from_hex("0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef").unwrap();
//!
//! // Create an address
//! let addr = Address::zero();
//!
//! // Work with large integers
//! let value = U256::from(1000u64);
//! let doubled = value.checked_mul(U256::from(2u64)).unwrap();
//! ```

#![cfg_attr(all(not(feature = "std"), not(test)), no_std)]
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

extern crate alloc;

mod hash;
mod address;
mod uint;
mod signature;
mod bytes;
mod error;

pub use hash::{H128, H160, H256, H512, H520};
pub use address::Address;
pub use uint::{U128, U256, U512};
pub use signature::{Signature, PublicKey, SecretKey};
pub use bytes::Bytes;
pub use error::PrimitiveError;

// ─────────────────────────────────────────────────────────────────────────────
// Type Aliases
// ─────────────────────────────────────────────────────────────────────────────

/// Block number/height
pub type BlockNumber = u64;

/// Transaction index within a block
pub type TxIndex = u32;

/// Log index within a transaction
pub type LogIndex = u32;

/// Account nonce for replay protection
pub type Nonce = u64;

/// Gas amount
pub type Gas = u64;

/// Timestamp in milliseconds since Unix epoch
pub type Timestamp = i64;

/// Chain ID for replay protection
pub type ChainId = u64;

// ─────────────────────────────────────────────────────────────────────────────
// Prelude
// ─────────────────────────────────────────────────────────────────────────────

/// Convenient re-exports
pub mod prelude {
    pub use crate::{
        Address, Bytes, H128, H160, H256, H512, PublicKey, SecretKey, Signature,
        U128, U256, U512, BlockNumber, ChainId, Gas, Nonce, Timestamp, TxIndex,
    };
}