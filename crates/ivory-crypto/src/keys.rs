//! Key generation and address derivation.

use ivory_primitives::{Address, PublicKey, SecretKey};
use rand::rngs::OsRng;

use crate::error::CryptoError;

/// Derive a v1 account address from an Ed25519 public key.
///
/// `blake3(pubkey)` then the last 20 bytes ([`Address::from_h256`]).
/// This domain is a placeholder until protocol hashes land in #16.
#[must_use]
pub fn address_from_public_key(pk: &PublicKey) -> Address {
    let digest = blake3::hash(pk.as_bytes());
    Address::from_h256(ivory_primitives::H256::from_bytes(*digest.as_bytes()))
}

/// Public key corresponding to `sk`.
#[must_use]
pub fn public_key_from_secret(sk: &SecretKey) -> PublicKey {
    let signing = ed25519_dalek::SigningKey::from_bytes(sk.as_bytes());
    PublicKey::from_bytes(signing.verifying_key().to_bytes())
}

/// Address corresponding to `sk`.
#[must_use]
pub fn address_from_secret(sk: &SecretKey) -> Address {
    address_from_public_key(&public_key_from_secret(sk))
}

/// Generate a random Ed25519 keypair and its v1 address.
#[must_use]
pub fn generate_keypair() -> (SecretKey, PublicKey, Address) {
    let signing = ed25519_dalek::SigningKey::generate(&mut OsRng);
    let sk = SecretKey::from_bytes(signing.to_bytes());
    let pk = PublicKey::from_bytes(signing.verifying_key().to_bytes());
    let addr = address_from_public_key(&pk);
    (sk, pk, addr)
}

/// Deterministic keypair from a 32-byte seed (tests and benches).
#[must_use]
pub fn keypair_from_seed(seed: [u8; 32]) -> (SecretKey, PublicKey, Address) {
    let signing = ed25519_dalek::SigningKey::from_bytes(&seed);
    let sk = SecretKey::from_bytes(signing.to_bytes());
    let pk = PublicKey::from_bytes(signing.verifying_key().to_bytes());
    let addr = address_from_public_key(&pk);
    (sk, pk, addr)
}

/// Convenience seed: all bytes equal to `b`.
#[must_use]
pub fn keypair_from_byte(b: u8) -> (SecretKey, PublicKey, Address) {
    keypair_from_seed([b; 32])
}

/// Parse a secret key from raw bytes.
///
/// # Errors
///
/// Currently infallible for 32-byte keys; reserved for future clamping checks.
pub fn secret_from_bytes(bytes: [u8; 32]) -> Result<SecretKey, CryptoError> {
    Ok(SecretKey::from_bytes(bytes))
}
