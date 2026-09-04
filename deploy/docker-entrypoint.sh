#!/bin/sh
set -eu

DATA_DIR="${DATA_DIR:-/data}"
RAW_ROLE="${NODE_ROLE:-master}"
RPC_ADDR="${RPC_ADDR:-0.0.0.0:8545}"
CHAIN_ID="${CHAIN_ID:-1}"
BLOCK_INTERVAL_MS="${BLOCK_INTERVAL_MS:-2000}"
BOOTSTRAP="${BOOTSTRAP:-${IVORY_PUBLIC_BOOTSTRAP:-}}"

mkdir -p "$DATA_DIR"

case "$(echo "$RAW_ROLE" | tr '[:upper:]' '[:lower:]')" in
  slave|follower)
    ROLE=slave
    ;;
  *)
    ROLE=master
    ;;
esac

if [ -z "${P2P_LISTEN:-}" ]; then
  if [ "$ROLE" = "master" ]; then
    P2P_LISTEN="/ip4/0.0.0.0/tcp/9000"
  else
    P2P_LISTEN="/ip4/0.0.0.0/tcp/0"
  fi
fi

bootstrap_toml() {
  if [ -z "$BOOTSTRAP" ]; then
    printf '%s' '[]'
  else
    printf '["%s"]' "$BOOTSTRAP"
  fi
}

write_config() {
  cat > "$DATA_DIR/config.toml" <<EOF
chain_id = ${CHAIN_ID}
rpc_addr = "${RPC_ADDR}"
p2p_listen = "${P2P_LISTEN}"
bootstrap = $(bootstrap_toml)
block_interval_ms = ${BLOCK_INTERVAL_MS}
contracts_dir = ""
role = "${ROLE}"
EOF
}

upsert_toml() {
  key="$1"
  value="$2"
  file="$DATA_DIR/config.toml"
  if grep -q "^${key} =" "$file"; then
    tmp="${file}.tmp.$$"
    awk -v k="$key" -v v="$value" '
      index($0, k " =") == 1 { print k " = " v; next }
      { print }
    ' "$file" > "$tmp"
    mv "$tmp" "$file"
  else
    printf '%s = %s\n' "$key" "$value" >> "$file"
  fi
}

install_overlays() {
  if [ -n "${VALIDATOR_KEY_FILE:-}" ] && [ -f "$VALIDATOR_KEY_FILE" ]; then
    cp "$VALIDATOR_KEY_FILE" "$DATA_DIR/validator.key"
  fi
  if [ -n "${GENESIS_FILE:-}" ] && [ -f "$GENESIS_FILE" ]; then
    cp "$GENESIS_FILE" "$DATA_DIR/genesis.json"
  elif [ "$ROLE" = "slave" ]; then
    if [ -f /shared/genesis.json ]; then
      cp /shared/genesis.json "$DATA_DIR/genesis.json"
    else
      echo "waiting for shared genesis..."
      i=0
      while [ ! -f /shared/genesis.json ]; do
        i=$((i + 1))
        if [ "$i" -gt 60 ]; then
          echo "timeout waiting for genesis (set GENESIS_FILE or mount /shared)" >&2
          exit 1
        fi
        sleep 1
      done
      cp /shared/genesis.json "$DATA_DIR/genesis.json"
    fi
  fi
}

if [ ! -f "$DATA_DIR/config.toml" ]; then
  if [ -n "${VALIDATOR_KEY_FILE:-}" ] && [ -f "${VALIDATOR_KEY_FILE}" ] \
    && [ -n "${GENESIS_FILE:-}" ] && [ -f "${GENESIS_FILE}" ]; then
    mkdir -p "$DATA_DIR/chain" "$DATA_DIR/contracts"
    cp "$VALIDATOR_KEY_FILE" "$DATA_DIR/validator.key"
    cp "$GENESIS_FILE" "$DATA_DIR/genesis.json"
    write_config
  else
    if [ "$ROLE" = "slave" ]; then
      if [ -n "$BOOTSTRAP" ]; then
        ivory init --data-dir "$DATA_DIR" --role slave --bootstrap "$BOOTSTRAP"
      else
        ivory init --data-dir "$DATA_DIR" --role slave
      fi
    else
      ivory init --data-dir "$DATA_DIR" --role master
    fi
    install_overlays
    write_config
  fi
else
  install_overlays
  upsert_toml rpc_addr "\"${RPC_ADDR}\""
  upsert_toml p2p_listen "\"${P2P_LISTEN}\""
  upsert_toml role "\"${ROLE}\""
  upsert_toml bootstrap "$(bootstrap_toml)"
fi

if [ "$ROLE" = "master" ] && [ -d /shared ] && [ -w /shared ]; then
  cp "$DATA_DIR/genesis.json" /shared/genesis.json
fi

exec ivory run --data-dir "$DATA_DIR"
