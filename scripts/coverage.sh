#!/usr/bin/env bash
# Generate lcov + HTML for the cheap ledger crates (no coverage fail gate).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
mkdir -p target

if [ "$#" -gt 0 ]; then
  PACKAGES="$*"
else
  PACKAGES="${COVERAGE_PACKAGES:--p ivory-core -p ivory-crypto -p ivory-state}"
fi

if cargo llvm-cov --version >/dev/null 2>&1; then
  cargo llvm-cov ${PACKAGES} --lcov --output-path target/lcov.info \
    --html --output-dir target/llvm-cov
  echo "lcov: target/lcov.info"
  echo "html: target/llvm-cov/html/index.html"
elif command -v cargo-tarpaulin >/dev/null 2>&1; then
  cargo tarpaulin ${PACKAGES} --out lcov --output-dir target
  echo "lcov: target/lcov.info (tarpaulin fallback)"
else
  echo "install cargo-llvm-cov (preferred) or cargo-tarpaulin" >&2
  exit 1
fi
