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

Use `ivory_getHeaderByNumber` when you do not want hashes of txs.

Gossip publishes the **header first**, then the full block, on `ivory/blocks/1`.
`ivory/sync/1` accepts `GetHeader` as well as `GetBlock`. A full node that
hears a new header requests the body; it does not insert a header-only stub
(that would collide when the body arrives).

`ivory-light follow --rpc <url>` walks `ivory_getHeaderByNumber` and checks
`parent_hash`, height + 1, and a non-empty `extraData` seal.

## Persistence

Canonical blocks stay on RocksDB. Patricia account-trie nodes are also written
(`t` + node hash) after import/produce. `archive=false` trims **in-memory**
snapshots to `archive_keep` heights; disk still keeps bodies so `ivory run` can
replay. Non-canonical `BlockStore` snapshots are pruned on head change.
