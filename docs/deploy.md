# Deploy (local / testnet compose)

## Single node

```bash
cargo run -p ivory -- init --data-dir ./ivory-data
cargo run -p ivory -- run --data-dir ./ivory-data
```

JSON-RPC: `http://127.0.0.1:8545`. The genesis alloc funds the validator address
(that key is the local **faucet** for demos).

Data dir layout:

- `config.toml` — `chain_id`, `rpc_addr`, `p2p_listen`, `bootstrap`, `block_interval_ms`
- `genesis.json` — timestamp, gas limit, validator, sealed `extra_data`, `alloc`
- `validator.key` — 32-byte Ed25519 secret (hex)
- `chain/` — RocksDB canonical blocks

## Docker Compose (validator + follower)

From the repo root (needs a C++ toolchain in the image build for RocksDB):

```bash
docker compose up --build
```

- Validator RPC: `http://127.0.0.1:8545`
- Follower shares genesis from the validator via the `shared` volume, then
  bootstraps `/dns4/validator/tcp/9000`.
- First start runs `ivory init` if the data volume is empty. The validator’s
  funded account is the faucet; copy `validator.key` from the volume if you need
  to sign transfers.

See `docker-compose.yml` and `deploy/docker-entrypoint.sh`.

## Monitoring

Prometheus (#21) is not wired yet. Use RPC `eth_blockNumber` as a liveness check
(`curl` in the compose healthcheck).
