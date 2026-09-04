# Structured envelope (`tx.data`)

Optional versioned blob in `Transaction.data`. The node does **structural**
validation only (magic, schema version, non-empty `decision_id`). There is no
on-chain rule engine.

## Wire format

```
IQNT || bincode(QuantEnvelope)
```

Magic is the four ASCII bytes `IQNT`. The body is bincode of:

| Field | Type |
|-------|------|
| `version` | `u16` (currently `1`) |
| `decision_id` | string |
| `schema` | string (application-defined, e.g. `app.v1`) |
| `metrics` | list of `{ name, value }` strings |
| `content_hash` | optional 32-byte hash |
| `cid` | optional string (IPFS CID or other content address) |

Encode/decode lives in `ivory_core::QuantEnvelope`.

## On-chain vs hash-anchor

| Mode | What is in `tx.data` | Tradeoff |
|------|----------------------|----------|
| **On-chain** | Metrics (and small payloads) in the envelope | Readable from the chain; limited by tx size and gas |
| **Hash-anchor** | `content_hash` and/or `cid` only (or plus a summary metric) | Cheap on-chain; verifier must fetch the document elsewhere |

Mixing both is valid.

## RPC smoke

Fund an account, `eth_sendRawTransaction` with hex-bincode of a signed tx whose
`data` is `QuantEnvelope::encode()`, then `eth_getTransactionByHash` and decode
the `input` field.

The Python client (`sdk/python`) wraps this as `submit_decision`.
