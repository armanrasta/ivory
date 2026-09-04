//! Keccak hexary Patricia trie (bincode nodes, not RLP).

use ivory_primitives::{H256, keccak256};
use serde::{Deserialize, Serialize};

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
    if pairs.is_empty() {
        return empty_root();
    }
    let mut map = std::collections::BTreeMap::new();
    for (key, value) in pairs {
        map.insert(to_nibbles(key), value.clone());
    }
    let items: Vec<(Vec<u8>, Vec<u8>)> = map.into_iter().collect();
    hash_node(&build_node(&items))
}

fn hash_node(node: &TrieNode) -> H256 {
    let bytes = bincode::serialize(node).expect("trie node bincode");
    keccak256(&bytes)
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

fn build_node(items: &[(Vec<u8>, Vec<u8>)]) -> TrieNode {
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
            child: hash_node(&build_node(&stripped)),
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
            children[usize::from(i)] = Some(hash_node(&build_node(&group)));
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
}
