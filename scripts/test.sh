#!/bin/bash
echo "Running Ivory Chain tests..."

# Run unit tests
cargo test --workspace

# Run clippy
cargo clippy --workspace -- -D warnings

# Run formatting check
cargo fmt --check --all

echo "All tests completed!"