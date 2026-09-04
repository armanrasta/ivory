# Ivory overview

Ivory is a Rust workspace for a **permissioned** Proof-of-Authority **ledger**:
local `init`/`run`, gossip, JSON-RPC, file-backed WASM contracts, and optional
structured `tx.data`. It records and tracks chain information. It is not a
mining or speculative “gold digging” network.

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
- **Crypto** — `sign` / `verify`, address = last 20 bytes of `keccak256(ed25519_pubkey)`
- **Core** — `Account`, `BlockHeader`, `Transaction`, `Receipt`, `Log`, `QuantEnvelope`; hashes are `keccak256(bincode(...))`; `signing_hash` omits signature and public key
- **State** — `StateDB` over `HashMap` plus a keccak Patricia `root_hash` (account + storage tries)
- **Storage** — `RocksDbBackend` KV API; node persists canonical blocks under `--data-dir/chain`
- **Tx pool** — Ed25519 verify on admit, strict contiguous nonces (`ivory-txpool`)
- **Executor** — transfers, CREATE, intrinsic gas, refunds; WASM via `ivory-vm` when the recipient has code
- **VM** — wasmi with fuel, `env.storage_get` / `env.storage_set` / `env.emit_log`
- **Consensus** — PoA validator set; each seal is an Ed25519 signature over the header hash with empty `extra_data`
- **Chain** — in-memory `BlockStore`, longest-chain reorgs that roll live state back, `BlockProducer` (`ivory-chain`)
- **Network** — gossipsub topics `ivory/blocks/1`, `ivory/txs/1`; `ivory/sync/1` for `GetBlock`
- **RPC** — `eth_*` subset over HTTP plus `ivory_nodeInfo` / `ivory_listContracts`; `GET /ui` explorer. `eth_sendRawTransaction` gossips after admit. Unknown methods return JSON-RPC `-32601`
- **Server** — `ivory init` / `ivory run` with `role` master|slave; restart reloads RocksDB; two-node smoke in `bin/ivory/tests/two_node.rs`
- **Dev** — `ivory-dev new` / `deploy` / `status` against local or `IVORY_PUBLIC_RPC`
- **Client** — `sdk/python` (`ivory-client`) for apps; URL from constructor / `IVORY_RPC_URL` / `IVORY_PUBLIC_RPC`

Header hashing is **bincode + keccak256**. Wire encoding stays bincode (not RLP). See [protocol.md](protocol.md).

## Docs

- [products.md](products.md) · [architecture.md](architecture.md) · [rpc.md](rpc.md) · [protocol.md](protocol.md) · [quant-envelope.md](quant-envelope.md) · [contracts.md](contracts.md) · [deploy.md](deploy.md)

## What is next

See [issue #24](https://github.com/armanrasta/ivory/issues/24): coverage/metrics, light client. Testnet compose is in-tree (`docker-compose.yml`). Re-init existing data dirs after the state-root seal change.

## Crate dependency sketch

```
ivory-rpc ──► ivory-chain, ivory-txpool, ivory-state, ivory-crypto
ivory-executor ──► ivory-state, ivory-vm
ivory-chain ──► ivory-consensus, ivory-executor, ivory-txpool
ivory-network ──► ivory-core, libp2p
ivory (bin) ──► chain, consensus, network, rpc, txpool, crypto
```
