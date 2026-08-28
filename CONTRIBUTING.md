# Contributing to Ivory Chain

Thank you for your interest in contributing to Ivory Chain!

## 📋 Table of Contents
- [Code of Conduct](#-code-of-conduct)
- [Getting Started](#-getting-started)
- [Development Setup](#-development-setup)
- [Making Changes](#-making-changes)
- [Pull Request Process](#-pull-request-process)
- [Coding Standards](#-coding-standards)
- [Testing](#-testing)
- [Documentation](#-documentation)

## 🤝 Code of Conduct
This project follows our [Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code.

## 🚀 Getting Started
Look for issues labeled `good first issue` - these are specifically curated for newcomers.

## ⚙️ Development Setup
```bash
# Clone the repository
git clone https://github.com/armanrasta/ivory
cd ivory-chain

# Install development tools
rustup component add rustfmt clippy

# Build the project
cargo build

# Run tests
cargo test --all

# Run lints
cargo clippy --all-targets -- -D warnings

# Format code
cargo fmt --all
```

## ✍️ Making Changes
- Use descriptive branch names: `feature/description`, `fix/description`
- Follow [Conventional Commits](https://www.conventionalcommits.org/)
- Write clear, concise commit messages

## 🔍 Pull Request Process
1. Ensure all tests pass
2. Ensure no clippy warnings
3. Ensure code is formatted
4. Update documentation if needed
5. Fill out PR template completely

## 🧪 Testing
- Unit tests go in the same file as the code
- Integration tests go in `tests/`
- Test edge cases and error conditions
- Aim for high coverage on core logic

## 📚 Documentation
All public items need doc comments with examples.