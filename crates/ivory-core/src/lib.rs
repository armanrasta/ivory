//! # Ivory Core
//!
//! Shared ledger types: accounts, blocks, transactions, receipts, and logs.

pub mod account;
pub mod block;
pub mod error;
pub mod quant;

pub use account::Account;
pub use block::{Block, BlockHeader, Log, Receipt, Transaction};
pub use error::BlockError;
pub use quant::{QUANT_MAGIC, QUANT_SCHEMA_VERSION, QuantEnvelope, QuantMetric};
