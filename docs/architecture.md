# Architecture

```
JSON-RPC clients
        │
   ivory-rpc          JSON-RPC · HTTP  (`eth_*`)
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
and an empty `chain/` RocksDB dir.

`ivory run --data-dir DIR` loads those files, replays persisted blocks into the
in-memory `BlockStore` and executor, then serves JSON-RPC and libp2p.

- **Producer** if the loaded key matches the genesis validator.
- **Follower** otherwise (`bootstrap` points at a validator multiaddr).
- After `eth_sendRawTransaction` admits a tx, the node gossips it.
- New blocks are written to RocksDB. Restart reloads them.

## Sync

Blocks and txs move on gossipsub (`ivory/blocks/1`, `ivory/txs/1`). Missing
parents use `ivory/sync/1` (`GetBlock`). There is no headers-first pipeline.

## Reorgs

Longest-chain (plus hash tie-break) updates the canonical head. Executor state
is **not** rolled back on fork; treat that as a known limitation until a later
slice.
