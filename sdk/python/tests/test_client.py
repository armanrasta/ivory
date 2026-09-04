from unittest.mock import patch

from ivory_client.codec import decode_envelope, encode_envelope, keccak256
from ivory_client.client import IvoryClient


def test_envelope_roundtrip():
    raw = encode_envelope(
        "dec-1",
        "app.v1",
        [("score", "0.82")],
        cid="bafyexample",
    )
    assert raw[:4] == b"IQNT"
    decoded = decode_envelope(raw)
    assert decoded["decision_id"] == "dec-1"
    assert decoded["metrics"] == [("score", "0.82")]
    assert decoded["cid"] == "bafyexample"


def test_minimal_envelope_layout():
    raw = encode_envelope("d", "s", [])
    expected = (
        b"IQNT"
        + (1).to_bytes(2, "little")
        + (1).to_bytes(8, "little")
        + b"d"
        + (1).to_bytes(8, "little")
        + b"s"
        + (0).to_bytes(8, "little")
        + bytes([0, 0])
    )
    assert raw == expected


def test_keccak_empty():
    assert keccak256(b"").hex() == "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"


def test_get_balance_mocked():
    client = IvoryClient("http://127.0.0.1:8545", secret_key=b"\x01" * 32, chain_id=1)
    with patch.object(client, "_rpc", return_value="0x2a"):
        assert client.get_balance("0x" + "00" * 20) == 42
    client.close()


def test_submit_decision_mocked():
    client = IvoryClient("http://127.0.0.1:8545", secret_key=b"\x01" * 32, chain_id=1)

    def rpc(method, params=None):
        if method == "eth_getTransactionCount":
            return "0x0"
        if method == "eth_sendRawTransaction":
            assert params and params[0].startswith("0x")
            return "0x" + "ab" * 32
        raise AssertionError(method)

    with patch.object(client, "_rpc", side_effect=rpc):
        txh = client.submit_decision("d1", "app.v1", [("k", "1")], to_hex="0x" + "11" * 20)
    assert txh.startswith("0x")
    assert len(txh) == 66
    client.close()
