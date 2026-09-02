# Ivory overview

Ivory is a Rust blockchain workspace aimed at **permissioned ledgers** and, first, an immutable store for **Orbis decision receipts**.

## Design choices (v1)

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Consensus | Proof-of-Authority | Simple validator set; enough for private / consortium nets |
| Storage | RocksDB + in-memory maps | Fast to ship; Merkle-Patricia trie deferred |
| Contracts | WASM via wasmi | Pure Rust interpreter; auditable |
| Networking | libp2p | Rust-first; gossipsub + DHT |
| RPC | JSON-RPC 2.0 (Axum) | Familiar to Ethereum tooling |
| Signatures | Ed25519 | Sign `Transaction::signing_hash` (unsigned fields only) |

## What works today

- **Primitives** — hashes, addresses, `U256`, bytes, Ed25519-shaped signature types
- **Crypto** — `sign` / `verify`, v1 address = last 20 bytes of `blake3(pubkey)` (domain may change in #16)
- **Core** — `Account`, `BlockHeader`, `Transaction` (includes `public_key`), `Receipt`, `Log`; `Transaction::hash` hashes the full signed struct; `signing_hash` omits signature and public key
- **State** — `StateDB` over `HashMap` (no real state root yet)
- **Storage** — `RocksDbBackend` KV API
- **Tx pool** — Ed25519 verify on admit, strict contiguous nonces (`ivory-txpool`)
- **Executor** — transfers, intrinsic gas, refunds; WASM via `ivory-vm` when the recipient has code
- **VM** — wasmi with `env.storage_get` / `env.storage_set` host stubs
- **Consensus** — PoA validator set; each seal is an Ed25519 signature over the header hash with empty `extra_data`
- **Chain** — in-memory `BlockStore`, longest-chain reorgs, `BlockProducer` (`ivory-chain`)
- **Network** — gossipsub topics `ivory/blocks/1`, `ivory/txs/1`; `ivory/sync/1` for `GetBlock`
- **RPC** — `eth_chainId`, `eth_blockNumber`, `eth_getBalance`, `eth_getBlockByNumber` / `ByHash`, `eth_sendRawTransaction`, `eth_getTransactionByHash` (and a few extra getters). Unknown methods return JSON-RPC `-32601`
- **Node** — `ivory init` writes genesis/config/key; `ivory run` wires store + pool + producer + network + HTTP on `:8545`

Header hashing is currently **bincode + blake3** (placeholder until RLP/keccak). `eth_sendRawTransaction` takes hex of bincode `Transaction` until #16.

## Benchmarks

Hot-path Criterion results (hash, state, pool, execute, pipeline): [benchmarks.md](benchmarks.md).

## What is next

See [issue #24](https://github.com/armanrasta/ivory/issues/24): Merkle trie (#22), quant envelope (#27), Orbis SDK (#13), Docker/testnet (#17), protocol hash migration (#16).

## Crate dependency sketch

```
ivory-rpc ──► ivory-chain, ivory-txpool, ivory-state, ivory-crypto
ivory-executor ──► ivory-state, ivory-vm
ivory-chain ──► ivory-consensus, ivory-executor, ivory-txpool
ivory-network ──► ivory-core, libp2p
ivory (bin) ──► chain, consensus, network, rpc, txpool, crypto
```
