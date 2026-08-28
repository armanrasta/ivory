//! # Ivory Storage
//!
//! Persistent key-value storage backed by RocksDB.

pub mod backend;

pub use backend::{RocksDbBackend, StorageError};
