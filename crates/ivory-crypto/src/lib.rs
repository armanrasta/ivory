//! # Ivory Crypto
//!
//! Ed25519 sign/verify and v1 address derivation (`blake3` of the public key).
//! Transaction signing uses [`ivory_core::Transaction::signing_hash`] until #16.

pub mod error;
pub mod keys;
pub mod sign;
pub mod tx;

pub use error::CryptoError;
pub use keys::{
    address_from_public_key, address_from_secret, generate_keypair, keypair_from_byte,
    keypair_from_seed, public_key_from_secret, secret_from_bytes,
};
pub use sign::{sign, verify};
pub use tx::{recover_sender, sign_transaction, signed_transfer, signed_tx};

pub use ivory_primitives::{Address, H256, PublicKey, SecretKey, Signature};

#[cfg(test)]
mod tests {
    use ivory_core::Transaction;
    use ivory_primitives::{Address, Bytes, U256};

    use super::*;

    #[test]
    fn sign_verify_roundtrip() {
        let (sk, pk, _) = keypair_from_byte(1);
        let msg = b"ivory-v1";
        let sig = sign(msg, &sk);
        verify(msg, &sig, &pk).unwrap();
    }

    #[test]
    fn verify_rejects_wrong_key() {
        let (sk, _, _) = keypair_from_byte(1);
        let (_, pk2, _) = keypair_from_byte(2);
        let sig = sign(b"msg", &sk);
        assert_eq!(
            verify(b"msg", &sig, &pk2),
            Err(CryptoError::InvalidSignature)
        );
    }

    #[test]
    fn verify_rejects_mangled_signature() {
        let (sk, pk, _) = keypair_from_byte(1);
        let mut sig = sign(b"msg", &sk);
        let mut bytes = sig.to_bytes();
        bytes[0] ^= 0xff;
        sig = Signature::from_bytes(bytes);
        assert_eq!(
            verify(b"msg", &sig, &pk),
            Err(CryptoError::InvalidSignature)
        );
    }

    #[test]
    fn address_is_deterministic() {
        let (_, pk, addr) = keypair_from_byte(7);
        assert_eq!(address_from_public_key(&pk), addr);
        assert_eq!(keypair_from_byte(7).2, addr);
        assert_ne!(addr, Address::zero());
    }

    #[test]
    fn generate_keypair_is_unique() {
        let a = generate_keypair();
        let b = generate_keypair();
        assert_ne!(a.1, b.1);
        assert_ne!(a.2, b.2);
    }

    #[test]
    fn signed_transfer_recovers_sender() {
        let (sk, pk, from) = keypair_from_byte(3);
        let to = keypair_from_byte(4).2;
        let tx = signed_transfer(&sk, to, 0, U256::from(10u64), 21_000);
        assert_eq!(tx.from, from);
        assert_eq!(tx.public_key, pk);
        assert_eq!(recover_sender(&tx).unwrap(), from);
    }

    #[test]
    fn recover_rejects_wrong_from() {
        let (sk, _, _) = keypair_from_byte(3);
        let to = keypair_from_byte(4).2;
        let mut tx = signed_transfer(&sk, to, 0, U256::from(10u64), 21_000);
        tx.from = Address::from_bytes([9u8; 20]);
        assert_eq!(recover_sender(&tx), Err(CryptoError::KeyMismatch));
    }

    #[test]
    fn recover_rejects_bad_signature() {
        let (sk, _, _) = keypair_from_byte(3);
        let to = keypair_from_byte(4).2;
        let mut tx = signed_transfer(&sk, to, 0, U256::from(10u64), 21_000);
        let mut bytes = tx.signature.to_bytes();
        bytes[1] ^= 0x01;
        tx.signature = Signature::from_bytes(bytes);
        assert_eq!(recover_sender(&tx), Err(CryptoError::InvalidSignature));
    }

    #[test]
    fn signing_hash_excludes_signature() {
        let (sk, _, _) = keypair_from_byte(1);
        let to = keypair_from_byte(2).2;
        let tx = signed_transfer(&sk, to, 0, U256::from(1u64), 21_000);
        let unsigned = Transaction {
            from: tx.from,
            to: tx.to,
            value: tx.value,
            data: Bytes::new(),
            gas_price: tx.gas_price,
            gas: tx.gas,
            nonce: tx.nonce,
            signature: Signature::zero(),
            public_key: ivory_primitives::PublicKey::zero(),
        };
        assert_eq!(tx.signing_hash(), unsigned.signing_hash());
        assert_ne!(tx.hash(), tx.signing_hash());
    }

    #[test]
    fn secret_from_bytes_roundtrip() {
        let (sk, pk, _) = keypair_from_byte(8);
        let sk2 = secret_from_bytes(sk.to_bytes()).unwrap();
        assert_eq!(public_key_from_secret(&sk2), pk);
    }
}
