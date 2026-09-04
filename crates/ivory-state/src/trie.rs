//! Keccak hexary Patricia trie (bincode nodes, not RLP).

use ivory_primitives::{H256, keccak256};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Node stored by hash: `keccak256(bincode(self))`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum TrieNode {
    Empty,
    Leaf {
        path: Vec<u8>,
        value: Vec<u8>,
    },
    Extension {
        path: Vec<u8>,
        child: H256,
    },
    Branch {
        children: Box<[Option<H256>; 16]>,
        value: Option<Vec<u8>>,
    },
}

/// Root of a trie with no leaves (`keccak256(bincode(Empty))`).
#[must_use]
pub fn empty_root() -> H256 {
    hash_node(&TrieNode::Empty)
}

/// Build a hexary Patricia root from raw byte keys and values.
///
/// Duplicate keys keep the last value. Order of `pairs` does not matter.
#[must_use]
pub fn patricia_root(pairs: &[(Vec<u8>, Vec<u8>)]) -> H256 {
    patricia_nodes(pairs).0
}

/// Root plus every encoded node (`hash → bincode(TrieNode)`).
#[must_use]
pub fn patricia_nodes(pairs: &[(Vec<u8>, Vec<u8>)]) -> (H256, Vec<(H256, Vec<u8>)>) {
    let mut nodes = Vec::new();
    if pairs.is_empty() {
        let root = store_node(&TrieNode::Empty, &mut nodes);
        return (root, nodes);
    }
    let mut map = std::collections::BTreeMap::new();
    for (key, value) in pairs {
        map.insert(to_nibbles(key), value.clone());
    }
    let items: Vec<(Vec<u8>, Vec<u8>)> = map.into_iter().collect();
    let root = store_tree(&items, &mut nodes);
    (root, nodes)
}

/// Proof walk failed (hash mismatch or undecodable node).
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ProofError {
    /// Node bytes are not a bincode trie node.
    #[error("invalid trie node encoding")]
    InvalidNode,
    /// Encoded node hash does not match the expected child / root.
    #[error("trie proof hash mismatch")]
    HashMismatch,
    /// Proof ended before the walk finished.
    #[error("incomplete trie proof")]
    Incomplete,
}

/// Account or storage Merkle proof: root, encoded nodes, and the leaf value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrieProof {
    /// Patricia root the walk started from.
    pub root: H256,
    /// `bincode(TrieNode)` along the path (root first).
    pub nodes: Vec<Vec<u8>>,
    /// Leaf payload when the key is present.
    pub value: Option<Vec<u8>>,
}

/// Build a path proof for `key` against `pairs`.
#[must_use]
pub fn prove(pairs: &[(Vec<u8>, Vec<u8>)], key: &[u8]) -> TrieProof {
    let (root, stored) = patricia_nodes(pairs);
    let by_hash: std::collections::HashMap<H256, Vec<u8>> = stored.into_iter().collect();
    let mut rest = to_nibbles(key);
    let mut nodes = Vec::new();
    let mut expected = root;
    let value = loop {
        let Some(enc) = by_hash.get(&expected) else {
            break None;
        };
        nodes.push(enc.clone());
        let Ok(node) = bincode::deserialize::<TrieNode>(enc) else {
            break None;
        };
        match node {
            TrieNode::Empty => break None,
            TrieNode::Leaf { path, value: leaf } => {
                break (path == rest).then_some(leaf);
            }
            TrieNode::Extension { path, child } => {
                if rest.starts_with(&path) {
                    rest = rest[path.len()..].to_vec();
                    expected = child;
                } else {
                    break None;
                }
            }
            TrieNode::Branch {
                children,
                value: branch_val,
            } => {
                if rest.is_empty() {
                    break branch_val;
                }
                let i = usize::from(rest[0]);
                rest = rest[1..].to_vec();
                match children[i] {
                    Some(h) => expected = h,
                    None => break None,
                }
            }
        }
    };
    TrieProof { root, nodes, value }
}

/// Walk `proof` from `root` and return the value for `key` (or `None` if excluded).
///
/// # Errors
///
/// [`ProofError`] when a node is corrupt or a hash does not match.
pub fn verify(root: H256, key: &[u8], proof: &[Vec<u8>]) -> Result<Option<Vec<u8>>, ProofError> {
    if proof.is_empty() {
        return Err(ProofError::Incomplete);
    }
    let mut rest = to_nibbles(key);
    let mut expected = root;
    for (i, enc) in proof.iter().enumerate() {
        if keccak256(enc) != expected {
            return Err(ProofError::HashMismatch);
        }
        let node: TrieNode = bincode::deserialize(enc).map_err(|_| ProofError::InvalidNode)?;
        let last = i + 1 == proof.len();
        match node {
            TrieNode::Empty => {
                return if last {
                    Ok(None)
                } else {
                    Err(ProofError::Incomplete)
                };
            }
            TrieNode::Leaf { path, value: leaf } => {
                return if last {
                    Ok((path == rest).then_some(leaf))
                } else {
                    Err(ProofError::Incomplete)
                };
            }
            TrieNode::Extension { path, child } => {
                if !rest.starts_with(&path) {
                    return if last {
                        Ok(None)
                    } else {
                        Err(ProofError::Incomplete)
                    };
                }
                rest = rest[path.len()..].to_vec();
                expected = child;
                if last {
                    return Err(ProofError::Incomplete);
                }
            }
            TrieNode::Branch {
                children,
                value: branch_val,
            } => {
                if rest.is_empty() {
                    return if last {
                        Ok(branch_val)
                    } else {
                        Err(ProofError::Incomplete)
                    };
                }
                let idx = usize::from(rest[0]);
                rest = rest[1..].to_vec();
                match children[idx] {
                    None => {
                        return if last {
                            Ok(None)
                        } else {
                            Err(ProofError::Incomplete)
                        };
                    }
                    Some(h) => {
                        expected = h;
                        if last {
                            return Err(ProofError::Incomplete);
                        }
                    }
                }
            }
        }
    }
    Err(ProofError::Incomplete)
}

