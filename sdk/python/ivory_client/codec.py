"""Bincode helpers matching Ivory's serde + bincode 1.x (little-endian, fixint)."""

from __future__ import annotations

import struct

from Crypto.Hash import keccak
from nacl.signing import SigningKey

QUANT_MAGIC = b"IQNT"
QUANT_VERSION = 1


def u8(n: int) -> bytes:
    return bytes([n])


def u16(n: int) -> bytes:
    return struct.pack("<H", n)


def u64(n: int) -> bytes:
    return struct.pack("<Q", n)


def encode_str(s: str) -> bytes:
    data = s.encode("utf-8")
    return u64(len(data)) + data


def encode_bytes(data: bytes) -> bytes:
    return u64(len(data)) + data


def encode_option_bytes(data: bytes | None) -> bytes:
    if data is None:
        return u8(0)
    return u8(1) + data


def keccak256(data: bytes) -> bytes:
    h = keccak.new(digest_bits=256)
    h.update(data)
    return h.digest()


def address_from_pubkey(pk: bytes) -> bytes:
    return keccak256(pk)[12:]


def encode_envelope(
    decision_id: str,
    schema: str,
    metrics: list[tuple[str, str]],
    content_hash: bytes | None = None,
    cid: str | None = None,
    version: int = QUANT_VERSION,
) -> bytes:
    body = u16(version) + encode_str(decision_id) + encode_str(schema)
    body += u64(len(metrics))
    for name, value in metrics:
        body += encode_str(name) + encode_str(value)
    body += encode_option_bytes(content_hash)
    if cid is None:
        body += u8(0)
    else:
        body += u8(1) + encode_str(cid)
    return QUANT_MAGIC + body


def decode_envelope(data: bytes) -> dict:
    if data[:4] != QUANT_MAGIC:
        raise ValueError("missing IQNT magic")
    view = memoryview(data)[4:]

    def take_u8() -> int:
        nonlocal view
        n = int(view[0])
        view = view[1:]
        return n

    def take_u16() -> int:
        nonlocal view
        n = struct.unpack_from("<H", view, 0)[0]
        view = view[2:]
        return n

    def take_u64() -> int:
        nonlocal view
        n = struct.unpack_from("<Q", view, 0)[0]
        view = view[8:]
        return n

    def take_str() -> str:
        nonlocal view
        n = take_u64()
        s = bytes(view[:n]).decode("utf-8")
        view = view[n:]
        return s

    version = take_u16()
    decision_id = take_str()
    schema = take_str()
    n_metrics = take_u64()
    metrics = [(take_str(), take_str()) for _ in range(n_metrics)]
    has_hash = take_u8()
    content_hash = None
    if has_hash == 1:
        content_hash = bytes(view[:32])
        view = view[32:]
    has_cid = take_u8()
    cid = take_str() if has_cid == 1 else None
    if version != QUANT_VERSION:
        raise ValueError("unsupported version")
    if not decision_id:
        raise ValueError("empty decision_id")
    return {
        "version": version,
        "decision_id": decision_id,
        "schema": schema,
        "metrics": metrics,
        "content_hash": content_hash,
        "cid": cid,
    }


def encode_unsigned_tx(
    sender: bytes,
    to: bytes | None,
    value: int,
    data: bytes,
    gas_price: int,
    gas: int,
    nonce: int,
) -> bytes:
    out = sender
    if to is None:
        out += u8(0)
    else:
        out += u8(1) + to
    out += value.to_bytes(32, "big")
    out += encode_bytes(data)
    out += gas_price.to_bytes(32, "big")
    out += u64(gas)
    out += u64(nonce)
    return out


def encode_signed_tx(
    sk: bytes,
    to: bytes | None,
    nonce: int,
    value: int,
    gas: int,
    gas_price: int,
    data: bytes,
) -> tuple[bytes, bytes]:
    """Return (tx_hash_32, bincode payload)."""
    signing = SigningKey(sk)
    pk = signing.verify_key.encode()
    sender = address_from_pubkey(pk)
    unsigned = encode_unsigned_tx(sender, to, value, data, gas_price, gas, nonce)
    signing_hash = keccak256(unsigned)
    sig = signing.sign(signing_hash).signature
    payload = unsigned + sig + pk
    tx_hash = keccak256(payload)
    return tx_hash, payload
