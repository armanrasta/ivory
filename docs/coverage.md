# Coverage

There is **no failing coverage gate**. Numbers below are a snapshot from
`cargo llvm-cov --summary-only` on 2026-09-05. These are observations, not SLAs.

```bash
scripts/coverage.sh
scripts/coverage.sh -p ivory-executor -p ivory-txpool -p ivory-vm
```

CI publishes lcov + HTML artifacts (`coverage-cheap`, `coverage-exec`) and runs
`pip install -e sdk/python[dev]` + pytest.

## Snapshot (2026-09-05)

| Slice | Packages | Line coverage |
|-------|----------|---------------|
| Core | `ivory-core`, `ivory-crypto`, `ivory-state` | **96.18%** (916 lines, 35 missed) |
| Exec | `ivory-executor`, `ivory-txpool`, `ivory-vm` | **97.58%** (1365 lines, 33 missed) |

Per-crate line rates in that run:

| Package | File | Lines |
|---------|------|-------|
| `ivory-core` | `account.rs` | 100.00% |
| `ivory-core` | `block.rs` | 100.00% |
| `ivory-core` | `quant.rs` | 98.55% |
| `ivory-crypto` | `keys.rs` | 90.00% |
| `ivory-crypto` | `lib.rs` / `sign.rs` / `tx.rs` | 100.00% |
| `ivory-state` | `state.rs` | 88.19% |
| `ivory-state` | `trie.rs` | 97.56% |
| `ivory-executor` | `call.rs` / `context.rs` / `gas.rs` | 100.00% |
| `ivory-executor` | `executor.rs` | 96.21% |
| `ivory-txpool` | `config.rs` | 100.00% |
| `ivory-txpool` | `pool.rs` | 99.68% |
| `ivory-vm` | `lib.rs` | 100.00% |
| `ivory-vm` | `vm.rs` | 93.06% |

Do not invent a 90% fail-under target from this table.
