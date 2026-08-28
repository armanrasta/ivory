# Ivory overview

Ivory is a Rust blockchain workspace aimed at **permissioned ledgers** and, first, an immutable store for **Orbis decision receipts**.

## Design choices (v1)

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Consensus | Proof-of-Authority | Simple validator set; enough for private / consortium nets |
| Storage | RocksDB + in-memory maps | Fast to ship; Merkle-Patricia trie deferred |
| Contracts | WASM via wasmi (planned) | Pure Rust interpreter; auditable |
| Networking | libp2p | Rust-first; gossipsub + DHT |
| RPC | JSON-RPC 2.0 (Axum) | Familiar to Ethereum tooling |

## What works today

- **Primitives** — hashes, addresses, `U256`, bytes, Ed25519-shaped signature types
- **Core** — `Account`, `BlockHeader`, `Transaction`, `Receipt`, `Log`; `Block::validate` (gas)
- **State** — `StateDB` over `HashMap` (no real state root yet)
- **Storage** — `RocksDbBackend` KV API

Header hashing is currently **bincode + blake3** (placeholder until RLP/keccak).

## What is next

See [issue #24](https://github.com/armanrasta/ivory/issues/24): tx pool → executor → PoA → chain → network → RPC → Orbis SDK → testnet.

## Crate dependency sketch

```
ivory-rpc ──► ivory-core ──► ivory-primitives
ivory-state ──► ivory-core
ivory-storage ──► rocksdb
ivory-executor ──► ivory-state, ivory-vm   (planned)
ivory-chain ──► ivory-consensus, ivory-core  (planned)
```
