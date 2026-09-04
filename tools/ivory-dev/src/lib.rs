//! Ivory-dev: scaffold a contract project and resolve chain / key targets.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

const TRACKER_YAML: &str = include_str!("../../../contracts/tracker.yaml");
const TRACKER_WAT: &str = include_str!("../../../contracts/tracker.wat");

/// Project file (`ivory.toml`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DevConfig {
    /// JSON-RPC URL for a local or self-hosted server.
    #[serde(default)]
    pub rpc: String,
    /// Path to an Ed25519 secret (hex file).
    #[serde(default)]
    pub key: String,
}

/// Where `ivory-dev` should send RPC.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChainTarget {
    /// `http://127.0.0.1:8545` (or `ivory.toml` / `IVORY_RPC_URL`).
    Local,
    /// `IVORY_PUBLIC_RPC` (hosted “our chain”).
    Public,
}

impl ChainTarget {
    /// Parse `local` or `public`.
    ///
    /// # Errors
    ///
    /// Unknown name.
    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "public" => Ok(Self::Public),
            other => bail!("unknown --chain {other} (use local or public)"),
        }
    }
}

/// Create `dir` with `ivory.toml` and example contracts.
///
/// # Errors
///
/// Directory exists and is not empty, or IO failures.
pub fn new_project(dir: &Path) -> Result<()> {
    if dir.exists() {
        let empty = std::fs::read_dir(dir)?.next().is_none();
        if !empty {
            bail!("{} exists and is not empty", dir.display());
        }
    }
    std::fs::create_dir_all(dir.join("contracts"))?;
    let cfg = DevConfig {
        rpc: "http://127.0.0.1:8545".into(),
        key: "./deploy.key".into(),
    };
    std::fs::write(dir.join("ivory.toml"), toml::to_string_pretty(&cfg)?)?;
    std::fs::write(dir.join("contracts/tracker.yaml"), TRACKER_YAML)?;
    std::fs::write(dir.join("contracts/tracker.wat"), TRACKER_WAT)?;
    std::fs::write(
        dir.join("README.md"),
        "# Ivory project\n\nWrite contracts here. Deploy with `ivory-dev deploy`.\n\
         Point `ivory.toml` at a local server or set `IVORY_PUBLIC_RPC` for our chain.\n\
         Apps import the Python `ivory-client` package.\n",
    )?;
    Ok(())
}

/// Walk from `start` upward looking for `ivory.toml`.
#[must_use]
pub fn find_project(start: &Path) -> Option<PathBuf> {
    let mut cur = start.to_path_buf();
    loop {
        if cur.join("ivory.toml").is_file() {
            return Some(cur);
        }
        if !cur.pop() {
            return None;
        }
    }
}

/// Load `ivory.toml` if present.
///
/// # Errors
///
/// Parse errors.
pub fn load_dev_config(project: &Path) -> Result<DevConfig> {
    let path = project.join("ivory.toml");
    if !path.exists() {
        return Ok(DevConfig::default());
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&text).context("ivory.toml")
}

/// Resolve RPC URL.
///
/// # Errors
///
/// `public` without `IVORY_PUBLIC_RPC`.
pub fn resolve_rpc(
    rpc_flag: Option<&str>,
    chain: Option<ChainTarget>,
    cfg: &DevConfig,
) -> Result<String> {
    if let Some(u) = rpc_flag.filter(|s| !s.is_empty()) {
        return Ok(u.trim_end_matches('/').to_string());
    }
    if chain == Some(ChainTarget::Public) {
        let public = std::env::var("IVORY_PUBLIC_RPC").unwrap_or_default();
        let public = public.trim().trim_end_matches('/');
        if public.is_empty() {
            bail!("IVORY_PUBLIC_RPC is unset (required for --chain public)");
        }
        return Ok(public.to_string());
    }
    if !cfg.rpc.is_empty() {
        return Ok(cfg.rpc.trim_end_matches('/').to_string());
    }
    if let Ok(u) = std::env::var("IVORY_RPC_URL")
        && !u.is_empty()
    {
        return Ok(u.trim_end_matches('/').to_string());
    }
    Ok("http://127.0.0.1:8545".into())
}

/// Resolve deploy key path.
///
/// # Errors
///
/// No key configured.
pub fn resolve_key_path(
    key_flag: Option<&Path>,
    data_dir: Option<&Path>,
    project: &Path,
    cfg: &DevConfig,
) -> Result<PathBuf> {
    if let Some(p) = key_flag {
        return Ok(p.to_path_buf());
    }
    if let Some(dd) = data_dir {
        return Ok(ivory_node::DataPaths::new(dd.to_path_buf()).validator_key);
    }
    if let Ok(p) = std::env::var("IVORY_DEPLOY_KEY")
        && !p.is_empty()
    {
        return Ok(PathBuf::from(p));
    }
    if !cfg.key.is_empty() {
        let p = PathBuf::from(&cfg.key);
        if p.is_absolute() {
            return Ok(p);
        }
        return Ok(project.join(p));
    }
    bail!("set --key, --data-dir, IVORY_DEPLOY_KEY, or ivory.toml key");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_project_writes_toml_and_contracts() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("app");
        new_project(&root).unwrap();
        assert!(root.join("ivory.toml").exists());
        assert!(root.join("contracts/tracker.yaml").exists());
        assert!(root.join("contracts/tracker.wat").exists());
        let cfg = load_dev_config(&root).unwrap();
        assert_eq!(cfg.rpc, "http://127.0.0.1:8545");
    }

    #[test]
    fn resolve_rpc_local_default() {
        let cfg = DevConfig {
            rpc: "http://127.0.0.1:8545".into(),
            key: String::new(),
        };
        let url = resolve_rpc(None, Some(ChainTarget::Local), &cfg).unwrap();
        assert_eq!(url, "http://127.0.0.1:8545");
    }

    #[test]
    fn resolve_rpc_flag_wins() {
        let cfg = DevConfig {
            rpc: "http://old".into(),
            key: String::new(),
        };
        let url = resolve_rpc(Some("http://flag:1"), Some(ChainTarget::Local), &cfg).unwrap();
        assert_eq!(url, "http://flag:1");
    }

    #[test]
    fn public_chain_requires_env() {
        let cfg = DevConfig::default();
        let prev = std::env::var("IVORY_PUBLIC_RPC").ok();
        unsafe {
            std::env::remove_var("IVORY_PUBLIC_RPC");
        }
        let err = resolve_rpc(None, Some(ChainTarget::Public), &cfg).unwrap_err();
        assert!(err.to_string().contains("IVORY_PUBLIC_RPC"), "{err}");
        match prev {
            Some(v) => unsafe {
                std::env::set_var("IVORY_PUBLIC_RPC", v);
            },
            None => unsafe {
                std::env::remove_var("IVORY_PUBLIC_RPC");
            },
        }
    }
}
