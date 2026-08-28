#!/bin/bash
echo "Setting up Ivory Chain development environment..."

# Check Rust installation
if ! command -v cargo &> /dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    source $HOME/.cargo/env
fi

# Install required components
rustup component add rustfmt clippy

# Build project
echo "Building project..."
cargo build

# Run tests
echo "Running tests..."
cargo test --workspace

echo "Setup complete! Happy coding! 🦣"