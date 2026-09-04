# Light header chain

A light peer stores **hash-linked headers** and checks PoA seals. It does not
need transaction bodies or receipts.

## What is committed

Each header includes `parent_hash`, `number`, `state_root`, `transactions_root`,
`receipts_root`, and `extra_data` (Ed25519 seal). `difficulty` stays `0` (PoA).

A header-only node:

1. Starts from genesis.
2. Accepts a header if `parent_hash` matches the previous header hash, the
   height increments by one, and the PoA seal verifies.
3. Ignores bodies until it asks for them.

## RPC

| Method | Body |
|--------|------|
| `ivory_getHeaderByNumber` | Header fields only |
| `eth_getBlockByNumber` | Same header plus a **transaction hash list** |

Use `ivory_getHeaderByNumber` when you do not want hashes of txs. There is no
headers-first sync protocol yet; gossip still carries full blocks.

## Persistence

Canonical blocks stay on RocksDB. Patricia account-trie nodes are also written
(`t` + node hash) after import/produce. `archive=false` trims **in-memory**
snapshots to `archive_keep` heights; disk still keeps bodies so `ivory run` can
replay. Non-canonical `BlockStore` snapshots are pruned on head change.
