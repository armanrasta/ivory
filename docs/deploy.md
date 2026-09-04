# Deploy (local / testnet compose)

## Single node

```bash
cargo run -p ivory -- init --data-dir ./ivory-data
cargo run -p ivory -- run --data-dir ./ivory-data
```

JSON-RPC: `http://127.0.0.1:8545`. Explorer: `http://127.0.0.1:8545/ui`.
The genesis alloc funds the validator address (that key is the local **faucet**
for demos).

Data dir layout:

- `config.toml` — `chain_id`, `rpc_addr`, `p2p_listen`, `bootstrap`, `block_interval_ms`, `role` (`master` / `slave`)
- `genesis.json` — timestamp, gas limit, validator, sealed `extra_data`, `alloc`
- `validator.key` — 32-byte Ed25519 secret (hex)
- `chain/` — RocksDB canonical blocks

## Docker Compose (validator + follower)

From the repo root (needs a C++ toolchain in the image build for RocksDB):

```bash
docker compose up --build
```

- Master (validator) RPC: `http://127.0.0.1:8545` — UI `http://127.0.0.1:8545/ui`
- Slave (follower) RPC: `http://127.0.0.1:8546` — UI `http://127.0.0.1:8546/ui`
- Slave shares genesis from the master via the `shared` volume, then
  bootstraps `/dns4/validator/tcp/9000`.
- The explorer can compare heads: set the follower URL in the Compare field.
- First start runs `ivory init` if the data volume is empty. The validator’s
  funded account is the faucet; copy `validator.key` from the volume if you need
  to sign transfers.

See `docker-compose.yml` and `deploy/docker-entrypoint.sh`.

## Contracts

Deploy from a YAML or WAT/WASM file with **ivory-dev** (not the server binary):

```bash
cargo run -p ivory-dev -- deploy contracts/tracker.yaml --chain local --data-dir ./ivory-data
```

See [contracts.md](contracts.md) and [products.md](products.md).

## Monitoring

Prometheus (#21) is not wired yet. Use RPC `eth_blockNumber` as a liveness check
(`curl` in the compose healthcheck).
