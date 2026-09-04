//! # Ivory Chain
//!
//! In-memory canonical chain, forks, and block production from the tx pool.

pub mod apply;
pub mod error;
pub mod producer;
pub mod store;

pub use apply::import_and_apply;
pub use error::ChainError;
pub use producer::{BlockProducer, ProduceParams};
pub use store::{BlockStore, InsertOutcome, TxLocation};
