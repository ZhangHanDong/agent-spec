use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use super::{
    AtlasError, CfgSummary, DispatchKind, Edge, EdgeConfidence, EdgeKind, EdgeResolution, EdgeSite,
    ExtractorIdentity, MirBuildOptions, NodeKind, Provenance, Shard, read_shard,
};

const MIR_OVERLAY_SCHEMA: &str = "rust-atlas/mir-overlay-v1";

#[derive(Debug)]
pub(super) struct MirApplied {
    pub tool: String,
    pub overlay_path: String,
    pub overlay_fingerprint: String,
    pub source_fingerprint: String,
    pub shards: BTreeMap<String, Shard>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MirOverlay {
    schema: String,
    extractor: MirExtractor,
    source_fingerprint: String,
    functions: Vec<MirFunction>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MirExtractor {
    name: String,
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MirFunction {
    symbol: String,
    cfg: MirCfg,
    #[serde(default)]
    calls: Vec<MirCall>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MirCall {
    target: String,
    site: MirSite,
    dispatch: DispatchKind,
    #[serde(default)]
    generic: bool,
    evidence: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct MirCfg {
    basic_blocks: usize,
    edges: usize,
    exits: usize,
    loop_headers: usize,
}

impl From<MirCfg> for CfgSummary {
    fn from(value: MirCfg) -> Self {
        Self {
            basic_blocks: value.basic_blocks,
            edges: value.edges,
            exits: value.exits,
            loop_headers: value.loop_headers,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct MirSite {
    file: String,
    line_start: usize,
    column_start: usize,
    line_end: usize,
    column_end: usize,
}

impl From<MirSite> for EdgeSite {
    fn from(value: MirSite) -> Self {
        Self {
            file: value.file,
            line_start: value.line_start,
            column_start: value.column_start,
            line_end: value.line_end,
            column_end: value.column_end,
        }
    }
}

pub(super) fn prepare_and_overlay(
    code_root: &Path,
    graph_dir: &Path,
    shards_dir: &Path,
    files: &BTreeMap<String, String>,
    options: &MirBuildOptions,
    allow_stale_source: bool,
) -> Result<Option<MirApplied>, String> {
    let overlay_path = match (&options.overlay, &options.driver) {
        (Some(_), Some(_)) => {
            return Err("--mir and --mir-driver are mutually exclusive".to_string());
        }
        (Some(path), None) => path.clone(),
        (None, Some(driver)) => run_driver(code_root, graph_dir, driver)?,
        (None, None) => return Ok(None),
    };
    overlay_mir(shards_dir, files, &overlay_path, allow_stale_source).map(Some)
}

fn run_driver(code_root: &Path, graph_dir: &Path, driver: &Path) -> Result<PathBuf, String> {
    let output_path = graph_dir.join("mir-overlay.json");
    let _ = std::fs::remove_file(&output_path);
    let output = Command::new(driver)
        .arg("--code")
        .arg(code_root)
        .arg("--out")
        .arg(&output_path)
        .output()
        .map_err(|error| format!("cannot run MIR driver `{}`: {error}", driver.display()))?;
    if !output.status.success() {
        return Err(format!(
            "MIR driver `{}` failed ({}): {}",
            driver.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    if !output_path.is_file() {
        return Err(format!(
            "MIR driver `{}` exited successfully but produced no {}",
            driver.display(),
            output_path.display()
        ));
    }
    Ok(output_path)
}

fn overlay_mir(
    shards_dir: &Path,
    files: &BTreeMap<String, String>,
    overlay_path: &Path,
    allow_stale_source: bool,
) -> Result<MirApplied, String> {
    let bytes = std::fs::read(overlay_path).map_err(|error| {
        format!(
            "cannot read MIR overlay {}: {error}",
            overlay_path.display()
        )
    })?;
    let overlay_fingerprint = blake3::hash(&bytes).to_hex().to_string();
    let overlay: MirOverlay = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "cannot parse MIR overlay {}: {error}",
            overlay_path.display()
        )
    })?;
    if overlay.schema != MIR_OVERLAY_SCHEMA {
        return Err(format!(
            "MIR overlay schema `{}` is unsupported; expected `{MIR_OVERLAY_SCHEMA}`",
            overlay.schema
        ));
    }
    if overlay.extractor.name.trim().is_empty() {
        return Err("MIR overlay extractor.name must not be empty".to_string());
    }
    let current_source_fingerprint = super::status::source_fingerprint(files)
        .map_err(|error| format!("cannot fingerprint MIR source set: {error}"))?;
    if !allow_stale_source && overlay.source_fingerprint != current_source_fingerprint {
        return Err(format!(
            "MIR overlay source fingerprint {} does not match current {}",
            overlay.source_fingerprint, current_source_fingerprint
        ));
    }

    let mut shards = load_shards(shards_dir, files).map_err(|error| error.to_string())?;
    let symbols = symbol_index(&shards);
    let mut seen_functions = BTreeSet::new();
    let mut changed = BTreeSet::new();
    for function in &overlay.functions {
        if !seen_functions.insert(function.symbol.clone()) {
            return Err(format!(
                "MIR overlay contains duplicate function `{}`",
                function.symbol
            ));
        }
        validate_cfg(function)?;
        let (caller_file, caller_id, caller_kind) = resolve_unique(&symbols, &function.symbol)?;
        if caller_kind != NodeKind::Fn {
            return Err(format!(
                "MIR function `{}` resolves to {:?}, which is not a function",
                function.symbol, caller_kind
            ));
        }
        let shard = shards
            .get_mut(&caller_file)
            .ok_or_else(|| format!("MIR caller shard `{caller_file}` is missing"))?;
        let caller = shard
            .nodes
            .iter_mut()
            .find(|node| node.id == caller_id)
            .ok_or_else(|| format!("MIR caller `{}` disappeared", function.symbol))?;
        caller.cfg = Some(function.cfg.into());
        changed.insert(caller_file.clone());

        for call in &function.calls {
            validate_call(call, files)?;
            let (_, target_id, target_kind) = resolve_unique(&symbols, &call.target)?;
            if target_kind != NodeKind::Fn {
                return Err(format!(
                    "MIR call target `{}` resolves to {:?}, which is not a function",
                    call.target, target_kind
                ));
            }
            shard.edges.push(Edge {
                from: caller_id.clone(),
                to: target_id,
                target_text: Some(call.target.clone()),
                resolution: EdgeResolution::Resolved,
                kind: EdgeKind::Calls,
                provenance: Provenance::Mir,
                site: Some(call.site.clone().into()),
                extractor: Some(ExtractorIdentity {
                    name: overlay.extractor.name.clone(),
                    version: overlay.extractor.version.clone(),
                }),
                dispatch: Some(call.dispatch),
                confidence: Some(EdgeConfidence::Exact),
                candidates: Vec::new(),
                evidence: Some(call.evidence.clone()),
                generic: call.generic,
            });
        }
    }

    for rel in changed {
        let Some(shard) = shards.get_mut(&rel) else {
            continue;
        };
        shard.edges.sort();
        shard.edges.dedup();
    }

    let tool = match overlay.extractor.version.as_deref() {
        Some(version) if !version.trim().is_empty() => {
            format!("{} {version}", overlay.extractor.name)
        }
        _ => overlay.extractor.name,
    };
    let absolute = std::fs::canonicalize(overlay_path).unwrap_or_else(|_| overlay_path.into());
    Ok(MirApplied {
        tool,
        overlay_path: absolute.to_string_lossy().into_owned(),
        overlay_fingerprint,
        source_fingerprint: overlay.source_fingerprint,
        shards,
    })
}

fn validate_cfg(function: &MirFunction) -> Result<(), String> {
    if function.cfg.basic_blocks == 0 {
        return Err(format!(
            "MIR function `{}` has zero basic blocks",
            function.symbol
        ));
    }
    if function.cfg.exits > function.cfg.basic_blocks
        || function.cfg.loop_headers > function.cfg.basic_blocks
    {
        return Err(format!(
            "MIR function `{}` has an invalid CFG summary",
            function.symbol
        ));
    }
    Ok(())
}

fn validate_call(call: &MirCall, files: &BTreeMap<String, String>) -> Result<(), String> {
    if call.evidence.trim().is_empty() {
        return Err(format!("MIR call to `{}` has empty evidence", call.target));
    }
    if !files.contains_key(&call.site.file) {
        return Err(format!(
            "MIR call to `{}` references unknown site file `{}`",
            call.target, call.site.file
        ));
    }
    if call.site.line_start == 0
        || call.site.column_start == 0
        || call.site.column_end == 0
        || call.site.line_end < call.site.line_start
        || (call.site.line_end == call.site.line_start
            && call.site.column_end < call.site.column_start)
    {
        return Err(format!(
            "MIR call to `{}` has an invalid call site",
            call.target
        ));
    }
    if call.generic != (call.dispatch == DispatchKind::Generic) {
        return Err(format!(
            "MIR call to `{}` has a generic flag inconsistent with dispatch {:?}",
            call.target, call.dispatch
        ));
    }
    Ok(())
}

fn load_shards(
    shards_dir: &Path,
    files: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, Shard>, AtlasError> {
    files
        .keys()
        .map(|rel| Ok((rel.clone(), read_shard(shards_dir, rel)?)))
        .collect()
}

fn symbol_index(
    shards: &BTreeMap<String, Shard>,
) -> BTreeMap<String, Vec<(String, String, NodeKind)>> {
    let mut symbols: BTreeMap<String, Vec<(String, String, NodeKind)>> = BTreeMap::new();
    for (rel, shard) in shards {
        for node in &shard.nodes {
            symbols.entry(node.symbol.clone()).or_default().push((
                rel.clone(),
                node.id.clone(),
                node.kind,
            ));
        }
    }
    symbols
}

fn resolve_unique(
    symbols: &BTreeMap<String, Vec<(String, String, NodeKind)>>,
    symbol: &str,
) -> Result<(String, String, NodeKind), String> {
    match symbols.get(symbol).map(Vec::as_slice) {
        Some([only]) => Ok(only.clone()),
        Some(many) => Err(format!(
            "MIR symbol `{symbol}` is ambiguous across {} declarations",
            many.len()
        )),
        None => Err(format!("MIR symbol `{symbol}` is not in the Atlas graph")),
    }
}
