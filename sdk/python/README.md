# Ivory Python client

Pure-Python JSON-RPC client for a running `ivory` node. No PyO3; it talks HTTP
`eth_*` and signs Ed25519 over `keccak256(bincode(unsigned tx))`.

## Install

```bash
cd sdk/python
pip install -e ".[dev]"
```

## Usage

```python
from pathlib import Path
from ivory_client import IvoryClient

sk = bytes.fromhex(Path("ivory-data/validator.key").read_text().strip().removeprefix("0x"))
with IvoryClient("http://127.0.0.1:8545", sk) as client:
    print(client.chain_id(), client.get_block_number())
    txh = client.submit_decision(
        "dec-1",
        "app.v1",
        [("score", "0.82")],
        to_hex="0x" + "11" * 20,  # any recipient; envelope is in data
    )
    print(txh)
    print(client.get_receipt(txh))
```

`submit_decision` reads `eth_getTransactionCount` when `nonce` is omitted.
Track nonce yourself if you fire many txs before inclusion.

## Tests

```bash
pytest sdk/python/tests
```

Unit tests mock HTTP. Against a live node (needs Phase 1 datadir):

```bash
cargo run -p ivory -- init --data-dir /tmp/ivory-py
cargo run -p ivory -- run --data-dir /tmp/ivory-py
```

Envelope spec: [../../docs/quant-envelope.md](../../docs/quant-envelope.md).
