#!/bin/sh
set -eu

DATA_DIR="${DATA_DIR:-/data}"
ROLE="${NODE_ROLE:-validator}"
BOOTSTRAP="${BOOTSTRAP:-/dns4/validator/tcp/9000}"
mkdir -p "$DATA_DIR"

if [ ! -f "$DATA_DIR/config.toml" ]; then
  ivory init --data-dir "$DATA_DIR"
fi

# Bind RPC on all interfaces. TOML from `ivory init` uses 127.0.0.1:8545.
sed -i 's/rpc_addr = .*/rpc_addr = "0.0.0.0:8545"/' "$DATA_DIR/config.toml"

if [ "$ROLE" = "validator" ]; then
  mkdir -p /shared
  sed -i 's|p2p_listen = .*|p2p_listen = "/ip4/0.0.0.0/tcp/9000"|' "$DATA_DIR/config.toml"
  cp "$DATA_DIR/genesis.json" /shared/genesis.json
else
  echo "waiting for shared genesis..."
  i=0
  while [ ! -f /shared/genesis.json ]; do
    i=$((i + 1))
    if [ "$i" -gt 60 ]; then
      echo "timeout waiting for genesis" >&2
      exit 1
    fi
    sleep 1
  done
  cp /shared/genesis.json "$DATA_DIR/genesis.json"
  sed -i 's|p2p_listen = .*|p2p_listen = "/ip4/0.0.0.0/tcp/0"|' "$DATA_DIR/config.toml"
  if grep -q '^bootstrap' "$DATA_DIR/config.toml"; then
    sed -i "s|^bootstrap = .*|bootstrap = [\"${BOOTSTRAP}\"]|" "$DATA_DIR/config.toml"
  else
    printf '\nbootstrap = ["%s"]\n' "$BOOTSTRAP" >> "$DATA_DIR/config.toml"
  fi
fi

exec ivory run --data-dir "$DATA_DIR"
