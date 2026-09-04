//! # Ivory State
//!
//! In-memory account and contract storage. Persistence is handled by `ivory-storage`.

pub mod state;
pub mod trie;

pub use state::{StateDB, StateSnapshot};
pub use trie::{ProofError, TrieProof, empty_root, patricia_nodes, patricia_root, prove, verify};
