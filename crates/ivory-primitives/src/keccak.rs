//! Keccak-256 (Ethereum-style; not SHA3-256).

use sha3::{Digest, Keccak256};

use crate::H256;

/// 32-byte Keccak-256 digest of `data`.
#[must_use]
pub fn keccak256(data: &[u8]) -> H256 {
    let mut hasher = Keccak256::new();
    hasher.update(data);
    let out = hasher.finalize();
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&out);
    H256::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_matches_ethereum() {
        assert_eq!(
            keccak256(b"").to_hex(),
            "0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
        );
    }
}