fn hash_node(node: &TrieNode) -> H256 {
    let bytes = bincode::serialize(node).expect("trie node bincode");
    keccak256(&bytes)
}

fn store_node(node: &TrieNode, out: &mut Vec<(H256, Vec<u8>)>) -> H256 {
    let bytes = bincode::serialize(node).expect("trie node bincode");
    let h = keccak256(&bytes);
    out.push((h, bytes));
    h
}

fn store_tree(items: &[(Vec<u8>, Vec<u8>)], out: &mut Vec<(H256, Vec<u8>)>) -> H256 {
    store_node(&build_node_collected(items, out), out)
}

fn build_node_collected(items: &[(Vec<u8>, Vec<u8>)], out: &mut Vec<(H256, Vec<u8>)>) -> TrieNode {
    if items.is_empty() {
        return TrieNode::Empty;
    }
    if items.len() == 1 {
        let (path, value) = &items[0];
        return TrieNode::Leaf {
            path: path.clone(),
            value: value.clone(),
        };
    }
    let prefix = common_prefix_len(items);
    if prefix > 0 {
        let stripped: Vec<(Vec<u8>, Vec<u8>)> = items
            .iter()
            .map(|(k, v)| (k[prefix..].to_vec(), v.clone()))
            .collect();
        return TrieNode::Extension {
            path: items[0].0[..prefix].to_vec(),
            child: store_tree(&stripped, out),
        };
    }
    let mut children = [None; 16];
    let mut value = None;
    for i in 0u8..16 {
        let group: Vec<(Vec<u8>, Vec<u8>)> = items
            .iter()
            .filter(|(k, _)| k.first() == Some(&i))
            .map(|(k, v)| (k[1..].to_vec(), v.clone()))
            .collect();
        if !group.is_empty() {
            children[usize::from(i)] = Some(store_tree(&group, out));
        }
    }
    for (key, val) in items {
        if key.is_empty() {
            value = Some(val.clone());
        }
    }
    TrieNode::Branch {
        children: Box::new(children),
        value,
    }
}

fn to_nibbles(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(b >> 4);
        out.push(b & 0x0f);
    }
    out
}

fn common_prefix_len(items: &[(Vec<u8>, Vec<u8>)]) -> usize {
    let Some((first, _)) = items.first() else {
        return 0;
    };
    let mut n = first.len();
    for (key, _) in items.iter().skip(1) {
        let shared = first
            .iter()
            .zip(key.iter())
            .take_while(|(a, b)| a == b)
            .count();
        n = n.min(shared);
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_pairs_use_empty_root() {
        assert_eq!(patricia_root(&[]), empty_root());
        assert_ne!(empty_root(), H256::ZERO);
    }

    #[test]
    fn order_independent() {
        let a = (b"addr-aaaa".to_vec(), b"one".to_vec());
        let b = (b"addr-bbbb".to_vec(), b"two".to_vec());
        assert_eq!(
            patricia_root(&[a.clone(), b.clone()]),
            patricia_root(&[b, a])
        );
    }

    #[test]
    fn last_duplicate_wins() {
        let k = b"same".to_vec();
        let first = (k.clone(), b"old".to_vec());
        let second = (k, b"new".to_vec());
        assert_eq!(
            patricia_root(&[first, second.clone()]),
            patricia_root(&[second])
        );
    }

    #[test]
    fn single_leaf_differs_from_empty() {
        let root = patricia_root(&[(b"k".to_vec(), b"v".to_vec())]);
        assert_ne!(root, empty_root());
    }

    #[test]
    fn prove_and_verify_present_and_absent() {
        let pairs = [
            (b"addr-aaaa".to_vec(), b"one".to_vec()),
            (b"addr-bbbb".to_vec(), b"two".to_vec()),
        ];
        let present = prove(&pairs, b"addr-aaaa");
        assert_eq!(present.value.as_deref(), Some(b"one".as_slice()));
        assert_eq!(
            verify(present.root, b"addr-aaaa", &present.nodes).unwrap(),
            present.value
        );
        let absent = prove(&pairs, b"addr-cccc");
        assert!(absent.value.is_none());
        assert_eq!(
            verify(absent.root, b"addr-cccc", &absent.nodes).unwrap(),
            None
        );
        assert_eq!(
            verify(present.root, b"addr-aaaa", &absent.nodes),
            Err(ProofError::Incomplete)
        );
        let mut bad = present.nodes.clone();
        bad[0][0] ^= 0xff;
        assert_eq!(
            verify(present.root, b"addr-aaaa", &bad),
            Err(ProofError::HashMismatch)
        );
    }
}
