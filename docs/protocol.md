# Protocol domain (frozen for testnet)

This is the #16 freeze: **keep Ed25519**, keep **bincode** on the wire, switch
digests to **keccak256**. RLP remains a possible later migration; it is not
required to launch.

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

Receipts include `logs`. Out-of-fuel traps.

## State root

`StateDB::root_hash()` is still `0x0` until the Merkle-Patricia trie (#22). That
is documented, not a blocker for testnet.

## Forks

The node’s executor state does **not** roll back on reorg. Persistence stores
canonical blocks; replay on start rebuilds state from genesis alloc + txs.
