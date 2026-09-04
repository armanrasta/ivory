# Deploy

Compose is the laptop stand-in: two containers, a shared genesis volume, open
CORS so `/ui` works in a browser. **Helm** is how you run **your** server on a
cluster. Hosted “our chain” stays env-only (`IVORY_PUBLIC_RPC` /
`IVORY_PUBLIC_BOOTSTRAP`); nothing in the chart hard-codes a public hostname.

Slaves **follow and sync**. They are not failover masters.

## Single node (laptop)

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
- `validator.key` — 32-byte Ed25519 secret (hex). Never commit it.
- `chain/` — RocksDB canonical blocks

## Docker Compose (validator + follower)

From the repo root (needs a C++ toolchain in the image build for RocksDB):

```bash
docker compose up --build
```

- Master (validator) RPC: `http://127.0.0.1:8545` — UI `http://127.0.0.1:8545/ui`
- Slave (follower) RPC: `http://127.0.0.1:8546` — UI `http://127.0.0.1:8546/ui`
- Compose sets `IVORY_CORS=*` so the explorer can call RPC from the browser.
- Slave waits on `/shared/genesis.json` (compose fallback). Kubernetes uses
  `GENESIS_FILE` / a ConfigMap instead.
- First start runs `ivory init` if the data volume is empty. Copy
  `validator.key` from the volume if you need to sign transfers.
- Healthcheck is `GET /livez` (process up). `GET /readyz` is genesis/head.

The image runs as uid `65532`. See `docker-compose.yml` and
`deploy/docker-entrypoint.sh`.

Entrypoint env (writes `config.toml`; no `sed`):

| Variable | Role |
|----------|------|
| `NODE_ROLE` | `master` / `slave` (aliases `validator` / `follower`) |
| `RPC_ADDR` | JSON-RPC bind (`0.0.0.0:8545` in containers) |
| `P2P_LISTEN` | libp2p multiaddr |
| `BOOTSTRAP` / `IVORY_PUBLIC_BOOTSTRAP` | slave dial target |
| `GENESIS_FILE` | copy into `$DATA_DIR/genesis.json` |
| `VALIDATOR_KEY_FILE` | install as `validator.key` (do not rotate the master key on an empty PVC when a Secret is mounted) |
| `IVORY_RPC_TOKEN` / `IVORY_RPC_TOKEN_FILE` | optional bearer on `POST /` and `/ui` |
| `IVORY_CORS` | empty = none; `*` = permissive; else comma-separated origins |

## Helm (your cluster)

Chart: `deploy/chart/ivory`. Restricted Pod Security: non-root `65532`, drop all
capabilities, read-only root FS, `RuntimeDefault` seccomp. No privileged, no
hostPath.

```bash
# 1. First install: master only (empty PVC → ivory init, key on the volume)
helm install ivory deploy/chart/ivory \
  --set image.repository=ivory --set image.tag=latest

# 2. Copy genesis off the master
kubectl exec ivory-master-0 -c ivory -- cat /data/genesis.json > genesis.json

# 3. Slaves + optional token / ingress host you actually own
helm upgrade ivory deploy/chart/ivory \
  --set-file genesis=genesis.json \
  --set slave.replicaCount=1 \
  --set rpc.token="$(openssl rand -hex 16)" \
  --set ingress.enabled=true \
  --set ingress.host=rpc.example.internal \
  --set ingress.certManagerIssuer=letsencrypt-prod
```

Optional master key Secret (skips generating a new key on first boot):

```bash
helm upgrade ivory deploy/chart/ivory --set-file validatorKey=./validator.key
```

Never commit `validator.key`. Rotate `rpc.token` with `helm upgrade`. After a
state-root / seal change, re-init data dirs and refresh the genesis ConfigMap
the same way (`--set-file genesis=`).

| Object | Purpose |
|--------|---------|
| StatefulSet `master` | replicas 1, PVC `/data`, `NODE_ROLE=master` |
| StatefulSet `slave` | PVC, `GENESIS_FILE=/genesis/genesis.json`, bootstrap `/dns4/<master-0>.<headless>.<ns>.svc.cluster.local/tcp/9000` |
| Service ClusterIP | RPC `:8545` |
| Service headless | P2P `:9000` (stable DNS) |
| ConfigMap | `genesis.json` (required when `slave.replicaCount > 0`) |
| Secret | optional `validator.key` + `rpc-token` |
| Ingress | TLS at the ingress (cert-manager annotation); HTTP to the RPC Service |
| NetworkPolicy | 8545 from ingress-nginx / `ivory.io/rpc-client` / kube-system; 9000 only chart pods |
| PDB | slaves, `minAvailable: 1` when replicas > 1 |

Probes: HTTP `GET /livez` and `GET /readyz` on 8545 (kubelet does not send a
bearer). If NetworkPolicy drops node traffic, set `networkPolicy.kubeletCidrs`.

Build and load the image into the cluster yourself (`docker build` / `kind load`
/ your registry). The chart does not assume a public image or a public RPC DNS
name.

## Contracts

Deploy from a YAML or WAT/WASM file with **ivory-dev** (not the server binary):

```bash
cargo run -p ivory-dev -- deploy contracts/tracker.yaml --chain local --data-dir ./ivory-data
```

See [contracts.md](contracts.md) and [products.md](products.md).

## Monitoring

Prometheus / ServiceMonitor is not in this chart (#21). Use `GET /livez` and
`GET /readyz` first; do not POST `eth_blockNumber` as a probe.
