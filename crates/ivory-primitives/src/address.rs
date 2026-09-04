// crates/ivory-primitives/src/address.rs

//! Account address type.
//!
//! Addresses in Ivory Chain are 20-byte identifiers derived from public keys.

use alloc::string::String;
use core::fmt;
use core::str::FromStr;

use crate::{H160, H256, PrimitiveError};

/// Account address (20 bytes).
///
/// An address uniquely identifies an account on Ivory Chain.
/// It is typically derived from the last 20 bytes of a public key hash.
///
/// # Example
///
/// ```rust
/// use ivory_primitives::Address;
///
/// let addr = Address::zero();
/// assert!(addr.is_zero());
///
/// let addr = Address::from_hex("0x742d35Cc6634C0532925a3b844Bc9e7595f5aB21").unwrap();
/// println!("Address: {}", addr);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct Address(pub H160);

impl Address {
    /// Size in bytes
    pub const SIZE: usize = 20;

    /// Zero address (all bytes are 0)
    pub const ZERO: Self = Address(H160::ZERO);

    /// Create a zero address
    #[inline]
    pub const fn zero() -> Self {
        Self::ZERO
    }

    /// Create from H160
    #[inline]
    pub const fn from_h160(h: H160) -> Self {
        Address(h)
    }

    /// Create from byte array
    #[inline]
    pub const fn from_bytes(bytes: [u8; 20]) -> Self {
        Address(H160(bytes))
    }

    /// Create from byte slice
    #[inline]
    pub fn from_slice(slice: &[u8]) -> Option<Self> {
        H160::from_slice(slice).map(Address)
    }

    /// Create from hex string
    pub fn from_hex(s: &str) -> Result<Self, PrimitiveError> {
        H160::from_hex(s).map(Address)
    }

    /// Create from H256 (take last 20 bytes)
    ///
    /// This is commonly used when deriving an address from a public key hash.
    #[inline]
    pub fn from_h256(h: H256) -> Self {
        Address(h.to_h160())
    }

    /// Get as byte slice
    #[inline]
    pub const fn as_bytes(&self) -> &[u8; 20] {
        self.0.as_bytes()
    }

    /// Get as mutable byte slice
    #[inline]
    pub fn as_bytes_mut(&mut self) -> &mut [u8; 20] {
        self.0.as_bytes_mut()
    }

    /// Convert to byte array
    #[inline]
    pub const fn to_bytes(self) -> [u8; 20] {
        self.0.to_bytes()
    }

    /// Convert to H160
    #[inline]
    pub const fn to_h160(self) -> H160 {
        self.0
    }

    /// Convert to H256 (left-padded with zeros)
    #[inline]
    pub fn to_h256(self) -> H256 {
        H256::from_h160(self.0)
    }

    /// Check if zero address
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// Convert to hex string with "0x" prefix
    #[inline]
    pub fn to_hex(&self) -> String {
        self.0.to_hex()
    }

    /// Convert to checksummed hex string (EIP-55).
    #[must_use]
    pub fn to_hex_checksummed(&self) -> String {
        let lower = hex::encode(self.as_bytes());
        let hash = crate::keccak256(lower.as_bytes());
        let mut out = String::from("0x");
        for (i, ch) in lower.chars().enumerate() {
            if ch.is_ascii_digit() {
                out.push(ch);
                continue;
            }
            let byte = hash.as_bytes()[i / 2];
            let nibble = if i % 2 == 0 { byte >> 4 } else { byte & 0x0f };
            if nibble >= 8 {
                out.push(ch.to_ascii_uppercase());
            } else {
                out.push(ch);
            }
        }
        out
    }

    /// Contract address from `CREATE`: `keccak256(sender || nonce_be)[12:]`.
    ///
    /// `nonce` is the sender nonce **before** the creating transaction increments it.
    #[must_use]
    pub fn create(sender: &Address, nonce: u64) -> Self {
        let mut data = [0u8; 28];
        data[..20].copy_from_slice(sender.as_bytes());
        data[20..28].copy_from_slice(&nonce.to_be_bytes());
        Address::from_h256(crate::keccak256(&data))
    }

