//! Transaction signing helpers.
//!
//! Signing payload is [`ivory_core::Transaction::signing_hash`] (bincode of
//! unsigned fields + keccak256).

use ivory_core::Transaction;
use ivory_primitives::{Address, Bytes, SecretKey, U256};

use crate::error::CryptoError;
use crate::keys::{address_from_public_key, public_key_from_secret};
use crate::sign::{sign, verify};

/// Verify `tx.signature` and that `tx.from` matches `tx.public_key`.
///
/// # Errors
///
/// [`CryptoError::InvalidSignature`] or [`CryptoError::KeyMismatch`].
pub fn recover_sender(tx: &Transaction) -> Result<Address, CryptoError> {
    let addr = address_from_public_key(&tx.public_key);
    if addr != tx.from {
        return Err(CryptoError::KeyMismatch);
    }
    verify(tx.signing_hash().as_bytes(), &tx.signature, &tx.public_key)?;
    Ok(addr)
}

/// Fill `public_key`, `from`, and `signature` using `sk`.
pub fn sign_transaction(tx: &mut Transaction, sk: &SecretKey) {
    tx.public_key = public_key_from_secret(sk);
    tx.from = address_from_public_key(&tx.public_key);
    tx.signature = sign(tx.signing_hash().as_bytes(), sk);
}

/// Signed transfer (or call) for tests, benches, and RPC helpers.
#[must_use]
pub fn signed_transfer(
    sk: &SecretKey,
    to: Address,
    nonce: u64,
    value: U256,
    gas: u64,
) -> Transaction {
    signed_tx(sk, Some(to), nonce, value, gas, U256::ONE, Bytes::new())
}

/// Signed transaction with explicit fields (still overwrites `from` / keys).
#[must_use]
pub fn signed_tx(
    sk: &SecretKey,
    to: Option<Address>,
    nonce: u64,
    value: U256,
    gas: u64,
    gas_price: U256,
    data: Bytes,
) -> Transaction {
    let mut tx = Transaction {
        from: Address::zero(),
        to,
        value,
        data,
        gas_price,
        gas,
        nonce,
        signature: ivory_primitives::Signature::zero(),
        public_key: ivory_primitives::PublicKey::zero(),
    };
    sign_transaction(&mut tx, sk);
    tx
}
