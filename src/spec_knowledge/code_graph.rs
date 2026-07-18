//! Boundary 2: the provider-neutral Code Graph IR consumer contract and the
//! `.agent-spec/code-bindings.json` artifact binding ready work units to code
//! targets. Rust Atlas is the first provider.
//!
//! Bindings are derived working data — never KLL truth: generation reads the
//! knowledge tree and the provider graph and writes only the bindings
//! artifact. A stale graph blocks definitive binding (`atlas-stale`
//! semantics): the command fails naming the stale files and writes nothing.

use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::provenance::blake3_hex;

pub const CODE_BINDINGS_SCHEMA_ID: &str = "agent-spec/intent-compiler/code-bindings-v1";

/// One resolved code target inside a provider graph.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
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

/// Rust Atlas adapter: nodes are syn-extracted facts keyed by canonical
/// symbol path; the fingerprint hashes the graph's `file -> shard hash` map.
pub struct AtlasProvider {
    pub code_root: PathBuf,
    pub graph_dir: PathBuf,
}

impl CodeGraphProvider for AtlasProvider {
    fn name(&self) -> &'static str {
        "rust-atlas"
    }

    fn fingerprint(&self) -> Result<String, String> {
        let (meta, _) = rust_atlas::load_graph(&self.graph_dir).map_err(|e| e.to_string())?;
        let combined = meta
            .files
            .iter()
            .map(|(file, hash)| format!("{file}:{hash}\n"))
            .collect::<String>();
        Ok(blake3_hex(combined.as_bytes()))
    }

    fn stale_files(&self) -> Result<Vec<String>, String> {
        rust_atlas::check(&self.code_root, &self.graph_dir).map_err(|e| e.to_string())
    }

    fn resolve(&self, symbol: &str) -> Result<CodeTarget, String> {
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CodeBindingEntry {
    pub requirement_id: String,
    pub work_unit_id: String,
    pub provider: String,
    pub graph_fingerprint: String,
    pub targets: Vec<CodeTarget>,
}

#[derive(Debug, Clone, Serialize)]
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
