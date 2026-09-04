//! YAML / WAT / WASM contract sources for deploy and the explorer catalog.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use ivory_primitives::{H256, keccak256};
use ivory_rpc::ContractMeta;
use serde::Deserialize;

/// On-disk contract package (YAML).
#[derive(Clone, Debug, Deserialize)]
pub struct ContractManifest {
    /// Display name (required).
    pub name: String,
    /// Application schema id (e.g. `app.v1`).
    #[serde(default = "default_schema")]
    pub schema: String,
    /// Human note for the explorer.
    #[serde(default)]
    pub description: String,
    /// Relative path to `.wat` or `.wasm`.
    #[serde(default)]
    pub source: Option<String>,
    /// Inline WebAssembly text (alternative to `source`).
    #[serde(default)]
    pub wat: Option<String>,
}

fn default_schema() -> String {
    "app.v1".into()
}

/// Compiled bytecode plus catalog fields.
#[derive(Clone, Debug)]
pub struct CompiledContract {
    /// Manifest name.
    pub name: String,
    /// Schema id.
    pub schema: String,
    /// Description.
    pub description: String,
    /// Path shown in the explorer (manifest or source file).
    pub source: String,
    /// Runtime WASM installed by CREATE.
    pub wasm: Vec<u8>,
    /// `keccak256(wasm)`.
    pub code_hash: H256,
}

impl CompiledContract {
    /// Metadata for `ivory_listContracts`.
    #[must_use]
    pub fn meta(&self) -> ContractMeta {
        ContractMeta {
            name: self.name.clone(),
            schema: self.schema.clone(),
            source: self.source.clone(),
            description: self.description.clone(),
        }
    }
}

/// Load a YAML manifest, `.wat`, or `.wasm` path.
///
/// # Errors
///
/// Missing files, empty name, or WAT parse failures.
pub fn load_contract_file(path: &Path) -> Result<CompiledContract> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "yaml" | "yml" => load_yaml(path),
        "wat" => compile_wat_file(
            path,
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("contract"),
        ),
        "wasm" => load_wasm_file(
            path,
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("contract"),
        ),
        other => bail!("unsupported contract file .{other} (use .yaml, .wat, or .wasm)"),
    }
}

fn load_yaml(path: &Path) -> Result<CompiledContract> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let man: ContractManifest =
        serde_yaml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    if man.name.trim().is_empty() {
        bail!("{}: name is required", path.display());
    }
    let parent = path.parent().unwrap_or(Path::new("."));
    let (wasm, source) = if let Some(inline) = man.wat.as_deref() {
        let wasm = wat::parse_str(inline).context("inline wat")?;
        (wasm, path.display().to_string())
    } else if let Some(rel) = man.source.as_deref() {
        let src = parent.join(rel);
        let compiled = load_contract_file(&src)?;
        (compiled.wasm, src.display().to_string())
    } else {
        bail!("{}: set `source` or `wat`", path.display());
    };
    if wasm.is_empty() {
        bail!("{}: empty bytecode", path.display());
    }
    Ok(CompiledContract {
        name: man.name,
        schema: man.schema,
        description: man.description,
        source,
        code_hash: keccak256(&wasm),
        wasm,
    })
}

fn compile_wat_file(path: &Path, name: &str) -> Result<CompiledContract> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let wasm = wat::parse_str(&text).with_context(|| format!("wat {}", path.display()))?;
    Ok(CompiledContract {
        name: name.into(),
        schema: default_schema(),
        description: String::new(),
        source: path.display().to_string(),
        code_hash: keccak256(&wasm),
        wasm,
    })
}

fn load_wasm_file(path: &Path, name: &str) -> Result<CompiledContract> {
    let wasm = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    if wasm.is_empty() {
        bail!("{}: empty wasm", path.display());
    }
    Ok(CompiledContract {
        name: name.into(),
        schema: default_schema(),
        description: String::new(),
        source: path.display().to_string(),
        code_hash: keccak256(&wasm),
        wasm,
    })
}

/// Copy a contract package into `dest_dir` so a running node can name it.
///
/// # Errors
///
/// IO errors.
pub fn install_contract_files(src: &Path, dest_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dest_dir).with_context(|| format!("mkdir {}", dest_dir.display()))?;
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(ext.as_str(), "yaml" | "yml") {
        let text = std::fs::read_to_string(src)?;
        let man: ContractManifest = serde_yaml::from_str(&text)?;
        let dest_yaml = dest_dir.join(src.file_name().unwrap_or_default());
        std::fs::copy(src, &dest_yaml)
            .with_context(|| format!("copy {} -> {}", src.display(), dest_yaml.display()))?;
        if let Some(rel) = man.source.as_deref() {
            let from = src.parent().unwrap_or(Path::new(".")).join(rel);
            let to = dest_dir.join(Path::new(rel).file_name().unwrap_or_default());
            if from.exists() {
                std::fs::copy(&from, &to)
                    .with_context(|| format!("copy {} -> {}", from.display(), to.display()))?;
            }
        }
    } else {
        let dest = dest_dir.join(src.file_name().unwrap_or_default());
        std::fs::copy(src, &dest)
            .with_context(|| format!("copy {} -> {}", src.display(), dest.display()))?;
    }
    Ok(())
}

/// Scan directories for YAML/WAT/WASM and index by code hash.
#[must_use]
pub fn load_catalog(dirs: &[PathBuf]) -> HashMap<H256, ContractMeta> {
    let mut out = HashMap::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !matches!(ext.as_str(), "yaml" | "yml" | "wat" | "wasm") {
                continue;
            }
            // Prefer YAML as the named package; skip raw source if a yaml exists.
            if matches!(ext.as_str(), "wat" | "wasm") {
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let has_yaml = dir.join(format!("{stem}.yaml")).exists()
                    || dir.join(format!("{stem}.yml")).exists();
                if has_yaml {
                    continue;
                }
            }
            match load_contract_file(&path) {
                Ok(c) => {
                    out.insert(c.code_hash, c.meta());
                }
                Err(e) => tracing::debug!(path = %path.display(), error = %e, "skip contract file"),
            }
        }
    }
    out
}

/// Directories the node / deploy tool search for packages.
#[must_use]
pub fn catalog_dirs(data_contracts: &Path, extra: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = vec![data_contracts.to_path_buf(), PathBuf::from("contracts")];
    if let Some(p) = extra
        && !p.as_os_str().is_empty()
    {
        dirs.push(p.to_path_buf());
    }
    dirs.sort();
    dirs.dedup();
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_repo_tracker_yaml() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/tracker.yaml");
        let c = load_contract_file(&path).unwrap();
        assert_eq!(c.name, "tracker");
        assert_eq!(c.schema, "app.v1");
        assert!(!c.wasm.is_empty());
        assert_eq!(c.code_hash, keccak256(&c.wasm));
    }

    #[test]
    fn yaml_inline_wat() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = dir.path().join("ping.yaml");
        std::fs::write(
            &yaml,
            "name: ping\nschema: app.v1\nwat: |\n  (module (func (export \"call\")))\n",
        )
        .unwrap();
        let c = load_contract_file(&yaml).unwrap();
        assert_eq!(c.name, "ping");
        assert!(!c.wasm.is_empty());
    }

    #[test]
    fn catalog_indexes_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts");
        install_contract_files(&src.join("tracker.yaml"), dir.path()).unwrap();
        let cat = load_catalog(&[dir.path().to_path_buf()]);
        assert!(cat.values().any(|m| m.name == "tracker"));
    }
}
