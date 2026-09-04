# Architecture

```
JSON-RPC clients
        │
   ivory-rpc          JSON-RPC · HTTP  (`eth_*`, `ivory_nodeInfo`) · `GET /ui`
        │
   ivory-chain        Canonical chain · forks · BlockProducer
   ├── ivory-consensus   PoA (Ed25519 seals)
   ├── ivory-txpool      Mempool (verify + strict nonces)
   ├── ivory-executor    Gas + transfers + CREATE + WASM calls
   │     └── ivory-vm      WASM (wasmi, fuel, logs)
   └── ivory-state       Accounts + storage maps
         └── ivory-core  Account · Block · Transaction · Receipt · QuantEnvelope
               └── ivory-primitives  (keccak256, Address, U256, …)
   ivory-network      libp2p gossip · sync
   ivory-crypto       Ed25519
   ivory-storage      RocksDB (`--data-dir/chain`)
```

## Process

`ivory init --data-dir DIR` writes `config.toml`, `genesis.json`, `validator.key`,
an empty `chain/` RocksDB dir, and `contracts/` for YAML/WAT packages.

`ivory run --data-dir DIR` loads those files, replays persisted blocks into the
in-memory `BlockStore` and executor, then serves JSON-RPC and libp2p.

- **Producer** if the loaded key matches the genesis validator.
- **Follower** otherwise (`bootstrap` points at a validator multiaddr).
- After `eth_sendRawTransaction` admits a tx, the node gossips it.
- New blocks are written to RocksDB. Restart reloads them.
- `GET /ui` serves the read-only ledger explorer (blocks, file-backed contracts, producer vs follower stats).
- `ivory-dev deploy` compiles a contract file and CREATE-submits it onto a server.

## Sync

Blocks and txs move on gossipsub (`ivory/blocks/1`, `ivory/txs/1`). Missing
parents use `ivory/sync/1` (`GetBlock`). There is no headers-first pipeline.

## Reorgs

Longest-chain (plus hash tie-break) updates the canonical head. Live executor
and RPC state follow the new head: `import_and_apply` verifies `state_root` on
a parent snapshot, then `reset_from` the winning post-state. Snapshots are
keyed by **block hash**. Persistence stores blocks only; restart replays the
canonical path from genesis alloc.
