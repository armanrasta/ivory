# Benchmarks

Criterion harness: [`tools/ivory-bench`](../tools/ivory-bench/). Measures ledger hot paths that matter for a quant-submission engine — hashing, in-memory state, mempool admission, transfer execution, and pool → execute.

## How to run

```bash
# Optimized `bench` profile (opt-level=3, thin LTO — same class as release)
cargo bench -p ivory-bench                  # Criterion time / throughput
cargo bench -p ivory-bench --bench memory   # heap + RSS footprint scenarios
cargo bench -p ivory-bench -- --quick       # shorter Criterion sample
```

Artifacts build under `target/release/deps/` with the `bench` profile. Criterion HTML reports land under `target/criterion/` (gitignored).

## Snapshot (2026-08-30)

| | |
|---|---|
| **Command** | `cargo bench -p ivory-bench` (full Criterion sample, not `--quick`) |
| **Profile** | `bench` — `opt-level = 3`, `lto = "thin"`, `codegen-units = 1` |
| **Host** | Linux (local developer machine) |
| **Notes** | Signature verify is a no-op; no RocksDB, P2P, or WASM in these paths. Numbers are indicative, not SLAs. |

Median times below are Criterion’s middle estimate; throughput is where Criterion reported it.

### Transaction hash (`bincode` + blake3)

| Benchmark | Time | Throughput |
|-----------|------|------------|
| `tx_hash/data_0` | ~1.06 µs | — |
| `tx_hash/data_256` | ~1.89 µs | ~129 MiB/s |
| `tx_hash/data_4096` | ~5.78 µs | ~675 MiB/s |

### In-memory state (`StateDB`)

| Benchmark | Time |
|-----------|------|
| `state/get_account` | ~87 ns |
| `state/set_account` | ~80 ns |

### Mempool admission

| Benchmark | Time | Throughput |
|-----------|------|------------|
| `pool_add/single_sender_64` | ~367 µs | ~175 K tx/s |
| `pool_add/multi_sender_64` | ~452 µs | ~142 K tx/s |
| `pool_add/single_sender_256` | ~1.61 ms | ~159 K tx/s |
| `pool_add/multi_sender_256` | ~1.55 ms | ~165 K tx/s |
| `pool_add/single_sender_1024` | ~6.47 ms | ~158 K tx/s |
| `pool_add/multi_sender_1024` | ~6.36 ms | ~161 K tx/s |

### Mempool `get_pending`

| Benchmark | Time | Throughput |
|-----------|------|------------|
| `pool_get_pending/take_64` | ~21 µs | ~3.0 M tx/s |
| `pool_get_pending/take_256` | ~45 µs | ~5.7 M tx/s |
| `pool_get_pending/take_1024` | ~168 µs | ~6.1 M tx/s |

### Transfer execution

| Benchmark | Time | Throughput |
|-----------|------|------------|
| `execute_transfer/single` | ~1.69 µs | — |
| `execute_transfer/block_64` | ~97 µs | ~660 K tx/s |
| `execute_transfer/block_256` | ~379 µs | ~675 K tx/s |
| `execute_transfer/block_1024` | ~1.51 ms | ~678 K tx/s |
| `execute_transfer/quant_data_256` | ~2.78 µs | ~88 MiB/s |
| `execute_transfer/quant_data_4096` | ~9.29 µs | ~421 MiB/s |

Quant benches execute a transfer whose `data` is 256 or 4096 bytes (intrinsic gas only; no VM).

### Pipeline (admit → pending → execute → remove)

| Benchmark | Time | Throughput |
|-----------|------|------------|
| `pool_to_execute/pipeline_64` | ~239 µs | ~268 K tx/s |
| `pool_to_execute/pipeline_256` | ~955 µs | ~268 K tx/s |

## Memory snapshot (2026-08-30)

| | |
|---|---|
| **Command** | `cargo bench -p ivory-bench --bench memory` |
| **Profile** | `bench` (same optimized settings as Criterion) |
| **Heap** | `peak_alloc` current-usage delta while objects stay live |
| **RSS** | Process `physical_mem` delta (`memory-stats`); may be 0 when the kernel does not grow RSS |
| **Notes** | Single process, no RocksDB/P2P/WASM. Not comparable to a Fabric peer+orderer deployment footprint. |

| Scenario | Items | Heap Δ | Heap / item | RSS Δ |
|----------|------:|--------|-------------|-------|
| `state_accounts_1000` | 1 000 | ~258 KiB | ~264 B | ~416 KiB |
| `state_accounts_10000` | 10 000 | ~2.0 MiB | ~211 B | ~2.0 MiB |
| `state_accounts_100000` | 100 000 | ~16.1 MiB | ~169 B | ~16.1 MiB |
| `pool_pending_1000_data_0` | 1 000 | ~484 KiB | ~495 B | ~412 KiB |
| `pool_pending_4096_data_0` | 4 096 | ~2.0 MiB | ~509 B | ~2.1 MiB |
| `pool_pending_256_data_256` | 256 | ~238 KiB | ~952 B | ~0* |
| `pool_pending_256_data_4096` | 256 | ~1.2 MiB | ~4.7 KiB | ~1.1 MiB |
| `execute_block_256` | 256 | ~676 B | — | ~0* |
| `execute_block_1024` | 1 024 | ~676 B | — | ~0* |
| `pipeline_256` | 256 | ~242 KiB | ~966 B | ~308 KiB |
| `pipeline_1024` | 1 024 | ~753 KiB | ~753 B | ~1.1 MiB |

\*RSS sometimes unchanged when allocations fit in already-mapped pages; trust heap Δ for per-item cost.

Process peak heap over the whole memory run: **~26.5 MiB**.

### Reading the memory numbers

- **~170–260 B / account** in `StateDB` (address key + `Account` + map overhead); 100 000 accounts ≈ **16 MiB** heap.
- **~0.5 KiB / empty pending tx**; **~4.7 KiB / item** when each pending tx carries 4 KiB quant `data`.
- **Execute-only** barely grows heap when only two accounts are updated (from/to) — cost is CPU, not RAM.
- **Pipeline** cost is dominated by the mempool’s live pending set (hash maps + cloned txs), not the executor.

For Fabric-style comparisons, contrast this **single-binary substrate** (tens of MiB for 100k accounts) with peer+orderer(+chaincode) deployments commonly provisioned in the **GB** range — different scope, but useful as a ceiling on Ivory’s *ledger core* before networking and crypto.

## Reading the numbers

- **~1.7 µs / transfer** and **~660–680 K transfers/s** in a synthetic block are upper bounds on the **in-memory execute** path. Real blocks add consensus, networking, persistence, and (later) signature verify + WASM.
- Pool admit (~140–175 K tx/s) is slower than execute mainly because each admit hashes the transaction and updates DashMap nonce maps.
- Quant-sized calldata is cheap until a VM interprets it; today cost is mostly hash + intrinsic gas accounting.
- Prefer the full `cargo bench` sample over `--quick` when updating the time tables; refresh the memory table via `--bench memory`.

Re-run and refresh this page when Phase 3 (chain / PoA) or crypto verify lands — those will change the pipeline picture.

## Related

- Time harness: [`tools/ivory-bench/benches/hot_paths.rs`](../tools/ivory-bench/benches/hot_paths.rs)
- Memory harness: [`tools/ivory-bench/benches/memory.rs`](../tools/ivory-bench/benches/memory.rs)
- Roadmap: [issue #24](https://github.com/armanrasta/ivory/issues/24)
