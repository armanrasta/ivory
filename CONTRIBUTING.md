# Contributing to Ivory

## Setup

```bash
git clone https://github.com/armanrasta/ivory.git
cd ivory
rustup component add rustfmt clippy
cargo build
cargo test -p ivory-primitives -p ivory-crypto -p ivory-core -p ivory-state -p ivory-storage -p ivory-txpool -p ivory-executor
```

Benchmarks (Criterion) live in `tools/ivory-bench`:

```bash
cargo bench -p ivory-bench
cargo bench -p ivory-bench --bench memory   # heap + RSS scenarios
cargo bench -p ivory-bench -- --quick       # shorter Criterion sample
```

Recorded results: [docs/benchmarks.md](docs/benchmarks.md).

RocksDB needs a C++ toolchain. On GCC 15+, see [`.cargo/config.toml`](.cargo/config.toml).

CI (`.github/workflows/ci.yml`) runs `fmt`, `clippy -D warnings` on implemented crates, and the same test packages in four blocks (`primitives`, `ledger`, `storage`, `execution`). Stub crates are not built.

## Workflow

1. Pick an issue (or open one) from the [board](https://github.com/users/armanrasta/projects/6).
2. Branch from `main`: `feature/…`, `fix/…`.
3. Keep changes scoped to one crate or concern when possible.
4. Run before opening a PR:

```bash
cargo fmt --all -- --check
cargo clippy -p <crate> --no-deps --tests -- -D warnings
cargo test -p <crate>
```

5. Use the [PR template](.github/PULL_REQUEST_TEMPLATE.md).

## Style

- Public items need rustdoc.
- Prefer `thiserror` for library errors.
- Match existing module layout and naming in each crate.
- Do not commit secrets, `.idea/`, or `.cursor/`.

## License

By contributing, you agree that your contributions are licensed under the same dual MIT / Apache-2.0 terms as the project.
