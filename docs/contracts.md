# Contracts (files, not ad-hoc hex)

Ivory is a **permissioned ledger**: it timestamps records and runs small WASM
programs. It is not a mining or “gold digging” chain.

Contracts that you deploy should live as **YAML manifests** and **WAT/WASM**
under [`contracts/`](../contracts/). The explorer matches on-chain bytecode to
those files by `keccak256(code)`.

## Manifest

```yaml
name: tracker
schema: app.v1
description: Writes a marker into contract storage
source: tracker.wat
```

Inline WAT is allowed via a `wat:` field instead of `source`.

## Deploy

```bash
cargo run -p ivory -- init --data-dir ./ivory-data --role master
cargo run -p ivory -- run --data-dir ./ivory-data
# other terminal
cargo run -p ivory-dev -- new ./my-app
cargo run -p ivory-dev -- deploy ./my-app/contracts/tracker.yaml \
  --chain local --data-dir ./ivory-data
```

`ivory-dev deploy` compiles the package, copies it into the catalog dir, and
submits a CREATE transaction. Open `/ui` on the server to see the named contract.

CREATE still accepts raw bytecode on RPC (tests and tools). Unregistered code
shows as **unregistered** in the explorer until a matching file is on disk.
