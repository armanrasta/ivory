# Ivory

A modular blockchain in Rust — built as an immutable **decision ledger** for platforms like [Orbis](https://github.com/armanrasta), and usable as a general permissioned chain.

Ivory is early. Primitives, core types, in-memory state, and RocksDB storage are in place. Consensus, networking, RPC handlers, and the WASM VM are still stubs.

## Why Ivory

| | |
|---|---|
| **Purpose** | Timestamp and prove quantitative decisions (AHP, supply-chain opts, quality metrics) on-chain |
| **Stack** | Rust workspace · Tokio · Axum · libp2p · RocksDB · wasmi (planned) |
| **Consensus (v1)** | Proof-of-Authority — simple, permissioned; PoS later if needed |
| **API** | Ethereum-style JSON-RPC over HTTP/WebSocket (in progress) |

Competitors often store analytics in a database. Ivory aims to make those decisions auditable and blockchain-verified.

## Status

| Layer | Crate | Status |
|-------|--------|--------|
| Primitives | `ivory-primitives` | Done — `H256`, `Address`, `U256`, `Bytes`, `Signature` |
| Crypto re-exports | `ivory-crypto` | Thin wrapper over primitives |
| Accounts / blocks / txs | `ivory-core` | Done — types + gas validation |
| State | `ivory-state` | Done — in-memory `StateDB` (trie later) |
| Storage | `ivory-storage` | Done — RocksDB get/put/delete/flush |
| Tx pool / executor / VM | `ivory-txpool`, `ivory-executor`, `ivory-vm` | Stub |
| Consensus / chain / P2P | `ivory-consensus`, `ivory-chain`, `ivory-network` | Stub |
| RPC | `ivory-rpc` | Types + WebSocket skeleton; handlers not wired |
| Node binary | `bin/ivory` | CLI scaffold (`init` / `run`) |

Track work on the [project board](https://github.com/users/armanrasta/projects/6) and [roadmap issue #24](https://github.com/armanrasta/ivory/issues/24).

## Architecture

```
Applications / Orbis
        │
   ivory-rpc          JSON-RPC · WebSocket  (skeleton)
        │
   ivory-chain        Canonical chain · forks  (stub)
   ├── ivory-consensus   PoA  (stub)
   ├── ivory-txpool      Mempool  (stub)
   ├── ivory-executor    Gas + execution  (stub)
   │     └── ivory-vm      WASM (wasmi)  (stub)
   └── ivory-state       Accounts + storage maps
         └── ivory-core  Account · Block · Transaction · Receipt
               └── ivory-primitives
   ivory-network      libp2p gossip · sync  (stub)
   ivory-storage      RocksDB
```

## Quick start

**Requirements:** Rust 1.75+ (edition 2024 toolchain), a C++ compiler (for RocksDB). On GCC 15+, the repo sets `CXXFLAGS=-include cstdint` via [`.cargo/config.toml`](.cargo/config.toml).

```bash
git clone https://github.com/armanrasta/ivory.git
cd ivory
cargo build
cargo test -p ivory-primitives -p ivory-core -p ivory-state -p ivory-storage
```

Run the node scaffold (prints only; no full node yet):

```bash
cargo run -p ivory -- init
cargo run -p ivory -- run
```

## Workspace layout

```
crates/
  ivory-primitives/   Fixed hashes, Address, U256, Bytes, Signature
  ivory-core/         Account, Block, Transaction, Receipt
  ivory-state/        In-memory StateDB
  ivory-storage/      RocksDB backend
  ivory-executor/     Transaction execution (stub)
  ivory-vm/           WASM contracts (stub)
  ivory-consensus/    PoA (stub)
  ivory-chain/        Chain store (stub)
  ivory-txpool/       Mempool (stub)
  ivory-network/      P2P (stub)
  ivory-rpc/          JSON-RPC types + Axum/WS skeleton
  ivory-crypto/       Crypto re-exports
bin/ivory/            Node CLI
tools/                CLI helpers, keygen
docs/                 Notes and planning sketches
```

## Roadmap (high level)

1. **Now** — Core types and storage (mostly done)
2. **Next** — Tx pool, executor, PoA, chain, basic P2P
3. **Then** — JSON-RPC handlers, Orbis Python SDK
4. **Later** — Testnet, Merkle trie, light client, audit

Details: [issues](https://github.com/armanrasta/ivory/issues) · [docs/overview.md](docs/overview.md)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Prefer focused PRs against open issues.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
