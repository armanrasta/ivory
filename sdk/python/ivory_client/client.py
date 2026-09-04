"""HTTP JSON-RPC client for an Ivory server."""

from __future__ import annotations

import os
from typing import Any

import httpx

from ivory_client.codec import decode_envelope, encode_envelope, encode_signed_tx


def resolve_rpc_url(rpc_url: str | None = None, chain: str | None = None) -> str:
    """Pick a server URL: explicit, `public` (`IVORY_PUBLIC_RPC`), `IVORY_RPC_URL`, or local."""
    if rpc_url:
        return rpc_url.rstrip("/")
    if chain == "public":
        public = os.environ.get("IVORY_PUBLIC_RPC", "").strip()
        if not public:
            raise ValueError("IVORY_PUBLIC_RPC is unset (required for chain='public')")
        return public.rstrip("/")
    env = os.environ.get("IVORY_RPC_URL", "").strip()
    if env:
        return env.rstrip("/")
    return "http://127.0.0.1:8545"


class IvoryClient:
    """JSON-RPC client: balances, blocks, quant envelopes, receipts."""

    def __init__(
        self,
        rpc_url: str | None = None,
        secret_key: bytes = b"",
        chain_id: int | None = None,
        timeout: float = 10.0,
        chain: str | None = None,
    ) -> None:
        if not secret_key:
            raise ValueError("secret_key is required")
        self.rpc_url = resolve_rpc_url(rpc_url, chain)
        self.secret_key = secret_key
        self._chain_id = chain_id
        self._http = httpx.Client(timeout=timeout)

    def close(self) -> None:
        self._http.close()

    def __enter__(self) -> IvoryClient:
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()

    def _rpc(self, method: str, params: list[Any] | None = None) -> Any:
        body = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params or [],
        }
        response = self._http.post(self.rpc_url, json=body)
        response.raise_for_status()
        payload = response.json()
        if "error" in payload and payload["error"]:
            raise RuntimeError(payload["error"])
        return payload.get("result")

    def chain_id(self) -> int:
        if self._chain_id is not None:
            return self._chain_id
        raw = self._rpc("eth_chainId")
        return int(raw, 16)

    def get_balance(self, address_hex: str) -> int:
        raw = self._rpc("eth_getBalance", [address_hex, "latest"])
        return int(raw, 16)

    def get_block_number(self) -> int:
        raw = self._rpc("eth_blockNumber")
        return int(raw, 16)

    def get_nonce(self, address_hex: str) -> int:
        raw = self._rpc("eth_getTransactionCount", [address_hex, "latest"])
        return int(raw, 16)

    def submit_decision(
        self,
        decision_id: str,
        schema: str,
        metrics: list[tuple[str, str]],
        *,
        to_hex: str | None = None,
        value: int = 0,
        nonce: int | None = None,
        gas: int | None = None,
        gas_price: int = 1,
        content_hash: bytes | None = None,
        cid: str | None = None,
    ) -> str:
        """Sign and submit a quant envelope. Returns tx hash hex."""
        data = encode_envelope(
            decision_id, schema, metrics, content_hash=content_hash, cid=cid
        )
        from ivory_client.codec import address_from_pubkey
        from nacl.signing import SigningKey

        pk = SigningKey(self.secret_key).verify_key.encode()
        sender = "0x" + address_from_pubkey(pk).hex()
        if nonce is None:
            nonce = self.get_nonce(sender)
        if gas is None:
            gas = 21_000 + 16 * len(data)
        to = None if to_hex is None else bytes.fromhex(to_hex.removeprefix("0x"))
        tx_hash, payload = encode_signed_tx(
            self.secret_key, to, nonce, value, gas, gas_price, data
        )
        raw = "0x" + payload.hex()
        result = self._rpc("eth_sendRawTransaction", [raw])
        expected = "0x" + tx_hash.hex()
        if result.lower() != expected.lower():
            return result
        return expected

    def get_receipt(self, tx_hash: str) -> dict[str, Any] | None:
        try:
            return self._rpc("eth_getTransactionReceipt", [tx_hash])
        except RuntimeError:
            return None

    def get_transaction(self, tx_hash: str) -> dict[str, Any]:
        return self._rpc("eth_getTransactionByHash", [tx_hash])

    def decode_tx_input(self, tx: dict[str, Any]) -> dict:
        raw = tx["input"]
        data = bytes.fromhex(raw.removeprefix("0x"))
        return decode_envelope(data)
