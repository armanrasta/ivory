//! Node configuration and genesis files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use ivory_consensus::{PoAConfig, Validator};
use ivory_core::Account;
use ivory_crypto::secret_from_bytes;
use ivory_primitives::{Address, PublicKey, SecretKey, U256};
use ivory_state::StateDB;
use serde::{Deserialize, Serialize};

/// Operator role: master may produce; slave only follows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerRole {
    /// Produce blocks when this key is the genesis validator.
    #[default]
    Master,
    /// Never produce; sync via `bootstrap` when set.
    Slave,
}

impl ServerRole {
    /// Parse `master` / `slave` and operator aliases.
    ///
    /// # Errors
    ///
    /// Unknown role string.
    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "master" | "validator" | "producer" => Ok(Self::Master),
            "slave" | "follower" => Ok(Self::Slave),
            other => bail!("unknown role {other} (use master or slave)"),
        }
    }

    /// Wire string for `config.toml`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Master => "master",
            Self::Slave => "slave",
        }
    }

    /// Whether this process is allowed to seal blocks.
    #[must_use]
    pub const fn may_produce(self) -> bool {
        matches!(self, Self::Master)
    }
}

/// Options for [`init_datadir_with`].
#[derive(Clone, Debug, Default)]
pub struct InitOpts {
    /// Written as `role` in `config.toml`.
    pub role: ServerRole,
    /// Written as `bootstrap` in `config.toml`.
    pub bootstrap: Vec<String>,
}

/// On-disk node config (`config.toml`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeFileConfig {
    /// Chain id for RPC.
    pub chain_id: u64,
    /// JSON-RPC bind address.
    pub rpc_addr: String,
    /// libp2p listen multiaddr.
    pub p2p_listen: String,
    /// Bootstrap multiaddrs.
    #[serde(default)]
    pub bootstrap: Vec<String>,
    /// Block production interval in milliseconds.
    pub block_interval_ms: u64,
    /// Extra directory of YAML/WAT/WASM contract packages (optional).
    #[serde(default)]
    pub contracts_dir: String,
    /// `master` produces (if authorized); `slave` never produces.
    #[serde(default)]
    pub role: ServerRole,
}

impl Default for NodeFileConfig {
    fn default() -> Self {
        Self {
            chain_id: 1,
            rpc_addr: "127.0.0.1:8545".into(),
            p2p_listen: "/ip4/127.0.0.1/tcp/0".into(),
            bootstrap: Vec::new(),
            block_interval_ms: 2_000,
            contracts_dir: String::new(),
            role: ServerRole::Master,
        }
    }
}

/// Genesis description (`genesis.json`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenesisFile {
    /// Unix timestamp for genesis.
    pub timestamp: u64,
    /// Header gas limit.
    pub gas_limit: u64,
    /// PoA validator.
    pub validator: GenesisValidator,
    /// Sealed genesis `extra_data` (hex).
    #[serde(default)]
    pub extra_data: String,
    /// Initial balances: address hex -> decimal or hex string.
    #[serde(default)]
    pub alloc: HashMap<String, String>,
}

/// Validator identity in genesis.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenesisValidator {
    /// Address.
    pub address: String,
    /// Ed25519 public key hex.
    pub public_key: String,
}

impl GenesisFile {
    /// Convert to a PoA validator set.
    ///
    /// # Errors
    ///
    /// Invalid hex keys/addresses.
    pub fn poa_config(&self) -> Result<PoAConfig> {
        let pk = parse_public_key(&self.validator.public_key)?;
        let address = Address::from_hex(&self.validator.address)
            .or_else(|_| {
                Address::from_hex(&format!(
                    "0x{}",
                    self.validator.address.trim_start_matches("0x")
                ))
            })
            .context("validator address")?;
        Ok(PoAConfig::single(Validator {
            address,
            public_key: pk,
        }))
    }

    /// State root of genesis alloc (Patricia account trie).
    ///
    /// # Errors
    ///
    /// Invalid address or balance.
    pub fn alloc_state_root(&self) -> Result<ivory_primitives::H256> {
        let state = StateDB::new();
        for (addr, bal) in self.parsed_alloc()? {
            let mut acc = Account::new();
            acc.balance = bal;
            state.set_account(addr, acc);
        }
        Ok(state.root_hash())
    }

    /// Parsed alloc map.
    ///
    /// # Errors
    ///
    /// Invalid address or balance.
    pub fn parsed_alloc(&self) -> Result<Vec<(Address, U256)>> {
        let mut out = Vec::new();
        for (addr, bal) in &self.alloc {
            let address = Address::from_hex(addr).context("alloc address")?;
            let balance = parse_u256(bal)?;
            out.push((address, balance));
        }
        Ok(out)
    }
}

fn parse_u256(s: &str) -> Result<U256> {
    if s.starts_with("0x") || s.starts_with("0X") {
        return U256::from_hex(s).context("alloc balance hex");
    }
    let n: u128 = s.parse().context("alloc balance decimal")?;
    Ok(U256::from_u128(n))
}

