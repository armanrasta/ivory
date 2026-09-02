# Ivory

A modular blockchain in Rust — built as an immutable **decision ledger** for platforms like [Orbis](https://github.com/armanrasta), and usable as a general permissioned chain.

Ivory is early. A local node can init a data directory, produce PoA-sealed blocks, gossip over libp2p, admit signed transfers, serve JSON-RPC, and run WASM contracts through wasmi.

## Why Ivory

| | |
|---|---|
| **Purpose** | Timestamp and prove quantitative decisions (AHP, supply-chain opts, quality metrics) on-chain |
| **Stack** | Rust workspace · Tokio · Axum · libp2p · RocksDB · wasmi |
| **Consensus (v1)** | Proof-of-Authority — simple, permissioned; PoS later if needed |
| **API** | Ethereum-style JSON-RPC over HTTP (WebSocket upgrade stubbed for later `eth_subscribe`) |

Competitors often store analytics in a database. Ivory aims to make those decisions auditable and blockchain-verified.

## Status

| Layer | Crate | Status |
|-------|--------|--------|
| Primitives | `ivory-primitives` | Done — `H256`, `Address`, `U256`, `Bytes`, `Signature` |
| Crypto | `ivory-crypto` | Done — Ed25519 sign/verify, v1 address from pubkey |
| Accounts / blocks / txs | `ivory-core` | Done — types + gas validation + `hash` / `signing_hash` |
| State | `ivory-state` | Done — in-memory `StateDB` (trie later) |
| Storage | `ivory-storage` | Done — RocksDB get/put/delete/flush |
| Tx pool | `ivory-txpool` | Done — signature check, strict contiguous nonces |
| Executor | `ivory-executor` | Done — transfers, gas, WASM dispatch via wasmi |
| VM | `ivory-vm` | Done — wasmi + `env.storage_get` / `env.storage_set` |
| Consensus | `ivory-consensus` | Done — PoA validator set, Ed25519 seals in `extra_data` |
| Chain | `ivory-chain` | Done — in-memory store, reorgs, block production |
| P2P | `ivory-network` | Done — gossipsub blocks/txs + sync `GetBlock` |
| RPC | `ivory-rpc` | Done — MVP `eth_*` over HTTP (`POST /`) |
| Node binary | `bin/ivory` | Done — `init` / `run` (store, pool, producer, network, RPC) |

Track work on the [project board](https://github.com/users/armanrasta/projects/6) and [roadmap issue #24](https://github.com/armanrasta/ivory/issues/24).

## Architecture

```
Applications / Orbis
        │
   ivory-rpc          JSON-RPC · HTTP  (`eth_chainId`, send raw tx, …)
        │
   ivory-chain        Canonical chain · forks · BlockProducer
   ├── ivory-consensus   PoA (Ed25519 seals)
   ├── ivory-txpool      Mempool (verify + strict nonces)
   ├── ivory-executor    Gas + transfers + WASM calls
   │     └── ivory-vm      WASM (wasmi)
   └── ivory-state       Accounts + storage maps
         └── ivory-core  Account · Block · Transaction · Receipt
               └── ivory-primitives
   ivory-network      libp2p gossip · sync
   ivory-crypto       Ed25519
   ivory-storage      RocksDB
```

## Quick start

**Requirements:** Rust 1.75+ (edition 2024 toolchain), a C++ compiler (for RocksDB). On GCC 15+, the repo sets `CXXFLAGS=-include cstdint` via [`.cargo/config.toml`](.cargo/config.toml).

```bash
git clone https://github.com/armanrasta/ivory.git
cd ivory
cargo build
cargo test -p ivory-primitives -p ivory-crypto -p ivory-core -p ivory-state -p ivory-storage
```

Initialize a data dir and run a local validator (JSON-RPC on `127.0.0.1:8545` by default):

```bash
cargo run -p ivory -- init
cargo run -p ivory -- run
```

`eth_sendRawTransaction` currently takes hex-encoded **bincode** of `Transaction` (placeholder until #16 RLP). State roots stay `0x0` until the Merkle trie (#22).

## Workspace layout

```
crates/
  ivory-primitives/   Fixed hashes, Address, U256, Bytes, Signature
  ivory-core/         Account, Block, Transaction, Receipt
  ivory-state/        In-memory StateDB
  ivory-storage/      RocksDB backend
  ivory-executor/     Transfers + gas metering + WASM dispatch
  ivory-vm/           WASM contracts (wasmi)
  ivory-consensus/    PoA (validator set, Ed25519 seals)
  ivory-chain/        Canonical store + BlockProducer
  ivory-txpool/       Mempool (strict nonces + signature check)
  ivory-network/      libp2p gossipsub + sync scaffold
  ivory-rpc/          JSON-RPC handlers + Axum HTTP
  ivory-crypto/       Ed25519 sign/verify
bin/ivory/            Node CLI (`init` / `run`)
tools/                CLI helpers, keygen, Criterion benches (`ivory-bench`)
docs/                 Notes and planning sketches
```

## Benchmarks

Hot paths for the ledger substrate (hash, state, pool, execute, pool→execute):

```bash
cargo bench -p ivory-bench
cargo bench -p ivory-bench --bench memory
```

Recorded numbers and how to read them: [docs/benchmarks.md](docs/benchmarks.md).

## Roadmap (high level)

1. **Now** — Local node MVP (crypto, gossip, RPC, WASM) — this tree
2. **Next** — Quant envelope (#27), Orbis Python SDK (#13)
3. **Later** — Testnet/Docker (#17), Merkle trie (#22), protocol hash migration (#16)

Details: [issues](https://github.com/armanrasta/ivory/issues) · [docs/overview.md](docs/overview.md) · [docs/benchmarks.md](docs/benchmarks.md)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Prefer focused PRs against open issues.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
