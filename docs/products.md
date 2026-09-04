# Ivory products

Three pieces, one protocol. A **server** is a ledger node. **Dev** is how you
write and deploy contracts onto a server. The **client** is how other apps
(Django, scripts, …) talk to a server.

```
apps (Django, …) ── ivory-client ──► Ivory server RPC
ivory-dev deploy / status     ──► Ivory server RPC
ivory server (master)  ◄──P2P──  ivory server (slave)
```

Slave means **follow and sync**, not failover. Master **produces** only if
`role = master` and the node key is the genesis validator.

## Server (`ivory`)

People run their own node:

```bash
cargo run -p ivory -- init --data-dir ./ivory-data --role master
cargo run -p ivory -- run --data-dir ./ivory-data
```

Join another server (or hosted P2P) as a slave:

```bash
cargo run -p ivory -- init --data-dir ./slave --role slave \
  --bootstrap /ip4/127.0.0.1/tcp/9000
# copy the master’s genesis.json into ./slave first
cargo run -p ivory -- run --data-dir ./slave
```

`bootstrap = []` is isolated. Non-empty `bootstrap` joins that multiaddr.

Docker Compose is the local stand-in for a small network (`validator` =
master, `follower` = slave). Aliases: `NODE_ROLE=master|slave|validator|follower`.

Helm (`deploy/chart/ivory`) is how you run **your** server on a cluster:
StatefulSets, genesis ConfigMap, optional `validator.key` / RPC token Secrets,
Ingress TLS, NetworkPolicy. See [deploy.md](deploy.md).

Hosted “our chain”: set `IVORY_PUBLIC_RPC` (JSON-RPC) and
`IVORY_PUBLIC_BOOTSTRAP` (libp2p) when that network exists. Nothing is
hard-coded; unset means there is no public endpoint yet.

## Dev (`ivory-dev`)

A **project** (`ivory.toml` + `contracts/`), not a data-dir:

```bash
cargo run -p ivory-dev -- new ./my-app
cargo run -p ivory-dev -- deploy --chain local --data-dir ./ivory-data
cargo run -p ivory-dev -- status --chain local
```

`--chain public` uses `IVORY_PUBLIC_RPC` (error if unset). `--rpc` wins.
Deploy key: `--key`, `--data-dir/validator.key`, `IVORY_DEPLOY_KEY`, or
`ivory.toml` `key`. Dev will not invent a key for the hosted chain.

## Client (`sdk/python`, package `ivory-client`)

JSON-RPC for apps. Import `IvoryClient`; point it at a server URL (self-hosted
or `IVORY_PUBLIC_RPC`). See [../sdk/python/README.md](../sdk/python/README.md).
