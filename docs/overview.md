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
- **Core** — `Account`, `BlockHeader`, `Transaction`, `Receipt`, `Log`; `Block::validate` (gas); `Transaction::hash` (bincode + blake3)
- **State** — `StateDB` over `HashMap` (no real state root yet)
- **Storage** — `RocksDbBackend` KV API
- **Tx pool** — strict contiguous nonces, pending entries (`ivory-txpool`)
- **Executor** — transfers, intrinsic gas, refunds; WASM stubbed (`ivory-executor`)

Header hashing is currently **bincode + blake3** (placeholder until RLP/keccak).

## Benchmarks

Hot-path Criterion results (hash, state, pool, execute, pipeline): [benchmarks.md](benchmarks.md).

## What is next

See [issue #24](https://github.com/armanrasta/ivory/issues/24): **#8 PoA → #9 chain** (then #10 network); **#7 WASM** in parallel; **#28** sig verify before dishonest admissions. Quant envelope: [#27](https://github.com/armanrasta/ivory/issues/27). CI/benches shipped: #25/#26.

## Crate dependency sketch

```
ivory-rpc ──► ivory-core ──► ivory-primitives
ivory-state ──► ivory-core
ivory-storage ──► rocksdb
ivory-executor ──► ivory-state, ivory-vm   (planned)
ivory-chain ──► ivory-consensus, ivory-core  (planned)
```
