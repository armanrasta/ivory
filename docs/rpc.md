# JSON-RPC

HTTP `POST /` with JSON-RPC 2.0. WebSocket upgrade supports `eth_subscribe`
(`newHeads`, `newPendingTransactions`, `logs`). HTTP subscribe returns an error.

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
| `eth_call` | Fork-and-simulate on live state (`latest`/`pending`) or `state_at` for other tags. Returns `0x` + hex of a 32-byte big-endian WASM `i32`, or `0x` for EOA/CREATE. WASM `data` is `env.calldata_*` (no Solidity ABI). |
| `eth_estimateGas` | Same simulation; returns intrinsic + VM fuel (no binary search) |
| `eth_getLogs` | Receipt scan by `fromBlock`/`toBlock`/`address`. Errors if the range exceeds 1000 blocks. No bloom. |
| `eth_getProof` | Not served: nodes are written to RocksDB (`t` + hash) but a proof walk is not exported yet. |
| `eth_subscribe` | WebSocket only: `newHeads`, `newPendingTransactions`, `logs` (optional address filter) |
| `eth_unsubscribe` | WebSocket only |
| `ivory_nodeInfo` | Role (`producer` / `follower`), address, chain id, peer id, peers, pending, head, bootstrap |
| `ivory_listContracts` | CREATE addresses, code size, and file catalog (`name` / `schema` / `registered`) |
| `ivory_getHeaderByNumber` | Header fields only (no `transactions`). Light clients should use this instead of `eth_getBlockByNumber`. |

Unknown methods return `-32601`. Not implemented: `eth_sendTransaction`,
`eth_feeHistory` / 1559 fields.

`GET /metrics` is unauthenticated like `/livez` / `/readyz` (Prometheus text).
Restrict scrape with NetworkPolicy label `ivory.io/metrics-client`.
`ivory_nodeInfo.peerId` is the libp2p id to put in `p2p_allowlist` / Helm
`p2p.allowPeerIds`. Header-only reads: [light.md](light.md).

## Raw transaction

1. Fill unsigned fields (`from` is overwritten from the key).
2. `signing_hash` = keccak256(bincode of unsigned fields).
3. Ed25519-sign that 32-byte hash; set `public_key` and `signature`.
4. Submit `0x` + hex(bincode of the full `Transaction`).

See [protocol.md](protocol.md) and the Python client under `sdk/python`.

Quant envelopes: [quant-envelope.md](quant-envelope.md).
Contracts: [contracts.md](contracts.md).
