#!/bin/sh
set -eu

DATA_DIR="${DATA_DIR:-/data}"
RAW_ROLE="${NODE_ROLE:-master}"
BOOTSTRAP="${BOOTSTRAP:-${IVORY_PUBLIC_BOOTSTRAP:-/dns4/validator/tcp/9000}}"
mkdir -p "$DATA_DIR"

case "$(echo "$RAW_ROLE" | tr '[:upper:]' '[:lower:]')" in
  slave|follower)
    ROLE=slave
    ;;
  *)
    ROLE=master
    ;;
esac

if [ ! -f "$DATA_DIR/config.toml" ]; then
  if [ "$ROLE" = "slave" ]; then
    ivory init --data-dir "$DATA_DIR" --role slave --bootstrap "$BOOTSTRAP"
  else
    ivory init --data-dir "$DATA_DIR" --role master
  fi
fi

# Bind RPC on all interfaces. TOML from `ivory init` uses 127.0.0.1:8545.
sed -i 's/rpc_addr = .*/rpc_addr = "0.0.0.0:8545"/' "$DATA_DIR/config.toml"

if grep -q '^role' "$DATA_DIR/config.toml"; then
  sed -i "s|^role = .*|role = \"${ROLE}\"|" "$DATA_DIR/config.toml"
else
  printf '\nrole = "%s"\n' "$ROLE" >> "$DATA_DIR/config.toml"
fi

if [ "$ROLE" = "master" ]; then
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
