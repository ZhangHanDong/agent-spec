//! `requirements compile`: per-requirement bundles (requirement document,
//! draft task spec, traceability projection, compilation manifest) in a
//! provider-neutral default layout, with `arc-v1` as a named edge-compat
//! projection over the same content — like SARIF or SCIP, compatibility is a
//! file-layout concern, never a core-schema concern.
//!
//! Writes are atomic: every artifact for every selected requirement renders
//! and validates in memory before the first file lands; a failed render
//! writes nothing. The manifest reuses the v2 run-manifest shape and adds a
//! bundle digest (blake3 over sorted `path:digest` lines) so external
//! admission checks can pin the exact bundle.

use std::path::{Path, PathBuf};

use super::provenance::{DigestEntry, blake3_hex, corpus_digest};
use super::run_manifest::{
    RUN_MANIFEST_SCHEMA_ID, RUN_MANIFEST_VERSION, RunConfigEntry, RunManifest, RunToolIdentity,
    build_commit,
};

pub const COMPILE_COMMAND: &str = "requirements compile";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleLayout {
    AgentSpecV1,
    ArcV1,
}

impl BundleLayout {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "agent-spec-v1" => Ok(Self::AgentSpecV1),
            "arc-v1" => Ok(Self::ArcV1),
            other => Err(format!(
                "unknown layout `{other}`; accepted layouts: agent-spec-v1 | arc-v1"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::AgentSpecV1 => "agent-spec-v1",
            Self::ArcV1 => "arc-v1",
        }
    }

    /// Bundle-relative file names for one requirement id.
    fn artifact_paths(self, id: &str) -> [String; 4] {
        let id = id.to_ascii_lowercase();
        match self {
            Self::AgentSpecV1 => [
                format!("{id}/requirements.md"),
                format!("{id}/spec.md"),
                format!("{id}/traceability.json"),
                format!("{id}/compilation.json"),
            ],
            Self::ArcV1 => [
                format!("{id}.requirements.md"),
                format!("{id}.spec.md"),
                format!("{id}.arc.traceability.json"),
                format!("{id}.arc.compilation.json"),
            ],
        }
    }
}

/// One staged bundle: requirement id, (relative path, content) files, manifest.
type StagedBundle = (String, Vec<(String, String)>, RunManifest);

#[derive(Debug, Clone)]
pub struct CompiledBundle {
    pub requirement_id: String,
    /// Bundle-relative artifact paths, in layout order.
    pub files: Vec<String>,
    pub bundle_digest: String,
}

/// Render the three content artifacts (requirement doc, draft spec,
/// traceability projection) for one accepted requirement, in memory.
fn render_bundle_content(
    knowledge: &Path,
    specs: &Path,
    trace_dir: &Path,
    id: &str,
) -> Result<[String; 3], String> {
    let graph = crate::spec_knowledge::build_requirement_graph(knowledge);
    let wanted = id.trim().to_ascii_uppercase();
    let Some(node) = graph.node(&wanted) else {
        return Err(format!(
            "no requirement document under {} declares id {wanted}",
            knowledge.display()
        ));
    };
    if node.status != Some(crate::spec_knowledge::DecisionStatus::Accepted) {
        return Err(format!(
            "{wanted} is not `accepted`; compile covers accepted requirements only"
        ));
    }
    let requirement_md = std::fs::read_to_string(&node.source_path)
        .map_err(|e| format!("cannot read {}: {e}", node.source_path.display()))?;

    let units = crate::spec_knowledge::build_work_units(&graph);
    let unit = units
        .units
        .iter()
        .find(|u| u.requirement_id == wanted)
        .ok_or_else(|| format!("no work unit lowered for {wanted}"))?;
    let draft = crate::spec_knowledge::render_draft_spec(node, unit).ok_or_else(|| {
        format!(
            "{wanted} has no renderable draft spec (work unit status: {:?})",
            unit.status
        )
    })?;

    let projection =
        crate::spec_knowledge::build_traceability_projection(knowledge, specs, trace_dir, &wanted)?;
    let traceability = crate::spec_knowledge::render_traceability_json(&projection)?;

    Ok([requirement_md, draft.content, traceability])
}

