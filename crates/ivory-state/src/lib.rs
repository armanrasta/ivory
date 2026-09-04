//! # Ivory State
//!
//! In-memory account and contract storage. Persistence is handled by `ivory-storage`.

pub mod state;
pub mod trie;

pub use state::StateDB;
pub use trie::{empty_root, patricia_root};
