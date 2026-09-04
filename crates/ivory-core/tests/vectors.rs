//! Frozen keccak / address / list-root / header / Ed25519 vectors.

use ivory_core::{BlockHeader, Receipt, Transaction, empty_list_roots, list_root};
use ivory_crypto::{keypair_from_byte, sign_transaction};
use ivory_primitives::{Address, Bytes, H256, U256, keccak256};

const KECCAK_EMPTY: &str = "0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470";
const ADDR_KEYPAIR_1: &str = "0x97b1c813eae702332ba3eaa1625f942c5472626d";
const CREATE_NONCE_0: &str = "0xbfababe70da3d03e64ab6e8f5976878e561ab240";
const CREATE_NONCE_1: &str = "0x584379695e7355db208ec36f7fc9d6d1adfbc68f";
const CREATE2_EMPTY_CODE: &str = "0x1c9af2c7e0690294ede14e10379a4e7f3e05da3c";
const SIGNING_HASH: &str = "0xd79782022aa4500e8ee4ad17526ae86687dd437b4f572d429dff47e88032d0bd";
const TX_HASH: &str = "0xbe7847d85f9109f7e14d99e97e5f54638f46724ed01127819c78cdb1f36a8e73";
const ED25519_SIG: &str = "0x4a2c495d7b81b32a85b2e818db083501d1d2123e4dfb041deacf145a7f94bd050e575784b1d4f945beae6f0576333ef9adcd7b01db8e4ee7c802e80e80e94603";
const EMPTY_LIST_ROOT: &str = "0x011b4d03dd8c01f1049143cf9c4c817e4b167f1d1b83e5c6f0f10d89ba1e7bce";
const DOCUMENTED_HEADER_HASH: &str =
    "0xa340036d231fba441ae03facd3d10bc20d2f76d8d24f20732af5a4618edcc1cc";

fn canonical_signed_transfer() -> Transaction {
    let (sk, _, _) = keypair_from_byte(1);
    let to = keypair_from_byte(2).2;
    let mut tx = Transaction {
        from: Address::zero(),
        to: Some(to),
        value: U256::from(1u64),
        data: Bytes::new(),
        gas_price: U256::ONE,
        gas: 21_000,
        nonce: 0,
        signature: ivory_primitives::Signature::zero(),
        public_key: ivory_primitives::PublicKey::zero(),
    };
    sign_transaction(&mut tx, &sk);
    tx
}

fn documented_header() -> BlockHeader {
    let (tx_root, rx_root) = empty_list_roots();
    BlockHeader {
        number: 0,
        parent_hash: H256::ZERO,
        timestamp: 1,
        miner: Address::zero(),
        gas_limit: 30_000_000,
        gas_used: 0,
        state_root: H256::ZERO,
        transactions_root: tx_root,
        receipts_root: rx_root,
        difficulty: U256::ZERO,
        extra_data: Bytes::new(),
    }
}

#[test]
fn keccak256_empty_bytes() {
    assert_eq!(keccak256(b"").to_hex(), KECCAK_EMPTY);
}

#[test]
fn address_from_keypair_byte_1() {
    assert_eq!(keypair_from_byte(1).2.to_hex(), ADDR_KEYPAIR_1);
}

#[test]
fn address_create_nonce_0_and_1() {
    let sender = keypair_from_byte(1).2;
    assert_eq!(Address::create(&sender, 0).to_hex(), CREATE_NONCE_0);
    assert_eq!(Address::create(&sender, 1).to_hex(), CREATE_NONCE_1);
}

#[test]
fn address_create2_ff_sender_salt_code_hash() {
    let sender = keypair_from_byte(1).2;
    let salt = H256::ZERO;
    let code_hash = keccak256(b"");
    assert_eq!(
        Address::create2(&sender, &salt, &code_hash).to_hex(),
        CREATE2_EMPTY_CODE
    );
}

#[test]
fn unsigned_transfer_signing_hash() {
    assert_eq!(
        canonical_signed_transfer().signing_hash().to_hex(),
        SIGNING_HASH
    );
}

#[test]
fn signed_transfer_hash_keypair_1() {
    assert_eq!(canonical_signed_transfer().hash().to_hex(), TX_HASH);
}

#[test]
fn empty_list_roots_frozen() {
    let (tx_root, rx_root) = empty_list_roots();
    assert_eq!(tx_root.to_hex(), EMPTY_LIST_ROOT);
    assert_eq!(rx_root.to_hex(), EMPTY_LIST_ROOT);
    assert_eq!(list_root::<Transaction>(&[]).to_hex(), EMPTY_LIST_ROOT);
    assert_eq!(list_root::<Receipt>(&[]).to_hex(), EMPTY_LIST_ROOT);
    assert_ne!(tx_root, H256::ZERO);
}

#[test]
fn documented_header_hash() {
    let header = documented_header();
    assert_eq!(header.hash().to_hex(), DOCUMENTED_HEADER_HASH);
    assert_eq!(header.difficulty, U256::ZERO);
}

#[test]
fn ed25519_known_sig_over_signing_hash() {
    let tx = canonical_signed_transfer();
    assert_eq!(tx.signature.to_hex(), ED25519_SIG);
}

#[test]
fn eip55_published_fixture() {
    let addr = Address::from_hex("0x5aaeb6053f3e94c9b9a09f33669435e7ef1beaed").unwrap();
    assert_eq!(
        addr.to_hex_checksummed(),
        "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed"
    );
}
