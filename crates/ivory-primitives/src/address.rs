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
    
    /// Convert to checksummed hex string (EIP-55 style)
    ///
    /// Note: This is a simplified version. Full EIP-55 requires keccak256.
    #[inline]
    pub fn to_hex_checksummed(&self) -> String {
        // For now, just return regular hex
        // TODO: Implement proper EIP-55 checksum when we have keccak
        self.0.to_hex()
    }
    
    /// Compute contract address using CREATE semantics
    ///
    /// `address = hash(sender || nonce)[12:]`
    ///
    /// Note: This is a placeholder. Real implementation needs proper hashing.
    pub fn create(sender: &Address, nonce: u64) -> Self {
        // Placeholder implementation
        // Real implementation would use RLP encoding + keccak256
        let mut data = [0u8; 28];
        data[..20].copy_from_slice(sender.as_bytes());
        data[20..28].copy_from_slice(&nonce.to_be_bytes());
        
        // Simple hash (not secure, placeholder only)
        let mut result = [0u8; 20];
        for (i, chunk) in data.chunks(20).enumerate() {
            for (j, &byte) in chunk.iter().enumerate() {
                if j < 20 {
                    result[j] ^= byte.wrapping_add(i as u8);
                }
            }
        }
        Address(H160(result))
    }
    
    /// Compute contract address using CREATE2 semantics
    ///
    /// `address = hash(0xff || sender || salt || code_hash)[12:]`
    ///
    /// Note: This is a placeholder. Real implementation needs proper hashing.
    pub fn create2(sender: &Address, salt: &H256, code_hash: &H256) -> Self {
        // Placeholder implementation
        let mut data = [0u8; 85];
        data[0] = 0xff;
        data[1..21].copy_from_slice(sender.as_bytes());
        data[21..53].copy_from_slice(salt.as_bytes());
        data[53..85].copy_from_slice(code_hash.as_bytes());
        
        // Simple hash (not secure, placeholder only)
        let mut result = [0u8; 20];
        for (i, &byte) in data.iter().enumerate() {
            result[i % 20] ^= byte.wrapping_add((i / 20) as u8);
        }
        Address(H160(result))
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
        let h = H256::from_hex("0x000000000000000000000000742d35cc6634c0532925a3b844bc9e7595f5ab21").unwrap();
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
        let addr: Address = "0x742d35cc6634c0532925a3b844bc9e7595f5ab21".parse().unwrap();
        assert!(!addr.is_zero());
    }
    
    #[test]
    fn test_address_create() {
        let sender = Address::from_hex("0x742d35cc6634c0532925a3b844bc9e7595f5ab21").unwrap();
        let addr1 = Address::create(&sender, 0);
        let addr2 = Address::create(&sender, 1);
        assert_ne!(addr1, addr2);
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