

# 🦣 Ivory Chain

**Lightweight blockchain for the real world**

Ivory Chain is a modular, high-performance blockchain framework written in Rust. Built for developers who want enterprise-grade blockchain capabilities without enterprise complexity.

## 🚀 Project Status

[![CI](https://github.com/armanrasta/ivory/actions/workflows/ci.yml/badge.svg)](https://github.com/armanrasta/ivory/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

## 🎯 Vision

Build a lightweight, modular blockchain platform that can be used for:
- **Fintech**: Payment systems, escrow, digital assets
- **Supply Chain**: Tracking, provenance, logistics
- **Enterprise**: Private networks, consortium chains
- **DApps**: Decentralized applications, tokens, NFTs

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────┐
│                    Applications                      │
│         (Your DApps, Services, Contracts)           │
└─────────────────────────┬───────────────────────────┘
                          │
┌─────────────────────────┴───────────────────────────┐
│                     Ivory RPC                       │
│              (JSON-RPC, WebSocket, gRPC)            │
└─────────────────────────┬───────────────────────────┘
                          │
┌─────────────────────────┴───────────────────────────┐
│                    Ivory Chain                      │
│    ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐  │
│    │Consensus│ │ TxPool  │ │Executor │ │  State  │  │
│    └─────────┘ └─────────┘ └─────────┘ └─────────┘  │
└─────────────────────────┬───────────────────────────┘
                          │
┌─────────────────────────┴───────────────────────────┐
│                   Ivory Network                     │
│                (P2P, Sync, Gossip)                  │
└─────────────────────────┬───────────────────────────┘
                          │
┌─────────────────────────┴───────────────────────────┐
│                   Ivory Storage                     │
│                (RocksDB, Indexes)                   │
└─────────────────────────────────────────────────────┘
```

## 🛠️ Getting Started

### Prerequisites
- Rust 1.75+
- Git

### Installation
```bash
git clone https://github.com/armanrasta/ivory
cd ivory-chain
cargo build --release
```

### Run Node
```bash
# Initialize chain
ivory init

# Start node
ivory run --rpc-addr 127.0.0.1:8545
```

## 📚 Documentation

- [Architecture Overview](docs/architecture.md)
- [Getting Started Guide](docs/getting-started.md)
- [API Reference](docs/api.md)
- [Development Guide](docs/development.md)

## 🤝 Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## 📜 License

MIT/Apache 2.0 - See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.
```

---

## 📅 PHASE PROCESSING (GitHub Issues & Milestones)

Create these milestones and issues in your GitHub repository:

### 🏁 Milestone 1: Foundation Layer (Due: 2 weeks)
*Focus: Core primitives and cryptography*

#### Issues:
1. **Implement Fixed Hash Types (H256, H160, H512)**  
   `area:primitives` `type:feature` `size:medium`  
   Implement core hash types with serialization support.

2. **Implement Address Type**  
   `area:primitives` `type:feature` `size:small`  
   Implement Ethereum-style 20-byte addresses with derivation.

3. **Implement U256 (Big Integer Arithmetic)**  
   `area:primitives` `type:feature` `size:large`  
   Implement 256-bit unsigned integer with overflow-safe operations.

4. **Cryptography Wrapper (Blake3 & Ed25519)**  
   `area:crypto` `type:feature` `size:medium`  
   Implement hashing, signing, verification, and key generation.

5. **Merkle Tree Implementation**  
   `area:crypto` `type:feature` `size:medium`  
   Implement Merkle trees for transaction proofs.

6. **Project Skeleton & Workspace Setup**  
   `type:chore` `size:small`  
   Complete workspace setup with CI/CD pipeline.

---

### 🏁 Milestone 2: Core Blockchain (Due: 4 weeks)
*Focus: Block structure, transactions, storage*

#### Issues:
7. **Define Block & Header Structures**  
   `area:core` `type:feature` `size:medium`  
   Implement block header with all necessary fields.

8. **Define Transaction Structure**  
   `area:core` `type:feature` `size:medium`  
   Implement transaction format with signature verification.

9. **RocksDB Storage Backend**  
   `area:storage` `type:feature` `size:large`  
   Implement persistent storage using RocksDB.

10. **State Management System**  
    `area:state` `type:feature` `size:large`  
    Implement account state management with balance tracking.

11. **Transaction Executor**  
    `area:executor` `type:feature` `size:large`  
    Implement transaction execution engine with gas handling.

---

### 🏁 Milestone 3: Consensus & Networking (Due: 6 weeks)
*Focus: Block production, consensus, P2P networking*

#### Issues:
12. **Proof of Authority Consensus**  
    `area:consensus` `type:feature` `size:large`  
    Implement simple PoA consensus for single-validator networks.

13. **Transaction Pool (Mempool)**  
    `area:txpool` `type:feature` `size:medium`  
    Implement transaction pool with nonce management.

14. **P2P Networking (libp2p)**  
    `area:network` `type:feature` `size:extra-large`  
    Implement peer discovery, block propagation, and sync.

15. **Chain Management**  
    `area:chain` `type:feature` `size:large`  
    Implement blockchain management with fork resolution.

---

### 🏁 Milestone 4: API & Tooling (Due: 8 weeks)
*Focus: Developer experience, RPC, CLI tools*

#### Issues:
16. **JSON-RPC Server (Axum)**  
    `area:rpc` `type:feature` `size:medium`  
    Implement Ethereum-compatible JSON-RPC API.

17. **WebSocket Subscriptions**  
    `area:rpc` `type:feature` `size:medium`  
    Implement real-time event subscriptions via WebSocket.

18. **CLI Tools (ivory-cli)**  
    `type:feature` `size:medium`  
    Implement command-line interface for node management.

19. **Key Generation Tool**  
    `type:feature` `size:small`  
    Implement tool for generating keypairs and addresses.

20. **Documentation & Examples**  
    `area:docs` `type:chore` `size:large`  
    Create comprehensive documentation and example projects.

---

### 🏁 Milestone 5: Smart Contracts (Due: 12 weeks)
*Focus: WASM-based smart contracts*

#### Issues:
21. **WASM Virtual Machine Integration**  
    `area:vm` `type:feature` `size:extra-large`  
    Integrate wasmer/wasmtime for smart contract execution.

22. **Contract Deployment & Execution**  
    `area:vm` `type:feature` `size:large`  
    Implement contract deployment and message calling.

23. **Gas Metering System**  
    `area:vm` `type:feature` `size:medium`  
    Implement gas metering for contract execution.

24. **Standard Library Contracts**  
    `area:vm` `type:feature` `size:large`  
    Implement standard contracts (Token, Escrow, etc.).

25. **Contract SDK (Rust)**  
    `area:vm` `type:feature` `size:large`  
    Implement Rust SDK for writing smart contracts.

---

## 📊 PROJECT ROADMAP

```markdown
## 🗓️ Roadmap

### Phase 1: Foundation (Current)
- [x] Project setup
- [ ] Primitives (H256, Address, U256)
- [ ] Cryptography (signing, hashing)
- [ ] Core types (Block, Transaction)
- [ ] Storage (RocksDB)
- [ ] Basic consensus (PoA)

### Phase 2: Network
- [ ] P2P networking (libp2p)
- [ ] Block propagation
- [ ] Chain synchronization
- [ ] JSON-RPC API

### Phase 3: Smart Contracts
- [ ] WASM VM integration
- [ ] Contract deployment
- [ ] Gas metering
- [ ] Contract SDK

### Phase 4: Production
- [ ] Security audit
- [ ] Performance optimization
- [ ] Documentation
- [ ] Mainnet launch