fn parse_public_key(s: &str) -> Result<PublicKey> {
    let h = ivory_primitives::H256::from_hex(s).context("public key")?;
    Ok(PublicKey::from_bytes(h.to_bytes()))
}

/// Load a 32-byte secret from a hex file.
///
/// # Errors
///
/// IO or hex errors.
pub fn load_secret_key(path: &Path) -> Result<SecretKey> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let text = text.trim();
    let h = ivory_primitives::H256::from_hex(text).context("secret key hex")?;
    secret_from_bytes(h.to_bytes()).context("secret key")
}

/// Write secret key hex.
///
/// # Errors
///
/// IO errors.
pub fn write_secret_key(path: &Path, sk: &SecretKey) -> Result<()> {
    std::fs::write(path, sk.to_hex()).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Paths under a data directory.
#[derive(Clone, Debug)]
pub struct DataPaths {
    /// Root.
    pub root: PathBuf,
    /// `config.toml`.
    pub config: PathBuf,
    /// `genesis.json`.
    pub genesis: PathBuf,
    /// `validator.key`.
    pub validator_key: PathBuf,
    /// RocksDB directory for canonical blocks.
    pub chain: PathBuf,
    /// Installed contract YAML / WAT / WASM.
    pub contracts: PathBuf,
}

impl DataPaths {
    /// Standard layout.
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self {
            config: root.join("config.toml"),
            genesis: root.join("genesis.json"),
            validator_key: root.join("validator.key"),
            chain: root.join("chain"),
            contracts: root.join("contracts"),
            root,
        }
    }
}

/// Initialize a data directory with genesis, config, and validator key.
///
/// # Errors
///
/// IO or serialization errors.
pub fn init_datadir(root: &Path) -> Result<DataPaths> {
    init_datadir_with(root, InitOpts::default())
}

/// [`init_datadir`] with role and bootstrap.
///
/// # Errors
///
/// IO or serialization errors.
pub fn init_datadir_with(root: &Path, opts: InitOpts) -> Result<DataPaths> {
    std::fs::create_dir_all(root).with_context(|| format!("mkdir {}", root.display()))?;
    let paths = DataPaths::new(root.to_path_buf());
    let (sk, pk, addr) = ivory_crypto::generate_keypair();
    write_secret_key(&paths.validator_key, &sk)?;
    let poa = ivory_consensus::PoAConsensus::from_secret(&sk)?;
    let mut header = ivory_core::BlockHeader {
        number: 0,
        parent_hash: ivory_primitives::H256::ZERO,
        timestamp: 1,
        miner: addr,
        gas_limit: 30_000_000,
        gas_used: 0,
        state_root: ivory_primitives::H256::ZERO,
        transactions_root: ivory_primitives::H256::ZERO,
        receipts_root: ivory_primitives::H256::ZERO,
        difficulty: ivory_primitives::U256::ZERO,
        extra_data: ivory_primitives::Bytes::new(),
    };
    let alloc = HashMap::from([(addr.to_hex(), "1000000000000000000".into())]);
    let mut funded = Account::new();
    funded.balance = U256::from_u128(1_000_000_000_000_000_000);
    let genesis_state = StateDB::new();
    genesis_state.set_account(addr, funded);
    header.state_root = genesis_state.root_hash();
    ivory_consensus::ConsensusEngine::seal_header(&poa, &mut header, &addr, &sk)?;
    let genesis = GenesisFile {
        timestamp: 1,
        gas_limit: 30_000_000,
        validator: GenesisValidator {
            address: addr.to_hex(),
            public_key: pk.to_hex(),
        },
        extra_data: format!("0x{}", hex::encode(header.extra_data.as_slice())),
        alloc,
    };
    std::fs::write(
        &paths.genesis,
        serde_json::to_string_pretty(&genesis)? + "\n",
    )?;
    let cfg = NodeFileConfig {
        role: opts.role,
        bootstrap: opts.bootstrap,
        ..NodeFileConfig::default()
    };
    std::fs::write(&paths.config, toml::to_string_pretty(&cfg)?)?;
    std::fs::create_dir_all(&paths.chain)
        .with_context(|| format!("mkdir {}", paths.chain.display()))?;
    std::fs::create_dir_all(&paths.contracts)
        .with_context(|| format!("mkdir {}", paths.contracts.display()))?;
    Ok(paths)
}

/// Load config + genesis from disk.
///
/// # Errors
///
/// Missing files or parse errors.
pub fn load_datadir(root: &Path) -> Result<(NodeFileConfig, GenesisFile, SecretKey, DataPaths)> {
    let paths = DataPaths::new(root.to_path_buf());
    let cfg: NodeFileConfig =
        toml::from_str(&std::fs::read_to_string(&paths.config).context("config.toml")?)?;
    let genesis: GenesisFile =
        serde_json::from_str(&std::fs::read_to_string(&paths.genesis).context("genesis.json")?)?;
    let sk = load_secret_key(&paths.validator_key)?;
    Ok((cfg, genesis, sk, paths))
}
