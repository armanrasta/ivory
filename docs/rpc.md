# JSON-RPC

HTTP `POST /` with JSON-RPC 2.0. WebSocket upgrade is accepted but `eth_subscribe`
is not implemented.

Default bind: `127.0.0.1:8545` (`rpc_addr` in `config.toml`).

Read-only explorer: `GET /ui` (and `GET /ui/`). Open
`http://127.0.0.1:8545/ui` on the producer and `http://127.0.0.1:8546/ui` on a
follower. The page does not submit transactions.

Probes (no JSON-RPC body; stay open when a token is set):

- `GET /livez` — process up
- `GET /readyz` — RPC bound and genesis/head present

Optional `IVORY_RPC_TOKEN` or `IVORY_RPC_TOKEN_FILE`: `POST /`, WebSocket, and
`/ui` require `Authorization: Bearer …`.

CORS (`IVORY_CORS`): empty = none (Helm default); `*` = permissive (Compose);
otherwise a comma-separated origin allowlist.

## Methods

| Method | Notes |
|--------|--------|
| `eth_chainId` | Configured chain id |
| `eth_blockNumber` | Canonical head |
| `eth_getBalance` | Live state (block tags ignored) |
| `eth_getCode` | Live bytecode |
| `eth_getStorageAt` | Live slot |
| `eth_getBlockByNumber` / `eth_getBlockByHash` | Hash list of txs |
| `eth_getTransactionByHash` | Pool or chain |
| `eth_getTransactionCount` | Live account nonce (block tags ignored) |
| `eth_getTransactionReceipt` | `logs`, `contractAddress` on CREATE |
| `eth_sendRawTransaction` | Hex of **bincode** `Transaction` |
| `ivory_nodeInfo` | Role (`producer` / `follower`), address, chain id, peer id, peers, pending, head, bootstrap |
| `ivory_listContracts` | CREATE addresses, code size, and file catalog (`name` / `schema` / `registered`) |

Unknown methods return `-32601`. Not implemented: `eth_call`, `eth_estimateGas`,
`eth_sendTransaction`.

## Raw transaction

1. Fill unsigned fields (`from` is overwritten from the key).
2. `signing_hash` = keccak256(bincode of unsigned fields).
3. Ed25519-sign that 32-byte hash; set `public_key` and `signature`.
4. Submit `0x` + hex(bincode of the full `Transaction`).

See [protocol.md](protocol.md) and the Python client under `sdk/python`.

Quant envelopes: [quant-envelope.md](quant-envelope.md).
Contracts: [contracts.md](contracts.md).