/// Replay surface for `verify-run`: bundle-relative paths and their contents,
/// rendered in memory from a recorded compile config.
pub fn render_bundle_artifacts(config: &[RunConfigEntry]) -> Result<Vec<(String, String)>, String> {
    let value = |flag: &str| -> Result<&str, String> {
        config
            .iter()
            .find(|entry| entry.flag == flag)
            .map(|entry| entry.value.as_str())
            .ok_or_else(|| format!("compile manifest config is missing `{flag}`"))
    };
    let id = value("id")?;
    let layout = BundleLayout::parse(value("layout")?)?;
    let knowledge = Path::new(value("knowledge")?);
    let specs = Path::new(value("specs")?);
    let trace_dir = Path::new(value("trace-dir")?);
    let [requirement_md, spec_md, traceability] =
        render_bundle_content(knowledge, specs, trace_dir, id)?;
    let [req_path, spec_path, trace_path, _] = layout.artifact_paths(id);
    Ok(vec![
        (req_path, requirement_md),
        (spec_path, spec_md),
        (trace_path, traceability),
    ])
}

/// Compile bundles for the selected requirement ids (default: every accepted
/// requirement) into `out_dir`. Atomic across the whole run.
pub fn compile_bundles(
    knowledge: &Path,
    specs: &Path,
    trace_dir: &Path,
    out_dir: &Path,
    ids: &[String],
    layout: BundleLayout,
    force: bool,
) -> Result<Vec<CompiledBundle>, String> {
    let graph = crate::spec_knowledge::build_requirement_graph(knowledge);
    let mut selected: Vec<String> = if ids.is_empty() {
        graph
            .nodes
            .iter()
            .filter(|node| node.status == Some(crate::spec_knowledge::DecisionStatus::Accepted))
            .map(|node| node.id.clone())
            .collect()
    } else {
        ids.iter()
            .map(|id| id.trim().to_ascii_uppercase())
            .collect()
    };
    selected.sort();
    selected.dedup();
    if selected.is_empty() {
        return Err(format!(
            "no accepted requirements to compile under {}",
            knowledge.display()
        ));
    }

    // Phase 1: render everything in memory; any failure writes nothing.
    let mut staged: Vec<StagedBundle> = Vec::new();
    for id in &selected {
        let [requirement_md, spec_md, traceability] =
            render_bundle_content(knowledge, specs, trace_dir, id)?;
        let [req_path, spec_path, trace_path, manifest_path] = layout.artifact_paths(id);
        let contents = vec![
            (req_path, requirement_md),
            (spec_path, spec_md),
            (trace_path, traceability),
        ];
        let mut outputs: Vec<DigestEntry> = contents
            .iter()
            .map(|(path, content)| DigestEntry {
                path: path.clone(),
                blake3: blake3_hex(content.as_bytes()),
            })
            .collect();
        outputs.sort_by(|a, b| a.path.cmp(&b.path));
        let bundle_digest = blake3_hex(
            outputs
                .iter()
                .map(|entry| format!("{}:{}\n", entry.path, entry.blake3))
                .collect::<String>()
                .as_bytes(),
        );
        let mut config = vec![
            RunConfigEntry {
                flag: "id".to_string(),
                value: id.clone(),
            },
            RunConfigEntry {
                flag: "knowledge".to_string(),
                value: knowledge.to_string_lossy().into_owned(),
            },
            RunConfigEntry {
                flag: "layout".to_string(),
                value: layout.as_str().to_string(),
            },
            RunConfigEntry {
                flag: "specs".to_string(),
                value: specs.to_string_lossy().into_owned(),
            },
            RunConfigEntry {
                flag: "trace-dir".to_string(),
                value: trace_dir.to_string_lossy().into_owned(),
            },
        ];
        config.sort_by(|a, b| a.flag.cmp(&b.flag));
        let manifest = RunManifest {
            manifest_version: RUN_MANIFEST_VERSION,
            schema: RUN_MANIFEST_SCHEMA_ID.to_string(),
            tool: RunToolIdentity {
                name: "agent-spec".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                build_commit: build_commit().to_string(),
            },
            command: COMPILE_COMMAND.to_string(),
            config,
            input: DigestEntry {
                path: knowledge.to_string_lossy().replace('\\', "/"),
                blake3: corpus_digest(knowledge).map_err(|e| e.to_string())?,
            },
            outputs,
            bundle_digest: Some(bundle_digest),
        };
        let mut files = contents;
        let manifest_text = {
            let mut text = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
            text.push('\n');
            text
        };
        files.push((manifest_path, manifest_text));
        staged.push((id.clone(), files, manifest));
    }

    // Phase 2: collision check across every target before the first write.
    if !force {
        let colliding: Vec<String> = staged
            .iter()
            .flat_map(|(_, files, _)| files.iter())
            .filter(|(path, _)| out_dir.join(path).exists())
            .map(|(path, _)| out_dir.join(path).to_string_lossy().into_owned())
            .collect();
        if !colliding.is_empty() {
            return Err(format!(
                "refusing to overwrite existing bundle files (use --force): {}",
                colliding.join(", ")
            ));
        }
    }

    // Phase 3: write.
    let mut compiled = Vec::new();
    for (id, files, manifest) in staged {
        let mut written = Vec::new();
        for (relative, content) in &files {
            let target = out_dir.join(relative);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
            }
            std::fs::write(&target, content)
                .map_err(|e| format!("cannot write {}: {e}", target.display()))?;
            written.push(relative.clone());
        }
        compiled.push(CompiledBundle {
            requirement_id: id,
            files: written,
            bundle_digest: manifest.bundle_digest.clone().unwrap_or_default(),
        });
    }
    Ok(compiled)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::fs;

    fn make_tree(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("knowledge/requirements")).unwrap();
        fs::write(
            dir.join("knowledge/requirements/req-b.md"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/fixtures/requirements-parity/knowledge/requirements/req-parity-booking.md"
            )),
        )
        .unwrap();
        dir
    }

    fn compile(
        dir: &std::path::Path,
        out: &std::path::Path,
        layout: BundleLayout,
        force: bool,
    ) -> Result<Vec<CompiledBundle>, String> {
        compile_bundles(
            &dir.join("knowledge"),
            &dir.join("specs"),
            &dir.join("trace"),
            out,
            &[],
            layout,
            force,
        )
    }

    #[test]
    fn test_compile_emits_neutral_bundle_with_bundle_digest() {
        let dir = make_tree("compile-neutral");
        let out = dir.join("bundles");
        let compiled = compile(&dir, &out, BundleLayout::AgentSpecV1, false).unwrap();
        assert_eq!(compiled.len(), 1);

        let id = "req-parity-booking";
        for file in [
            "requirements.md",
            "spec.md",
            "traceability.json",
            "compilation.json",
        ] {
            assert!(
                out.join(id).join(file).exists(),
                "neutral bundle must contain {file}"
            );
        }
        let manifest: RunManifest = serde_json::from_str(
            &fs::read_to_string(out.join(id).join("compilation.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest.command, COMPILE_COMMAND);
        let mut lines: Vec<String> = manifest
            .outputs
            .iter()
            .map(|entry| {
                let content = fs::read(
                    out.join(id)
                        .join(std::path::Path::new(&entry.path).file_name().unwrap()),
                )
                .unwrap();
                assert_eq!(
                    entry.blake3,
                    blake3_hex(&content),
                    "artifact digest must match written bytes for {}",
                    entry.path
                );
                format!("{}:{}\n", entry.path, entry.blake3)
            })
            .collect();
        lines.sort();
        let expected = blake3_hex(lines.concat().as_bytes());
        assert_eq!(manifest.bundle_digest.as_deref(), Some(expected.as_str()));
        assert_eq!(compiled[0].bundle_digest, expected);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_compile_arc_layout_matches_reference_names() {
        let dir = make_tree("compile-arc");
        let out_a = dir.join("a");
        let out_b = dir.join("b");
        compile(&dir, &out_a, BundleLayout::ArcV1, false).unwrap();
        compile(&dir, &out_b, BundleLayout::ArcV1, false).unwrap();

        let id = "req-parity-booking";
        let names = [
            format!("{id}.requirements.md"),
            format!("{id}.spec.md"),
            format!("{id}.arc.traceability.json"),
            format!("{id}.arc.compilation.json"),
        ];
        for name in &names {
            assert!(out_a.join(name).exists(), "arc layout must emit {name}");
            assert_eq!(
                fs::read(out_a.join(name)).unwrap(),
                fs::read(out_b.join(name)).unwrap(),
                "{name} must be byte-identical across two runs"
            );
        }
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_compile_rejects_unknown_layout() {
        let err = BundleLayout::parse("nonsense-v9").unwrap_err();
        assert!(
            err.contains("agent-spec-v1") && err.contains("arc-v1"),
            "diagnostic must list accepted layouts: {err}"
        );
    }

    #[test]
    fn test_compile_refuses_overwrite_without_force() {
        let dir = make_tree("compile-overwrite");
        let out = dir.join("bundles");
        compile(&dir, &out, BundleLayout::AgentSpecV1, false).unwrap();
        let existing = out.join("req-parity-booking/requirements.md");
        let before = fs::read(&existing).unwrap();

        let err = compile(&dir, &out, BundleLayout::AgentSpecV1, false).unwrap_err();
        assert!(
            err.contains("requirements.md") && err.contains("--force"),
            "refusal must name colliding files: {err}"
        );
        assert_eq!(
            fs::read(&existing).unwrap(),
            before,
            "pre-existing files must be byte-identical after refusal"
        );

        compile(&dir, &out, BundleLayout::AgentSpecV1, true).unwrap();
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_compile_atomic_failure_leaves_no_partial_bundle() {
        let dir = make_tree("compile-atomic");
        // Second accepted requirement without scenarios: its work unit never
        // reaches ready, so the draft spec cannot render and the run fails.
        fs::write(
            dir.join("knowledge/requirements/req-noscenario.md"),
            "---\nkind: requirement\nid: REQ-PARITY-NOSCENARIO\ntitle: \"No Scenario\"\nstatus: accepted\nliveness: auto\ntags: []\n---\n\n# No Scenario\n\n## Problem\n\np\n\n## Requirements\n\n[REQ-PARITY-NOSCENARIO-ONE] The system MUST hold one obligation.\n",
        )
        .unwrap();
        let out = dir.join("bundles");
        let err = compile(&dir, &out, BundleLayout::AgentSpecV1, false).unwrap_err();
        assert!(
            err.contains("REQ-PARITY-NOSCENARIO"),
            "failure must name the requirement: {err}"
        );
        assert!(!out.exists(), "a failed compile must write nothing at all");
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_core_schemas_and_neutral_layout_carry_no_reference_token() {
        let schema_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/intent-compiler/schemas");
        let token = regex_lite_word_scan();
        for entry in fs::read_dir(&schema_dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let text = fs::read_to_string(&path).unwrap().to_ascii_lowercase();
            assert!(
                !token(&text),
                "core schema {} must not carry the reference-project token",
                path.display()
            );
        }

        let dir = make_tree("compile-vocab");
        let out = dir.join("bundles");
        compile(&dir, &out, BundleLayout::AgentSpecV1, false).unwrap();
        for file in [
            "requirements.md",
            "spec.md",
            "traceability.json",
            "compilation.json",
        ] {
            let text = fs::read_to_string(out.join("req-parity-booking").join(file))
                .unwrap()
                .to_ascii_lowercase();
            assert!(
                !token(&text),
                "neutral bundle artifact {file} must not carry the reference-project token"
            );
        }
        fs::remove_dir_all(dir).ok();
    }

    /// Word-boundary scan for the standalone reference token without a regex
    /// dependency: `arc` bounded by non-alphanumeric characters.
    fn regex_lite_word_scan() -> impl Fn(&str) -> bool {
        |text: &str| {
            let bytes = text.as_bytes();
            let needle = b"arc";
            let mut from = 0;
            while let Some(pos) = text[from..].find("arc") {
                let start = from + pos;
                let end = start + needle.len();
                let left_ok = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
                let right_ok = end == bytes.len() || !bytes[end].is_ascii_alphanumeric();
                if left_ok && right_ok {
                    return true;
                }
                from = end;
            }
            false
        }
    }

    #[test]
    fn test_parity_fixture_reference_tree_imports_cleanly() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/requirements-parity/requirements.yaml");
        let input = fs::read_to_string(&fixture).unwrap();
        let docs = crate::spec_knowledge::import_requirements_yaml(&input, "requirements.yaml")
            .expect("the reference-shaped tree must import without unsupported constructs");
        assert_eq!(docs.len(), 1);
        assert!(docs[0].content.contains("status: accepted"));
        assert!(docs[0].content.contains("## Scenarios"));
    }
}
