# Ivory client

Pure-Python JSON-RPC client for an **Ivory server**. Django and other apps import
this package; they do not run a node. There is no Django package and no
`crates/ivory-sdk-py` PyO3 crate — `import ivory_client` only.

```python
from ivory_client import IvoryClient

# Self-hosted or compose: pass the URL, or set IVORY_RPC_URL.
# Hosted “our chain”: IvoryClient(chain="public", secret_key=...) needs IVORY_PUBLIC_RPC.
with IvoryClient("http://127.0.0.1:8545", sk) as client:
    print(client.chain_id(), client.get_block_number())
```

Default URL order: constructor `rpc_url`, then `chain="public"` → `IVORY_PUBLIC_RPC`,
then `IVORY_RPC_URL`, then `http://127.0.0.1:8545`.

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
        to_hex="0x" + "11" * 20,
    )
    print(txh)
    print(client.get_receipt(txh))
    print(client.estimate_gas({"to": "0x" + "11" * 20, "value": "0x1"}))
```

`submit_decision` reads `eth_getTransactionCount` when `nonce` is omitted.

Django (or any app): `import ivory_client` / add `ivory-client` to dependencies.
There is **no Django package** in this repo. Optional `IVORY_RPC_TOKEN` is sent
as `Authorization: Bearer`. `encode_signed_transfer_hex` hex-encodes a signed
transfer (no ABI encoder). `get_logs` wraps `eth_getLogs`.

## Tests

```bash
pytest sdk/python/tests
```

Envelope spec: [../../docs/quant-envelope.md](../../docs/quant-envelope.md).
Products: [../../docs/products.md](../../docs/products.md).
