//! Boundary 2: the provider-neutral Code Graph IR consumer contract and the
//! `.agent-spec/code-bindings.json` artifact binding ready work units to code
//! targets. Rust Atlas is the first provider.
//!
//! Bindings are derived working data — never KLL truth: generation reads the
//! knowledge tree and the provider graph and writes only the bindings
//! artifact. A stale graph blocks definitive binding (`atlas-stale`
//! semantics): the command fails naming the stale files and writes nothing.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::provenance::blake3_hex;

pub const CODE_BINDINGS_SCHEMA_ID: &str = "agent-spec/intent-compiler/code-bindings-v1";
pub const CODE_IMPACT_SCHEMA_ID: &str = "agent-spec/intent-compiler/code-impact-v1";

/// One resolved code target inside a provider graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodeTarget {
    pub node_id: String,
    pub kind: String,
    pub file: String,
    /// Fact origin inside the provider graph (e.g. `syn`, `scip`, `mir`).
    pub provenance: String,
}

/// Provider-neutral consumer contract over a code graph: identity, staleness
/// facts, a graph fingerprint, and symbol resolution. Implementations must be
/// pure reads — binding never mutates the graph.
pub trait CodeGraphProvider {
    fn name(&self) -> &'static str;
    /// Fingerprint of the exact graph state (stable across identical graphs).
    fn fingerprint(&self) -> Result<String, String>;
    /// Files whose graph shards lag the code; non-empty blocks binding.
    fn stale_files(&self) -> Result<Vec<String>, String>;
    fn resolve(&self, symbol: &str) -> Result<CodeTarget, String>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum CodeImpactInput {
    Paths { paths: Vec<String> },
    Symbol { symbol: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodeImpactOptions {
    pub max_depth: usize,
    pub max_nodes: usize,
}

impl Default for CodeImpactOptions {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_nodes: 200,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactCodeNode {
    pub node_id: String,
    pub symbol: String,
    pub kind: String,
    pub file: String,
    pub line_start: usize,
    pub line_end: usize,
    pub provenance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactSourceSpan {
    pub file: String,
    pub line_start: usize,
    pub column_start: usize,
    pub line_end: usize,
    pub column_end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactPathHop {
    pub from: String,
    pub to: String,
    pub chosen_target: String,
    pub direction: String,
    pub kind: String,
    pub resolution: String,
    pub provenance: String,
    pub site: Option<ImpactSourceSpan>,
    pub extractor: Option<String>,
    pub extractor_version: Option<String>,
    pub dispatch: Option<String>,
    pub confidence: Option<String>,
    pub candidates: Vec<String>,
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactPath {
    pub nodes: Vec<ImpactCodeNode>,
    pub hops: Vec<ImpactPathHop>,
    pub confidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderImpactEntry {
    pub node: ImpactCodeNode,
    pub distance: usize,
    pub path: ImpactPath,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderImpactDiagnostic {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderImpact {
    pub schema: String,
    pub provider: String,
    pub graph_fingerprint: String,
    pub input: CodeImpactInput,
    pub entries: Vec<ProviderImpactEntry>,
    pub truncated: bool,
    pub diagnostics: Vec<ProviderImpactDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderImpactError {
    pub code: String,
    pub provider: String,
    pub message: String,
}

impl std::fmt::Display for ProviderImpactError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

pub trait CodeImpactProvider: CodeGraphProvider {
    fn impact(
        &self,
        input: &CodeImpactInput,
        options: &CodeImpactOptions,
    ) -> Result<ProviderImpact, ProviderImpactError>;
}

/// Rust Atlas adapter: nodes are syn-extracted facts keyed by canonical
/// symbol path; the fingerprint hashes canonical recorded authority inputs.
pub struct AtlasProvider {
    pub code_root: PathBuf,
    pub graph_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct AtlasRecordedGraphIdentity {
    repository_root: String,
    git_common_dir: Option<String>,
    worktree_root: String,
    graph_root: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct AtlasProviderFingerprintInput {
    schema_version: u32,
    recorded_graph_identity: AtlasRecordedGraphIdentity,
    toolchain_identity: String,
    recorded_source_set_fingerprint: Option<String>,
    graph_fingerprint: String,
    layer_recorded_fingerprints: BTreeMap<String, Option<String>>,
}

fn atlas_provider_fingerprint_input(
    status: &rust_atlas::AtlasStatus,
) -> AtlasProviderFingerprintInput {
    AtlasProviderFingerprintInput {
        schema_version: rust_atlas::SCHEMA_VERSION,
        recorded_graph_identity: AtlasRecordedGraphIdentity {
            repository_root: status.recorded_identity.repository_root.clone(),
            git_common_dir: status.recorded_identity.git_common_dir.clone(),
            worktree_root: status.recorded_identity.worktree_root.clone(),
            graph_root: status.recorded_identity.graph_root.clone(),
        },
        toolchain_identity: status.recorded_identity.toolchain.clone(),
        recorded_source_set_fingerprint: status.syn.recorded_fingerprint.clone(),
        graph_fingerprint: status.graph_fingerprint.clone(),
        layer_recorded_fingerprints: BTreeMap::from([
            ("mir".to_string(), status.mir.recorded_fingerprint.clone()),
            ("scip".to_string(), status.scip.recorded_fingerprint.clone()),
            ("syn".to_string(), status.syn.recorded_fingerprint.clone()),
        ]),
    }
}

fn canonical_atlas_provider_fingerprint(
    input: &AtlasProviderFingerprintInput,
) -> Result<String, String> {
    let bytes = serde_json::to_vec(input).map_err(|error| error.to_string())?;
    Ok(blake3_hex(&bytes))
}

fn atlas_provider_fingerprint(status: &rust_atlas::AtlasStatus) -> Result<String, String> {
    canonical_atlas_provider_fingerprint(&atlas_provider_fingerprint_input(status))
}

impl AtlasProvider {
    fn authoritative_status(&self) -> Result<rust_atlas::AtlasStatus, String> {
        let status = rust_atlas::status(&self.code_root, &self.graph_dir)
            .map_err(|error| error.to_string())?;
        rust_atlas::require_authority(&status).map_err(|error| error.to_string())?;
        Ok(status)
    }
}

impl CodeGraphProvider for AtlasProvider {
    fn name(&self) -> &'static str {
        "rust-atlas"
    }

    fn fingerprint(&self) -> Result<String, String> {
        let status = self.authoritative_status()?;
        atlas_provider_fingerprint(&status)
    }

    fn stale_files(&self) -> Result<Vec<String>, String> {
        Ok(self.authoritative_status()?.syn.stale_files)
    }

    fn resolve(&self, symbol: &str) -> Result<CodeTarget, String> {
        self.authoritative_status()?;
        let result = rust_atlas::query(
            &self.code_root,
            &self.graph_dir,
            symbol,
            &rust_atlas::QueryOptions { frozen: true },
        )
        .map_err(|e| e.to_string())?;
        let kind = serde_json::to_value(result.node.kind)
            .map_err(|e| e.to_string())?
            .as_str()
            .unwrap_or("unknown")
            .to_ascii_lowercase();
        Ok(CodeTarget {
            node_id: result.node.id,
            kind,
            file: result.node.file,
            provenance: "syn".to_string(),
        })
    }
}

impl CodeImpactProvider for AtlasProvider {
    fn impact(
        &self,
        input: &CodeImpactInput,
        options: &CodeImpactOptions,
    ) -> Result<ProviderImpact, ProviderImpactError> {
        let status = self.authoritative_status().map_err(|message| {
            let code = if message.contains("stale") {
                "provider-stale"
            } else {
                "provider-unavailable"
            };
            ProviderImpactError {
                code: code.into(),
                provider: self.name().into(),
                message,
            }
        })?;
        let stale_layers = [
            ("syn", &status.syn),
            ("scip", &status.scip),
            ("mir", &status.mir),
        ]
        .into_iter()
        .filter(|(_, layer)| layer.state == rust_atlas::LayerState::Stale)
        .map(|(name, _)| name)
        .collect::<Vec<_>>();
        if !stale_layers.is_empty() {
            return Err(ProviderImpactError {
                code: "provider-stale".into(),
                provider: self.name().into(),
                message: format!(
                    "rust-atlas layers are stale: {}; refresh before intent impact",
                    stale_layers.join(", ")
                ),
            });
        }
        let graph_fingerprint =
            atlas_provider_fingerprint(&status).map_err(|message| ProviderImpactError {
                code: "provider-unavailable".into(),
                provider: self.name().into(),
                message,
            })?;
        let atlas_options = rust_atlas::ImpactOptions {
            max_depth: options.max_depth,
            max_nodes: options.max_nodes,
            frozen: true,
        };
        let (mut entries, truncated, diagnostics) = match input {
            CodeImpactInput::Paths { paths } => {
                let result = rust_atlas::affected_paths(
                    &self.code_root,
                    &self.graph_dir,
                    &paths.iter().map(PathBuf::from).collect::<Vec<_>>(),
                    &rust_atlas::AffectedOptions {
                        impact: atlas_options,
                    },
                )
                .map_err(|error| provider_query_error(self.name(), error.to_string()))?;
                let mut projected = result
                    .seeds
                    .iter()
                    .flat_map(|seed| seed.nodes.iter())
                    .map(project_seed)
                    .collect::<Vec<_>>();
                projected.extend(result.affected.iter().map(project_impact_entry));
                (
                    projected,
                    result.truncated,
                    result
                        .diagnostics
                        .into_iter()
                        .map(|diagnostic| ProviderImpactDiagnostic {
                            code: diagnostic.code,
                            message: diagnostic.message,
                        })
                        .collect(),
                )
            }
            CodeImpactInput::Symbol { symbol } => {
                let result =
                    rust_atlas::impact(&self.code_root, &self.graph_dir, symbol, &atlas_options)
                        .map_err(|error| provider_query_error(self.name(), error.to_string()))?;
                let mut projected = vec![project_seed(&result.seed)];
                projected.extend(result.affected.iter().map(project_impact_entry));
                (
                    projected,
                    result.truncated,
                    result
                        .diagnostics
                        .into_iter()
                        .map(|diagnostic| ProviderImpactDiagnostic {
                            code: diagnostic.code,
                            message: diagnostic.message,
                        })
                        .collect(),
                )
            }
        };
        entries.sort_by(|left, right| {
            left.distance
                .cmp(&right.distance)
                .then_with(|| left.node.node_id.cmp(&right.node.node_id))
        });
        entries.dedup_by(|left, right| left.node.node_id == right.node.node_id);
        Ok(ProviderImpact {
            schema: CODE_IMPACT_SCHEMA_ID.into(),
            provider: self.name().into(),
            graph_fingerprint,
            input: input.clone(),
            entries,
            truncated,
            diagnostics,
        })
    }
}

fn provider_query_error(provider: &str, message: String) -> ProviderImpactError {
    ProviderImpactError {
        code: "provider-query-error".into(),
        provider: provider.into(),
        message,
    }
}

fn project_seed(node: &rust_atlas::Node) -> ProviderImpactEntry {
    let projected = project_node(node, "syn");
    ProviderImpactEntry {
        node: projected.clone(),
        distance: 0,
        path: ImpactPath {
            nodes: vec![projected],
            hops: Vec::new(),
            confidence: "exact".into(),
        },
    }
}

fn project_impact_entry(entry: &rust_atlas::ImpactEntry) -> ProviderImpactEntry {
    let provenance = entry
        .path
        .hops
        .last()
        .map(|hop| enum_name(hop.edge.provenance))
        .unwrap_or_else(|| "syn".into());
    ProviderImpactEntry {
        node: project_node(&entry.node, &provenance),
        distance: entry.distance,
        path: ImpactPath {
            nodes: entry
                .path
                .nodes
                .iter()
                .map(|node| project_node(node, "syn"))
                .collect(),
            hops: entry
                .path
                .hops
                .iter()
                .map(|hop| ImpactPathHop {
                    from: hop.edge.from.clone(),
                    to: hop.edge.to.clone(),
                    chosen_target: hop.chosen_target.clone(),
                    direction: enum_name(hop.direction),
                    kind: enum_name(hop.edge.kind),
                    resolution: enum_name(hop.edge.resolution),
                    provenance: enum_name(hop.edge.provenance),
                    site: hop.edge.site.as_ref().map(|site| ImpactSourceSpan {
                        file: site.file.clone(),
                        line_start: site.line_start,
                        column_start: site.column_start,
                        line_end: site.line_end,
                        column_end: site.column_end,
                    }),
                    extractor: hop.edge.extractor.as_ref().map(|item| item.name.clone()),
                    extractor_version: hop
                        .edge
                        .extractor
                        .as_ref()
                        .and_then(|item| item.version.clone()),
                    dispatch: hop.edge.dispatch.map(enum_name),
                    confidence: hop.edge.confidence.map(enum_name),
                    candidates: hop.edge.candidates.clone(),
                    evidence: hop.edge.evidence.clone(),
                })
                .collect(),
            confidence: enum_name(entry.path.confidence),
        },
    }
}

fn project_node(node: &rust_atlas::Node, provenance: &str) -> ImpactCodeNode {
    ImpactCodeNode {
        node_id: node.id.clone(),
        symbol: node.symbol.clone(),
        kind: enum_name(node.kind),
        file: node.file.clone(),
        line_start: node.line_start,
        line_end: node.line_end,
        provenance: provenance.into(),
    }
}

fn enum_name(value: impl Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".into())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodeBindingEntry {
    pub requirement_id: String,
    pub work_unit_id: String,
    pub provider: String,
    pub graph_fingerprint: String,
    pub targets: Vec<CodeTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBindings {
    pub schema: String,
    pub entries: Vec<CodeBindingEntry>,
}

/// A `- <provider>: <symbol>` declaration from a contract's `### Symbols`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolDeclaration {
    pub provider: String,
    pub symbol: String,
    pub spec: PathBuf,
}

/// Extract `### Symbols` declarations from one spec file. The block ends at
/// the next heading; entries are `- <provider>: <symbol-path>` lines.
pub fn extract_symbol_declarations(spec_path: &Path) -> Result<Vec<SymbolDeclaration>, String> {
    let text = std::fs::read_to_string(spec_path)
        .map_err(|e| format!("cannot read {}: {e}", spec_path.display()))?;
    let mut declarations = Vec::new();
    let mut in_block = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("### Symbols") {
            in_block = true;
            continue;
        }
        if in_block {
            if trimmed.starts_with('#') {
                in_block = false;
                continue;
            }
            let Some(entry) = trimmed.strip_prefix("- ") else {
                continue;
            };
            let Some((provider, symbol)) = entry.split_once(':') else {
                return Err(format!(
                    "{}: malformed symbol declaration `{trimmed}`; expected `- <provider>: <symbol>`",
                    spec_path.display()
                ));
            };
            declarations.push(SymbolDeclaration {
                provider: provider.trim().to_string(),
                symbol: symbol.trim().to_string(),
                spec: spec_path.to_path_buf(),
            });
        }
    }
    Ok(declarations)
}

/// Build code bindings for every ready work unit whose contracts declare
/// symbols. Fails without writing when a provider is unknown or its graph is
/// stale; knowledge documents stay byte-identical throughout.
pub fn build_code_bindings(
    knowledge: &Path,
    specs: &Path,
    providers: &BTreeMap<String, Box<dyn CodeGraphProvider>>,
) -> Result<CodeBindings, String> {
    let graph = crate::spec_knowledge::build_requirement_graph(knowledge);
    let units = crate::spec_knowledge::build_work_units(&graph);
    let index = crate::spec_knowledge::build_satisfies_index(specs);

    // Stale gate first: any used provider with a lagging graph blocks the run.
    let mut fingerprints: BTreeMap<String, String> = BTreeMap::new();
    for (name, provider) in providers {
        let stale = provider.stale_files()?;
        if !stale.is_empty() {
            return Err(format!(
                "{name}-stale: graph lags the code for {}; rebuild before binding",
                stale.join(", ")
            ));
        }
        fingerprints.insert(name.clone(), provider.fingerprint()?);
    }

    let mut entries = Vec::new();
    for unit in units
        .units
        .iter()
        .filter(|unit| unit.status == crate::spec_knowledge::WorkUnitStatus::Ready)
    {
        let mut declarations = Vec::new();
        if let Some(spec_paths) = index.get(&unit.requirement_id) {
            let mut sorted = spec_paths.clone();
            sorted.sort();
            for spec_path in sorted {
                declarations.extend(extract_symbol_declarations(&spec_path)?);
            }
        }
        if declarations.is_empty() {
            continue;
        }
        // Group targets per provider so each entry carries one fingerprint.
        let mut per_provider: BTreeMap<String, Vec<CodeTarget>> = BTreeMap::new();
        for declaration in declarations {
            let Some(provider) = providers.get(&declaration.provider) else {
                return Err(format!(
                    "unknown code graph provider `{}` declared in {}; registered providers: {}",
                    declaration.provider,
                    declaration.spec.display(),
                    providers.keys().cloned().collect::<Vec<_>>().join(", ")
                ));
            };
            per_provider
                .entry(declaration.provider.clone())
                .or_default()
                .push(provider.resolve(&declaration.symbol)?);
        }
        for (provider_name, mut targets) in per_provider {
            targets.sort_by(|a, b| a.node_id.cmp(&b.node_id));
            entries.push(CodeBindingEntry {
                requirement_id: unit.requirement_id.clone(),
                work_unit_id: unit.id.clone(),
                provider: provider_name.clone(),
                graph_fingerprint: fingerprints
                    .get(&provider_name)
                    .cloned()
                    .unwrap_or_default(),
                targets,
            });
        }
    }
    entries.sort_by(|a, b| {
        a.requirement_id
            .cmp(&b.requirement_id)
            .then_with(|| a.provider.cmp(&b.provider))
    });
    Ok(CodeBindings {
        schema: CODE_BINDINGS_SCHEMA_ID.to_string(),
        entries,
    })
}

/// Resolve typed code-target facts from a spec's `### Symbols` boundary
/// entries against a fresh Atlas graph at `<code_root>/.agent-spec/graph`.
/// Best-effort enrichment for trace evidence: any staleness, missing graph,
/// or unresolved symbol yields an empty list — the lifecycle verifier owns
/// failing those cases loudly.
pub fn collect_atlas_code_target_facts(
    sections: &[crate::spec_core::Section],
    code_root: &Path,
) -> Vec<crate::spec_knowledge::CodeTargetFact> {
    let mut symbols = Vec::new();
    for section in sections {
        let crate::spec_core::Section::Boundaries { items, .. } = section else {
            continue;
        };
        for item in items {
            if item.category != crate::spec_core::BoundaryCategory::Symbols {
                continue;
            }
            if let Some((provider, symbol)) = item.text.split_once(':')
                && provider.trim() == "rust-atlas"
            {
                symbols.push(symbol.trim().to_string());
            }
        }
    }
    if symbols.is_empty() {
        return Vec::new();
    }
    symbols.sort();
    symbols.dedup();

    let provider = AtlasProvider {
        code_root: code_root.to_path_buf(),
        graph_dir: code_root.join(".agent-spec/graph"),
    };
    match provider.stale_files() {
        Ok(stale) if stale.is_empty() => {}
        _ => return Vec::new(),
    }
    let Ok(fingerprint) = provider.fingerprint() else {
        return Vec::new();
    };
    let mut facts = Vec::new();
    for symbol in &symbols {
        let Ok(target) = provider.resolve(symbol) else {
            return Vec::new();
        };
        facts.push(crate::spec_knowledge::CodeTargetFact {
            provider: provider.name().to_string(),
            node_id: target.node_id,
            kind: target.kind,
            file: target.file,
            provenance: target.provenance,
            graph_fingerprint: fingerprint.clone(),
        });
    }
    facts
}

/// Render the bindings artifact as pretty JSON with a trailing newline.
pub fn render_code_bindings(bindings: &CodeBindings) -> Result<String, String> {
    let mut text = serde_json::to_string_pretty(bindings).map_err(|e| e.to_string())?;
    text.push('\n');
    Ok(text)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;

    fn scip_fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/atlas/scip/index.json")
    }

    /// Temp tree: a tiny cargo crate (atlas source), a built graph, a
    /// knowledge tree with one ready requirement, and a spec declaring
    /// symbols against the graph.
    fn make_tree(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("code/src")).unwrap();
        fs::write(
            dir.join("code/Cargo.toml"),
            "[package]\nname = \"bind_demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        fs::write(
            dir.join("code/src/lib.rs"),
            "pub struct SlotStore;\n\npub fn reserve() -> bool {\n    true\n}\n",
        )
        .unwrap();
        rust_atlas::build(
            &dir.join("code"),
            &dir.join("graph"),
            &rust_atlas::BuildOptions::default(),
        )
        .unwrap();

        fs::create_dir_all(dir.join("knowledge/requirements")).unwrap();
        fs::write(
            dir.join("knowledge/requirements/req-bind.md"),
            "---\nkind: requirement\nid: REQ-BIND-DEMO\ntitle: \"Bind Demo\"\nstatus: accepted\nliveness: auto\ntags: []\n---\n\n# Bind Demo\n\n## Problem\n\np\n\n## Requirements\n\n[REQ-BIND-DEMO-ONE] The system MUST reserve a slot exactly once.\n\n## Scenarios\n\nScenario: reserves\n  Given an available slot\n  When reserve runs\n  Then the slot is held\n",
        )
        .unwrap();
        fs::create_dir_all(dir.join("specs")).unwrap();
        fs::write(
            dir.join("specs/task-bind.spec.md"),
            "spec: task\nname: \"Bind Demo Contract\"\nsatisfies: [REQ-BIND-DEMO]\n---\n\n## Intent\n\nx\n\n## Boundaries\n\n### Allowed Changes\n- src/**\n\n### Symbols\n- rust-atlas: bind_demo::SlotStore\n- rust-atlas: bind_demo::reserve\n\n## Completion Criteria\n\nScenario: reserves\n  Test: test_reserves\n  Given an available slot\n  When reserve runs\n  Then the slot is held\n",
        )
        .unwrap();
        dir
    }

    fn providers(dir: &Path) -> BTreeMap<String, Box<dyn CodeGraphProvider>> {
        let mut map: BTreeMap<String, Box<dyn CodeGraphProvider>> = BTreeMap::new();
        map.insert(
            "rust-atlas".to_string(),
            Box::new(AtlasProvider {
                code_root: dir.join("code"),
                graph_dir: dir.join("graph"),
            }),
        );
        map
    }

    #[test]
    fn test_code_bindings_generate_for_ready_units() {
        let dir = make_tree("bind-ok");
        assert_eq!(
            rust_atlas::status(&dir.join("code"), &dir.join("graph"))
                .unwrap()
                .scip
                .state,
            rust_atlas::LayerState::Unavailable
        );
        let bindings =
            build_code_bindings(&dir.join("knowledge"), &dir.join("specs"), &providers(&dir))
                .unwrap();
        assert_eq!(bindings.schema, CODE_BINDINGS_SCHEMA_ID);
        assert_eq!(bindings.entries.len(), 1);
        let entry = &bindings.entries[0];
        assert_eq!(entry.requirement_id, "REQ-BIND-DEMO");
        assert!(entry.work_unit_id.starts_with("WU-"));
        assert_eq!(entry.provider, "rust-atlas");
        assert_eq!(entry.graph_fingerprint.len(), 64);
        let ids: Vec<&str> = entry.targets.iter().map(|t| t.node_id.as_str()).collect();
        assert_eq!(ids.len(), 2);
        assert!(ids[0].starts_with("bind_demo::SlotStore#"));
        assert!(ids[1].starts_with("bind_demo::reserve#"));
        assert!(entry.targets.iter().all(|t| t.provenance == "syn"));
        assert!(entry.targets.iter().all(|t| t.file == "src/lib.rs"));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_code_bindings_block_on_stale_graph() {
        let dir = make_tree("bind-stale");
        // Modify the source after the build: the shard now lags the code.
        let lib = dir.join("code/src/lib.rs");
        let mut text = fs::read_to_string(&lib).unwrap();
        text.push_str("\npub fn cancel() {}\n");
        fs::write(&lib, text).unwrap();

        let err = build_code_bindings(&dir.join("knowledge"), &dir.join("specs"), &providers(&dir))
            .unwrap_err();
        assert!(
            err.contains("stale") && err.contains("src/lib.rs"),
            "stale gate must name the lagging files: {err}"
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_atlas_provider_projects_typed_impact_and_rejects_stale_layers() {
        let dir = make_tree("typed-impact");
        let provider = AtlasProvider {
            code_root: dir.join("code"),
            graph_dir: dir.join("graph"),
        };
        let result = CodeImpactProvider::impact(
            &provider,
            &CodeImpactInput::Paths {
                paths: vec!["src/lib.rs".into()],
            },
            &CodeImpactOptions::default(),
        )
        .unwrap();
        assert_eq!(result.schema, CODE_IMPACT_SCHEMA_ID);
        assert_eq!(result.provider, "rust-atlas");
        assert!(!result.entries.is_empty());
        assert!(
            result
                .entries
                .iter()
                .all(|entry| entry.node.file == "src/lib.rs")
        );

        let source = dir.join("code/src/lib.rs");
        fs::write(&source, "pub fn changed_after_build() {}\n").unwrap();
        let error = CodeImpactProvider::impact(
            &provider,
            &CodeImpactInput::Paths {
                paths: vec!["src/lib.rs".into()],
            },
            &CodeImpactOptions::default(),
        )
        .unwrap_err();
        assert_eq!(error.code, "provider-stale");
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_code_bindings_block_on_stale_semantic_layer() {
        let dir = make_tree("bind-stale-scip");
        rust_atlas::build(
            &dir.join("code"),
            &dir.join("graph"),
            &rust_atlas::BuildOptions {
                full: false,
                scip_index: Some(scip_fixture()),
                dynamic_dispatch: false,
            },
        )
        .unwrap();
        let lib = dir.join("code/src/lib.rs");
        let mut source = fs::read_to_string(&lib).unwrap();
        source.push_str("\npub fn refreshed_syn_only() {}\n");
        fs::write(&lib, source).unwrap();
        rust_atlas::query(
            &dir.join("code"),
            &dir.join("graph"),
            "bind_demo::SlotStore",
            &rust_atlas::QueryOptions::default(),
        )
        .unwrap();
        let status = rust_atlas::status(&dir.join("code"), &dir.join("graph")).unwrap();
        assert_eq!(status.syn.state, rust_atlas::LayerState::Fresh);
        assert_eq!(status.scip.state, rust_atlas::LayerState::Stale);

        let error =
            build_code_bindings(&dir.join("knowledge"), &dir.join("specs"), &providers(&dir))
                .unwrap_err();
        assert!(error.contains("atlas-stale"), "{error}");
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_atlas_provider_fingerprint_exact_inputs_and_exclusions() {
        fn layer(recorded: Option<&str>) -> rust_atlas::LayerStatus {
            rust_atlas::LayerStatus {
                state: rust_atlas::LayerState::Fresh,
                extractor: None,
                recorded_fingerprint: recorded.map(str::to_string),
                current_fingerprint: Some("current".to_string()),
                recorded_source_fingerprint: None,
                current_source_fingerprint: None,
                stale_files: vec!["z.rs".to_string(), "a.rs".to_string()],
                diagnostics: vec!["z diagnostic".to_string(), "a diagnostic".to_string()],
            }
        }

        let recorded_identity = rust_atlas::GraphIdentity {
            repository_root: "/repo".to_string(),
            git_common_dir: Some("/repo/.git".to_string()),
            worktree_root: "/repo/worktree".to_string(),
            graph_root: "/repo/worktree/.agent-spec/graph".to_string(),
            toolchain: "rustc 1.92.0".to_string(),
        };
        let status = rust_atlas::AtlasStatus {
            graph_fingerprint: "graph-v1".to_string(),
            recorded_identity: recorded_identity.clone(),
            current_identity: recorded_identity,
            worktree_mismatch: None,
            syn: layer(Some("source-set-v1")),
            scip: layer(Some("scip-v1")),
            mir: layer(Some("mir-v1")),
        };
        let input = atlas_provider_fingerprint_input(&status);
        let expected = canonical_atlas_provider_fingerprint(&input).unwrap();
        assert_eq!(
            expected,
            "5b7597a4354fe3ae5750a56d57bae9138b9ca7513668568e86e241cd956c4147"
        );
        assert_eq!(atlas_provider_fingerprint(&status).unwrap(), expected);

        let mut excluded = status.clone();
        excluded.current_identity.worktree_root = "/other/worktree".to_string();
        excluded.current_identity.toolchain = "other current toolchain".to_string();
        excluded.worktree_mismatch = Some("different display diagnostic".to_string());
        for layer in [&mut excluded.syn, &mut excluded.scip, &mut excluded.mir] {
            layer.current_fingerprint = Some("other current fingerprint".to_string());
            layer.stale_files.reverse();
            layer.stale_files.push("new-display-file.rs".to_string());
            layer.diagnostics.reverse();
            layer.diagnostics.push("new display diagnostic".to_string());
        }
        assert_eq!(atlas_provider_fingerprint(&excluded).unwrap(), expected);

        let assert_changes = |changed: AtlasProviderFingerprintInput, label: &str| {
            assert_ne!(
                canonical_atlas_provider_fingerprint(&changed).unwrap(),
                expected,
                "{label}"
            );
        };
        let mut changed = input.clone();
        changed.schema_version += 1;
        assert_changes(changed, "schema version");
        let mut changed = input.clone();
        changed.recorded_graph_identity.worktree_root = "/other/recorded".to_string();
        assert_changes(changed, "recorded graph identity");
        let mut changed = input.clone();
        changed.toolchain_identity = "rustc other".to_string();
        assert_changes(changed, "toolchain identity");
        let mut changed = input.clone();
        changed.recorded_source_set_fingerprint = Some("source-set-v2".to_string());
        assert_changes(changed, "recorded source-set fingerprint");
        let mut changed = input.clone();
        changed.graph_fingerprint = "graph-v2".to_string();
        assert_changes(changed, "graph fingerprint");
        for layer_name in ["syn", "scip", "mir"] {
            let mut changed = input.clone();
            changed
                .layer_recorded_fingerprints
                .insert(layer_name.to_string(), Some(format!("{layer_name}-v2")));
            assert_changes(changed, layer_name);
        }
    }

    #[test]
    fn test_code_bindings_preserve_atlas_schema_mismatch_precedence() {
        let dir = make_tree("bind-schema-precedence");
        let meta_path = rust_atlas::graph_snapshot(&dir.join("graph"))
            .unwrap()
            .data_dir
            .join("meta.json");
        let mut meta: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&meta_path).unwrap()).unwrap();
        meta["schema_version"] = serde_json::json!(5);
        fs::write(&meta_path, serde_json::to_vec_pretty(&meta).unwrap()).unwrap();

        let error =
            build_code_bindings(&dir.join("knowledge"), &dir.join("specs"), &providers(&dir))
                .unwrap_err();
        assert!(error.contains("atlas-schema-mismatch"), "{error}");
        assert!(!error.contains("atlas-stale"), "{error}");
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_code_bindings_reject_unknown_provider() {
        let dir = make_tree("bind-unknown");
        let spec = dir.join("specs/task-bind.spec.md");
        let text = fs::read_to_string(&spec).unwrap().replace(
            "- rust-atlas: bind_demo::SlotStore",
            "- ghost-graph: bind_demo::SlotStore",
        );
        fs::write(&spec, text).unwrap();

        let err = build_code_bindings(&dir.join("knowledge"), &dir.join("specs"), &providers(&dir))
            .unwrap_err();
        assert!(
            err.contains("ghost-graph"),
            "diagnostic must name the unknown provider: {err}"
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_atlas_trace_target_records_provider_node_and_fingerprint() {
        let dir = make_tree("bind-trace-facts");
        // Facts resolve against the conventional graph location.
        rust_atlas::build(
            &dir.join("code"),
            &dir.join("code/.agent-spec/graph"),
            &rust_atlas::BuildOptions::default(),
        )
        .unwrap();
        let spec = dir.join("specs/task-bind.spec.md");
        let parsed = crate::spec_parser::parse_spec(&spec).unwrap();
        let req = dir.join("knowledge/requirements/req-bind.md");
        let before = fs::read_to_string(&req).unwrap();

        // Resolve typed facts from the contract symbols against the fresh graph.
        let facts = collect_atlas_code_target_facts(&parsed.sections, &dir.join("code"));
        assert_eq!(facts.len(), 2, "both declared symbols must resolve");

        // Persist trace evidence for a passing run carrying those facts.
        let report = crate::spec_core::VerificationReport::from_results(
            "Bind Demo Contract".into(),
            vec![crate::spec_core::ScenarioResult {
                scenario_name: "reserves".into(),
                verdict: crate::spec_core::Verdict::Pass,
                step_results: vec![],
                evidence: vec![],
                duration_ms: 1,
                provenance: None,
            }],
        );
        let record = crate::spec_knowledge::RequirementTraceRecord::from_parts(
            crate::spec_knowledge::RequirementTraceRecordInput {
                run_id: "run-facts".into(),
                timestamp: 1,
                requirement_id: "REQ-BIND-DEMO".into(),
                requirement_source: req.clone(),
                work_unit_id: "WU-REQ-BIND-DEMO".into(),
                spec_path: spec.clone(),
                scenario_name: "reserves".into(),
                test_selector: Some("test_reserves".into()),
                report: &report,
                worktree_path: None,
                branch: None,
                vcs: None,
                code_target_facts: facts.clone(),
            },
        )
        .unwrap();

        assert_eq!(record.code_target_facts.len(), 2);
        let fact = &record.code_target_facts[0];
        assert_eq!(fact.provider, "rust-atlas");
        assert!(fact.node_id.starts_with("bind_demo::SlotStore#"));
        assert_eq!(fact.kind, "struct");
        assert_eq!(fact.file, "src/lib.rs");
        assert_eq!(fact.provenance, "syn");
        assert_eq!(fact.graph_fingerprint.len(), 64);
        assert!(
            record
                .code_target_facts
                .iter()
                .all(|f| f.graph_fingerprint == fact.graph_fingerprint),
            "one run resolves against one graph state"
        );

        // Round-trip through the persisted ledger.
        let ledger = crate::spec_knowledge::RequirementTraceLedger {
            version: 1,
            records: vec![record],
            affected_records: Vec::new(),
            diagnostics: Vec::new(),
        };
        let text = serde_json::to_string_pretty(&ledger).unwrap();
        let reread: crate::spec_knowledge::RequirementTraceLedger =
            serde_json::from_str(&text).unwrap();
        assert_eq!(reread.records[0].code_target_facts.len(), 2);

        assert_eq!(
            fs::read_to_string(&req).unwrap(),
            before,
            "Requirement IR must remain byte-identical"
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_code_bindings_keep_requirement_ir_byte_identical() {
        let dir = make_tree("bind-pure");
        let req = dir.join("knowledge/requirements/req-bind.md");
        let before = fs::read_to_string(&req).unwrap();

        build_code_bindings(&dir.join("knowledge"), &dir.join("specs"), &providers(&dir)).unwrap();

        assert_eq!(
            fs::read_to_string(&req).unwrap(),
            before,
            "binding generation must not mutate knowledge documents"
        );
        fs::remove_dir_all(dir).ok();
    }
}
