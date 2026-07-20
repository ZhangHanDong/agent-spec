use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{AtlasError, BuildOptions, SCHEMA_VERSION, TargetLayout, io_err};

const INPUT_PLAN_SCHEMA_VERSION: u32 = 2;
const INPUT_PLAN_FILE: &str = "input-plan.json";
const PROVIDER_ID: &str = "cargo-metadata-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InputPlanState {
    Hit,
    Miss,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InputPlanKey {
    schema_version: u32,
    provider: String,
    toolchain: String,
    manifests: BTreeMap<String, String>,
    features: Vec<String>,
    target: Option<String>,
    cfg: Vec<String>,
    environment: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CargoInputPlan {
    schema_version: u32,
    pub(crate) fingerprint: String,
    pub(crate) graph_root: String,
    pub(crate) packages: Vec<String>,
    pub(crate) targets: Vec<TargetLayout>,
    features: Vec<String>,
    target: Option<String>,
    cfg: Vec<String>,
}

impl InputPlanKey {
    pub(crate) fn capture(
        code_root: &Path,
        opts: &BuildOptions,
        toolchain: &str,
    ) -> Result<Self, AtlasError> {
        let features = canonical_values(&opts.features, "feature")?;
        let cfg = canonical_values(&opts.cfg, "cfg")?;
        let target = opts
            .target
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if opts.target.is_some() && target.is_none() {
            return Err(AtlasError::Cargo("target must not be empty".to_string()));
        }
        let environment = [
            "CARGO_HOME",
            "CARGO_TARGET_DIR",
            "RUSTFLAGS",
            "RUSTDOCFLAGS",
            "RUSTUP_TOOLCHAIN",
        ]
        .into_iter()
        .filter_map(|name| {
            std::env::var(name)
                .ok()
                .map(|value| (name.to_string(), value))
        })
        .collect();
        Ok(Self {
            schema_version: SCHEMA_VERSION,
            provider: PROVIDER_ID.to_string(),
            toolchain: toolchain.to_string(),
            manifests: input_hashes(code_root)?,
            features,
            target,
            cfg,
            environment,
        })
    }

    pub(crate) fn fingerprint(&self) -> Result<String, AtlasError> {
        let bytes = serde_json::to_vec(self).map_err(|error| AtlasError::Io(error.to_string()))?;
        Ok(blake3::hash(&bytes).to_hex().to_string())
    }
}

impl CargoInputPlan {
    pub(crate) fn new(
        fingerprint: String,
        graph_root: String,
        packages: Vec<String>,
        targets: Vec<TargetLayout>,
        key: &InputPlanKey,
    ) -> Self {
        Self {
            schema_version: INPUT_PLAN_SCHEMA_VERSION,
            fingerprint,
            graph_root,
            packages,
            targets,
            features: key.features.clone(),
            target: key.target.clone(),
            cfg: key.cfg.clone(),
        }
    }

    pub(crate) fn apply_build_inputs(&self, options: &mut BuildOptions) {
        options.features.clone_from(&self.features);
        options.target.clone_from(&self.target);
        options.cfg.clone_from(&self.cfg);
    }
}

pub(crate) fn load(data_dir: &Path, fingerprint: &str) -> Option<CargoInputPlan> {
    let plan = load_committed(data_dir)?;
    (plan.fingerprint == fingerprint).then_some(plan)
}

pub(crate) fn load_committed(data_dir: &Path) -> Option<CargoInputPlan> {
    let text = std::fs::read_to_string(data_dir.join(INPUT_PLAN_FILE)).ok()?;
    let plan: CargoInputPlan = serde_json::from_str(&text).ok()?;
    (plan.schema_version == INPUT_PLAN_SCHEMA_VERSION).then_some(plan)
}

pub(crate) fn write(data_dir: &Path, plan: &CargoInputPlan) -> Result<(), AtlasError> {
    crate::index::write_json_atomic(&data_dir.join(INPUT_PLAN_FILE), plan)
}

fn canonical_values(values: &[String], label: &str) -> Result<Vec<String>, AtlasError> {
    let mut canonical = BTreeSet::new();
    for value in values {
        let value = value.trim();
        if value.is_empty() {
            return Err(AtlasError::Cargo(format!("{label} must not be empty")));
        }
        canonical.insert(value.to_string());
    }
    Ok(canonical.into_iter().collect())
}

fn input_hashes(code_root: &Path) -> Result<BTreeMap<String, String>, AtlasError> {
    let mut paths = ignore::WalkBuilder::new(code_root)
        .hidden(false)
        .git_ignore(true)
        .build()
        .filter_map(Result::ok)
        .map(|entry| entry.into_path())
        .filter(|path| {
            path.is_file()
                && !path
                    .components()
                    .any(|component| component.as_os_str() == "target")
                && matches!(
                    path.file_name().and_then(|name| name.to_str()),
                    Some(
                        "Cargo.toml"
                            | "Cargo.lock"
                            | "rust-toolchain"
                            | "rust-toolchain.toml"
                            | "config"
                            | "config.toml"
                    )
                )
        })
        .collect::<Vec<PathBuf>>();
    paths.sort();
    let mut hashes = BTreeMap::new();
    for path in paths {
        let bytes = std::fs::read(&path).map_err(io_err)?;
        let canonical = std::fs::canonicalize(&path).map_err(io_err)?;
        hashes.insert(
            canonical.to_string_lossy().into_owned(),
            blake3::hash(&bytes).to_hex().to_string(),
        );
    }
    Ok(hashes)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    fn fixture() -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "rust-atlas-input-plan-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join(".cargo")).unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
        fs::write(root.join(".cargo/config.toml"), "[build]\n").unwrap();
        root
    }

    #[test]
    fn test_atlas_input_plan_uses_content_not_manifest_mtime() {
        let root = fixture();
        let options = BuildOptions::default();
        let first = InputPlanKey::capture(&root, &options, "rustc test")
            .unwrap()
            .fingerprint()
            .unwrap();
        let same = InputPlanKey::capture(&root, &options, "rustc test")
            .unwrap()
            .fingerprint()
            .unwrap();
        assert_eq!(first, same);

        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
        let changed = InputPlanKey::capture(&root, &options, "rustc test")
            .unwrap()
            .fingerprint()
            .unwrap();
        assert_ne!(first, changed);
        fs::remove_dir_all(root).ok();
    }
}
