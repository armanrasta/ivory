//! Ivory key generation.

use ivory_crypto::{Address, PublicKey, SecretKey, generate_keypair};

/// Fresh Ed25519 keypair and v1 address.
#[must_use]
pub fn generate() -> (SecretKey, PublicKey, Address) {
    generate_keypair()
}

/// Encode a secret key as hex (with `0x` prefix).
#[must_use]
pub fn secret_to_hex(sk: &SecretKey) -> String {
    sk.to_hex()
}
