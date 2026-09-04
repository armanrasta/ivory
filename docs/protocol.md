# Protocol domain (frozen for testnet)

This is the #16 freeze: **Ed25519** signatures, **bincode** on the wire, and
**keccak256** digests. ECDSA and RLP are out of scope for this protocol.
Criterion numbers live in [benchmarks.md](benchmarks.md), not here.

## Hashes

| Object | Digest |
|--------|--------|
| `BlockHeader::hash` | `keccak256(bincode(header))` |
| `Transaction::hash` | `keccak256(bincode(tx))` including signature and public key |
| `Transaction::signing_hash` | `keccak256(bincode(unsigned fields))` — no `signature` / `public_key` |
| Account address | last 20 bytes of `keccak256(ed25519_pubkey)` |
| `CREATE` address | last 20 bytes of `keccak256(sender \|\| nonce_be)` where `nonce` is the sender nonce **before** increment |
| `CREATE2` address | last 20 bytes of `keccak256(0xff \|\| sender \|\| salt \|\| code_hash)` |
| EIP-55 | checksum over keccak of the **lowercase hex digits** (no `0x`) |

`eth_sendRawTransaction` and gossip still carry **bincode** `Transaction` /
`Block` (hex on RPC). Changing that encoding is a second breaking change.

## Signatures

Ed25519 over `signing_hash` (32 bytes). Ethereum secp256k1 wallets are **not**
in this freeze.

## CREATE

`to: None` derives the contract address from `tx.from` and `tx.nonce`, credits
the endowment, installs `tx.data` as **runtime** bytecode (no constructor), and
sets `Account.code_hash` to `keccak256(code)`. Receipts expose `contractAddress`.

## VM

wasmi fuel is the remaining gas after intrinsic. Host imports:

- `env.storage_get` / `env.storage_set`
- `env.emit_log` (i32 topic → one log)
- `env.calldata_len` / `env.calldata_at` (`tx.data` / `eth_call` data; existing
  `call () -> i32` contracts keep working; no Solidity ABI encoder)

Receipts include `logs`. Out-of-fuel traps.

## State root

`StateDB::root_hash()` is a keccak hexary Patricia trie over accounts
(`keccak256(bincode(node))`, not RLP). Account leaves are `bincode(Account)`;
each contract’s `storage_root` is a Patricia trie of non-zero slots
(32-byte key, 32-byte `U256`). An empty account trie has a documented
non-zero `empty_root`. Empty contract storage stays `0x0` so `Account::is_empty`
still holds.

Genesis headers seal the alloc root and the empty-list transaction/receipt
roots. Existing data dirs must be re-initialized (`ivory init`) after this
header change.

## Transaction and receipt roots

`transactions_root` and `receipts_root` are `list_root` =
`keccak256(bincode(items))`. They are **not** Merkle-Patricia tries and are
**not** Ethereum RLP list roots.

An empty list is the keccak of bincode’s empty `Vec` (length prefix, no
elements). That hash is **not** `0x0`. Empty transaction and receipt lists
share the same empty-`Vec` encoding, so their empty roots are equal. A header
that still commits `0x0` for either root is invalid.

## Forks

Import executes each block on a fork of the **parent** snapshot and rejects a
mismatched `state_root`. If the canonical head moves, live executor/RPC state
is reset from the new-head snapshot. Dropped-fork transactions return to the
pool. Persistence still stores canonical blocks only; restart replays genesis
alloc plus the canonical path.

## Light header chain

A light client can follow **hash-linked headers + PoA seals** without bodies:

- `parent_hash` links the header chain
- `extra_data` holds the Ed25519 seal over the header hash
- `state_root` / `transactions_root` / `receipts_root` commit to state and lists
- Bodies are optional; `ivory_getHeaderByNumber` returns header fields only

`eth_getBlockByNumber` still returns the full block (tx hashes) when the
node has that body. Patricia account-trie nodes are written under RocksDB
keys `t` + hash (`ChainPersist::persist_trie_nodes`). `eth_getProof` walks
those nodes (and the storage trie) on live or `state_at` snapshots.

Non-canonical `BlockStore` snapshots are pruned on head movement.

## Archive

`config.toml` `archive = true` (default) keeps every body and snapshot.
`archive = false` plus `archive_keep` drops non-canonical snapshots and
in-memory bodies below `head - archive_keep + 1`. Disk still stores full
canonical blocks so restart can replay, then apply the same window.