    /// Contract address from `CREATE2`: `keccak256(0xff || sender || salt || code_hash)[12:]`.
    #[must_use]
    pub fn create2(sender: &Address, salt: &H256, code_hash: &H256) -> Self {
        let mut data = [0u8; 85];
        data[0] = 0xff;
        data[1..21].copy_from_slice(sender.as_bytes());
        data[21..53].copy_from_slice(salt.as_bytes());
        data[53..85].copy_from_slice(code_hash.as_bytes());
        Address::from_h256(crate::keccak256(&data))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Trait Implementations
// ─────────────────────────────────────────────────────────────────────────────

impl AsRef<[u8]> for Address {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl AsRef<H160> for Address {
    #[inline]
    fn as_ref(&self) -> &H160 {
        &self.0
    }
}

impl From<H160> for Address {
    #[inline]
    fn from(h: H160) -> Self {
        Address(h)
    }
}

impl From<Address> for H160 {
    #[inline]
    fn from(addr: Address) -> Self {
        addr.0
    }
}

impl From<[u8; 20]> for Address {
    #[inline]
    fn from(arr: [u8; 20]) -> Self {
        Address(H160(arr))
    }
}

impl From<Address> for [u8; 20] {
    #[inline]
    fn from(addr: Address) -> Self {
        addr.0.0
    }
}

impl TryFrom<&[u8]> for Address {
    type Error = PrimitiveError;

    fn try_from(slice: &[u8]) -> Result<Self, Self::Error> {
        H160::try_from(slice).map(Address)
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Address({})", self.0.to_hex())
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.to_hex())
    }
}

impl fmt::LowerHex for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

impl FromStr for Address {
    type Err = PrimitiveError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_hex(s)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Address {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Address {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        H160::deserialize(deserializer).map(Address)
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for Address {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        H160::arbitrary(u).map(Address)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_zero() {
        let addr = Address::zero();
        assert!(addr.is_zero());
        assert_eq!(addr, Address::ZERO);
    }

    #[test]
    fn test_address_from_hex() {
        let addr = Address::from_hex("0x742d35Cc6634C0532925a3b844Bc9e7595f5aB21").unwrap();
        assert!(!addr.is_zero());
        assert_eq!(addr.as_bytes()[0], 0x74);
    }

    #[test]
    fn test_address_from_hex_lowercase() {
        let addr = Address::from_hex("0x742d35cc6634c0532925a3b844bc9e7595f5ab21").unwrap();
        assert!(!addr.is_zero());
    }

    #[test]
    fn test_address_display() {
        let addr = Address::from_hex("0x742d35Cc6634C0532925a3b844Bc9e7595f5aB21").unwrap();
        let display = format!("{}", addr);
        assert!(display.starts_with("0x"));
        assert_eq!(display.len(), 42); // "0x" + 40 hex chars
    }

    #[test]
    fn test_address_from_h256() {
        let h =
            H256::from_hex("0x000000000000000000000000742d35cc6634c0532925a3b844bc9e7595f5ab21")
                .unwrap();
        let addr = Address::from_h256(h);
        assert_eq!(addr.as_bytes()[0], 0x74);
    }

    #[test]
    fn test_address_to_h256() {
        let addr = Address::from_hex("0x742d35Cc6634C0532925a3b844Bc9e7595f5aB21").unwrap();
        let h = addr.to_h256();
        // First 12 bytes should be zero
        for i in 0..12 {
            assert_eq!(h.0[i], 0);
        }
        // Last 20 bytes should be the address
        assert_eq!(&h.0[12..], addr.as_bytes());
    }

    #[test]
    fn test_address_roundtrip() {
        let original = "0x742d35cc6634c0532925a3b844bc9e7595f5ab21";
        let addr = Address::from_hex(original).unwrap();
        let back = addr.to_hex();
        assert_eq!(original, back);
    }

    #[test]
    fn test_address_parse() {
        let addr: Address = "0x742d35cc6634c0532925a3b844bc9e7595f5ab21"
            .parse()
            .unwrap();
        assert!(!addr.is_zero());
    }

    #[test]
    fn test_address_create() {
        let sender = Address::from_hex("0x742d35cc6634c0532925a3b844bc9e7595f5ab21").unwrap();
        let addr1 = Address::create(&sender, 0);
        let addr2 = Address::create(&sender, 1);
        assert_ne!(addr1, addr2);
        assert!(!addr1.is_zero());
    }

    #[test]
    fn test_eip55_checksum() {
        let addr = Address::from_hex("0x5aaeb6053f3e94c9b9a09f33669435e7ef1beaed").unwrap();
        assert_eq!(
            addr.to_hex_checksummed(),
            "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed"
        );
    }

    #[test]
    fn test_eip55_published_fixture() {
        let addr = Address::from_hex("0xfb6916095ca1df60bb79ce92ce3ea74c37c5d359").unwrap();
        assert_eq!(
            addr.to_hex_checksummed(),
            "0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359"
        );
    }

    #[test]
    fn test_address_create2() {
        let sender = Address::from_hex("0x742d35cc6634c0532925a3b844bc9e7595f5ab21").unwrap();
        let salt = H256::ZERO;
        let code_hash = crate::keccak256(b"ivory-create2");
        let a = Address::create2(&sender, &salt, &code_hash);
        let b = Address::create2(&sender, &H256::from_bytes([1u8; 32]), &code_hash);
        assert_ne!(a, b);
        assert!(!a.is_zero());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn test_address_serde() {
        let original = Address::from_hex("0x742d35cc6634c0532925a3b844bc9e7595f5ab21").unwrap();
        let json = serde_json::to_string(&original).unwrap();
        let decoded: Address = serde_json::from_str(&json).unwrap();
        assert_eq!(original, decoded);
    }
}
