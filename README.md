# Ivory

A modular **permissioned ledger** in Rust: Proof-of-Authority, libp2p gossip, JSON-RPC, file-backed WASM contracts, and optional structured payloads in `tx.data`.

Ivory timestamps records and tracks chain data. It is not a mining or gold-digging network. A local node can init a data directory, persist blocks, produce PoA-sealed blocks, gossip over libp2p, admit signed transfers, serve JSON-RPC, and run WASM contracts from YAML/WAT files.

## Why Ivory

| | |
|---|---|
| **Purpose** | Open-source permissioned chain you can run locally or in a small validator set |
| **Stack** | Rust workspace · Tokio · Axum · libp2p · RocksDB · wasmi |
| **Consensus (v1)** | Proof-of-Authority — simple validator set; PoS later if needed |
| **API** | Ethereum-style JSON-RPC over HTTP (WebSocket upgrade stubbed for later `eth_subscribe`) |

## Status

| Layer | Crate | Status |
|-------|--------|--------|
| Primitives | `ivory-primitives` | Done — `H256`, `Address`, `U256`, `Bytes`, `Signature` |
| Crypto | `ivory-crypto` | Done — Ed25519, address = keccak256(pubkey)[12:] |
| Accounts / blocks / txs | `ivory-core` | Done — keccak hashes, quant envelope |
| State | `ivory-state` | Done — in-memory `StateDB` (trie later) |
| Storage | `ivory-storage` | Done — RocksDB get/put/delete/flush |
| Tx pool | `ivory-txpool` | Done — signature check, strict contiguous nonces |
| Executor | `ivory-executor` | Done — transfers, CREATE, gas, WASM |
| VM | `ivory-vm` | Done — wasmi fuel, storage, `emit_log` |
| Consensus | `ivory-consensus` | Done — PoA validator set, Ed25519 seals in `extra_data` |
| Chain | `ivory-chain` | Done — in-memory store, reorgs, block production |
| P2P | `ivory-network` | Done — gossipsub blocks/txs + sync `GetBlock` |
| RPC | `ivory-rpc` | Done — `eth_*` + `ivory_nodeInfo` / `ivory_listContracts`; `GET /ui` |
| Server | `bin/ivory` | Done — persist, master/slave `role`, `init` / `run` |
| Dev | `tools/ivory-dev` | Done — `new` / `deploy` / `status` |
| Client | `sdk/python` | Done — JSON-RPC `ivory-client` |

Track work on the [project board](https://github.com/users/armanrasta/projects/6) and [roadmap issue #24](https://github.com/armanrasta/ivory/issues/24).

## Architecture

```
JSON-RPC clients
        │
   ivory-rpc          JSON-RPC · HTTP  (`eth_chainId`, send raw tx, …)
        │
   ivory-chain        Canonical chain · forks · BlockProducer
   ├── ivory-consensus   PoA (Ed25519 seals)
   ├── ivory-txpool      Mempool (verify + strict nonces)
   ├── ivory-executor    Gas + transfers + CREATE + WASM calls
   │     └── ivory-vm      WASM (wasmi, fuel, logs)
   └── ivory-state       Accounts + storage maps
         └── ivory-core  Account · Block · Transaction · Receipt · QuantEnvelope
               └── ivory-primitives  (keccak256)
   ivory-network      libp2p gossip · sync
   ivory-crypto       Ed25519
   ivory-storage      RocksDB (`data-dir/chain`)
```

## Quick start

**Requirements:** Rust 1.75+ (edition 2024 toolchain), a C++ compiler (for RocksDB). On GCC 15+, the repo sets `CXXFLAGS=-include cstdint` via [`.cargo/config.toml`](.cargo/config.toml).

```bash
git clone https://github.com/armanrasta/ivory.git
cd ivory
cargo build
cargo test -p ivory-primitives -p ivory-crypto -p ivory-core -p ivory-state -p ivory-storage
```

Initialize a data dir and run a local producer (JSON-RPC on `127.0.0.1:8545` by default; explorer at `/ui`):

```bash
cargo run -p ivory -- init
cargo run -p ivory -- run
```

`eth_sendRawTransaction` takes hex-encoded **bincode** of `Transaction` (keccak256 domain; see [docs/protocol.md](docs/protocol.md)). **Server** (`ivory init` / `run`), **dev** (`ivory-dev new` / `deploy`), **client** (`sdk/python`): [docs/products.md](docs/products.md). State roots stay `0x0` until the Merkle trie (#22). Two-node compose: [docs/deploy.md](docs/deploy.md). Explorer: `/ui`.

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
bin/ivory/            Server CLI (`init` / `run`)
tools/ivory-dev/      Project toolchain (`new` / `deploy` / `status`)
sdk/python/           App client (`ivory-client`)
deploy/               Docker entrypoint
docs/                 Architecture, products, RPC, protocol, envelope, deploy
```

## Benchmarks

Hot paths for the ledger substrate (hash, state, pool, execute, pool→execute):

```bash
cargo bench -p ivory-bench
cargo bench -p ivory-bench --bench memory
```

Recorded numbers and how to read them: [docs/benchmarks.md](docs/benchmarks.md).

## Roadmap (high level)

1. **Now** — Durable local/multi-node demo (persist, gossip from RPC, quant envelope, Python client)
2. **Testnet** — Docker compose (#17)
3. **Later** — Merkle trie (#22), coverage/metrics, light client

Details: [issues](https://github.com/armanrasta/ivory/issues) · [docs/overview.md](docs/overview.md) · [docs/architecture.md](docs/architecture.md) · [docs/protocol.md](docs/protocol.md)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Prefer focused PRs against open issues.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
