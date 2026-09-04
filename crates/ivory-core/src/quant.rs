//! Versioned application payload carried in `tx.data`.

use ivory_primitives::{Bytes, H256};
use serde::{Deserialize, Serialize};

use crate::error::BlockError;

/// ASCII magic prefix (`IQNT`) for [`QuantEnvelope::encode`].
pub const QUANT_MAGIC: [u8; 4] = *b"IQNT";

/// Current envelope schema version.
pub const QUANT_SCHEMA_VERSION: u16 = 1;

/// Named metric stored in an envelope (structural only; no WASM rules).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuantMetric {
    /// Metric name (e.g. `score`).
    pub name: String,
    /// Decimal or free-form value as a string.
    pub value: String,
}

/// Optional structured payload placed in [`super::Transaction::data`].
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuantEnvelope {
    /// Schema version (must be [`QUANT_SCHEMA_VERSION`] for decode).
    pub version: u16,
    /// Application-defined id.
    pub decision_id: String,
    /// Named schema (e.g. `app.v1`).
    pub schema: String,
    /// Key/value metrics.
    pub metrics: Vec<QuantMetric>,
    /// Optional content hash of off-chain payload.
    pub content_hash: Option<H256>,
    /// Optional IPFS/CID (or other content address) for the full document.
    pub cid: Option<String>,
}

impl QuantEnvelope {
    /// Encode as `IQNT` magic + bincode body.
    #[must_use]
    pub fn encode(&self) -> Bytes {
        let body = bincode::serialize(self).expect("envelope serialization is infallible");
        let mut out = Vec::with_capacity(4 + body.len());
        out.extend_from_slice(&QUANT_MAGIC);
        out.extend_from_slice(&body);
        Bytes::from_vec(out)
    }

    /// Decode bytes produced by [`Self::encode`].
    ///
    /// Structural checks only (magic, version, non-empty `decision_id`).
    ///
    /// # Errors
    ///
    /// [`BlockError::InvalidQuantEnvelope`].
    pub fn decode(data: &[u8]) -> Result<Self, BlockError> {
        if data.len() < 4 || data[..4] != QUANT_MAGIC {
            return Err(BlockError::InvalidQuantEnvelope("missing IQNT magic"));
        }
        let env: Self = bincode::deserialize(&data[4..])
            .map_err(|_| BlockError::InvalidQuantEnvelope("bincode"))?;
        if env.version != QUANT_SCHEMA_VERSION {
            return Err(BlockError::InvalidQuantEnvelope("unsupported version"));
        }
        if env.decision_id.is_empty() {
            return Err(BlockError::InvalidQuantEnvelope("empty decision_id"));
        }
        Ok(env)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> QuantEnvelope {
        QuantEnvelope {
            version: QUANT_SCHEMA_VERSION,
            decision_id: "dec-1".into(),
            schema: "app.v1".into(),
            metrics: vec![QuantMetric {
                name: "score".into(),
                value: "0.82".into(),
            }],
            content_hash: Some(H256::from_bytes([9u8; 32])),
            cid: Some("bafyexample".into()),
        }
    }

    #[test]
    fn roundtrip() {
        let original = sample();
        let encoded = original.encode();
        assert_eq!(&encoded.as_slice()[..4], &QUANT_MAGIC);
        let decoded = QuantEnvelope::decode(encoded.as_slice()).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn rejects_bad_magic() {
        assert!(QuantEnvelope::decode(b"NOPE").is_err());
    }

    #[test]
    fn rejects_empty_decision_id() {
        let mut env = sample();
        env.decision_id.clear();
        assert!(QuantEnvelope::decode(env.encode().as_slice()).is_err());
    }

    #[test]
    fn bincode_layout_matches_python_codec() {
        let env = QuantEnvelope {
            version: QUANT_SCHEMA_VERSION,
            decision_id: "d".into(),
            schema: "s".into(),
            metrics: Vec::new(),
            content_hash: None,
            cid: None,
        };
        let mut expected = QUANT_MAGIC.to_vec();
        expected.extend_from_slice(&1u16.to_le_bytes());
        expected.extend_from_slice(&1u64.to_le_bytes());
        expected.push(b'd');
        expected.extend_from_slice(&1u64.to_le_bytes());
        expected.push(b's');
        expected.extend_from_slice(&0u64.to_le_bytes());
        expected.push(0);
        expected.push(0);
        assert_eq!(env.encode().as_slice(), expected);
    }
}
