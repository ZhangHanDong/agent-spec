//! Compilation-run provenance (manifest v2): binds the exact compiler build,
//! the effective command configuration, and input/output digests, and replays
//! a recorded compilation to prove it still reproduces byte-identical output.
//!
//! The five replayable artifact commands render through one function —
//! `render_run_artifact` — used by both the CLI `--out` path and
//! `verify_run`, so byte parity between record and replay holds by
//! construction. Replay renders in memory: it writes nothing, which is the
//! strongest form of "sandboxed to a temporary target". Per ADR-001 the
//! manifest carries deterministic facts only — no approval, authority, actor,
//! or policy fields; v1 manifests keep verifying through `verify_provenance`.

use serde::{Deserialize, Serialize};
use std::path::Path;

use super::provenance::{DigestEntry, blake3_hex, corpus_digest};

pub const RUN_MANIFEST_VERSION: u32 = 2;
pub const RUN_MANIFEST_SCHEMA_ID: &str = "agent-spec/intent-compiler/compilation-provenance-v2";

/// Build commit embedded by `build.rs`; `unknown` when the compiler was built
/// outside a git checkout. Resolved at compile time of the binary itself.
pub fn build_commit() -> &'static str {
    option_env!("AGENT_SPEC_BUILD_COMMIT").unwrap_or("unknown")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunToolIdentity {
    pub name: String,
    pub version: String,
    pub build_commit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunConfigEntry {
    pub flag: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunManifest {
    pub manifest_version: u32,
    pub schema: String,
    pub tool: RunToolIdentity,
    /// Recorded subcommand, e.g. `requirements work-units`.
    pub command: String,
    /// Effective configuration, sorted by flag.
    pub config: Vec<RunConfigEntry>,
    /// Knowledge corpus digest at record time.
    pub input: DigestEntry,
    pub outputs: Vec<DigestEntry>,
    /// Bundle digest (blake3 over sorted `path:digest` lines); compile only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_digest: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VerifyRunReport {
    pub command: String,
    pub manifest: String,
    pub drifted: Vec<String>,
}

fn config_value<'a>(config: &'a [RunConfigEntry], flag: &str) -> Result<&'a str, String> {
    config
        .iter()
        .find(|entry| entry.flag == flag)
        .map(|entry| entry.value.as_str())
        .ok_or_else(|| format!("run manifest config is missing `{flag}`"))
}

/// Render the artifact for one replayable command from its recorded config.
/// This is the single source of artifact bytes for both `--out` and replay.
pub fn render_run_artifact(command: &str, config: &[RunConfigEntry]) -> Result<String, String> {
    match command {
        "requirements graph" => {
            let knowledge = Path::new(config_value(config, "knowledge")?);
            let mut graph = crate::spec_knowledge::build_requirement_graph(knowledge);
            graph
                .diagnostics
                .extend(crate::spec_knowledge::validate_requirement_graph(&graph));
            serde_json::to_string_pretty(&graph).map_err(|e| e.to_string())
        }
        "requirements plan" => {
            let knowledge = Path::new(config_value(config, "knowledge")?);
            let specs = Path::new(config_value(config, "specs")?);
            let plan = crate::spec_knowledge::build_requirement_plan(knowledge, specs);
            serde_json::to_string_pretty(&plan).map_err(|e| e.to_string())
        }
        "requirements work-units" => {
            let knowledge = Path::new(config_value(config, "knowledge")?);
            let mut graph = crate::spec_knowledge::build_requirement_graph(knowledge);
            graph
                .diagnostics
                .extend(crate::spec_knowledge::validate_requirement_graph(&graph));
            let units = crate::spec_knowledge::build_work_units(&graph);
            serde_json::to_string_pretty(&units).map_err(|e| e.to_string())
        }
        "requirements test-obligations" => {
            let knowledge = Path::new(config_value(config, "knowledge")?);
            let specs = Path::new(config_value(config, "specs")?);
            let obligations = crate::spec_knowledge::build_test_obligations(knowledge, specs);
            serde_json::to_string_pretty(&obligations).map_err(|e| e.to_string())
        }
        "requirements traceability" => {
            let id = config_value(config, "id")?;
            let knowledge = Path::new(config_value(config, "knowledge")?);
            let specs = Path::new(config_value(config, "specs")?);
            let trace_dir = Path::new(config_value(config, "trace-dir")?);
            let projection = crate::spec_knowledge::build_traceability_projection(
                knowledge, specs, trace_dir, id,
            )?;
            crate::spec_knowledge::render_traceability_json(&projection)
        }
        other => Err(format!(
            "run provenance does not cover command `{other}`; replayable commands: \
             requirements graph | plan | work-units | test-obligations | traceability"
        )),
    }
}

/// Emit a v2 run manifest after the `--out` artifact was written.
pub fn write_run_provenance(
    command: &str,
    config: &[RunConfigEntry],
    knowledge_dir: &Path,
    out_path: &Path,
    manifest_path: &Path,
) -> Result<RunManifest, String> {
    let json_target = manifest_path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("json"));
    if !json_target {
        return Err(format!(
            "provenance target must end in .json: {}",
            manifest_path.display()
        ));
    }
    let mut config = config.to_vec();
    config.sort_by(|a, b| a.flag.cmp(&b.flag));
    let output_bytes =
        std::fs::read(out_path).map_err(|e| format!("cannot read {}: {e}", out_path.display()))?;
    let manifest = RunManifest {
        manifest_version: RUN_MANIFEST_VERSION,
        schema: RUN_MANIFEST_SCHEMA_ID.to_string(),
        tool: RunToolIdentity {
            name: "agent-spec".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            build_commit: build_commit().to_string(),
        },
        command: command.to_string(),
        config,
        input: DigestEntry {
            path: knowledge_dir.to_string_lossy().replace('\\', "/"),
            blake3: corpus_digest(knowledge_dir).map_err(|e| e.to_string())?,
        },
        outputs: vec![DigestEntry {
            path: out_path.to_string_lossy().replace('\\', "/"),
            blake3: blake3_hex(&output_bytes),
        }],
        bundle_digest: None,
    };
    let mut text = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    text.push('\n');
    std::fs::write(manifest_path, text)
        .map_err(|e| format!("cannot write provenance {}: {e}", manifest_path.display()))?;
    Ok(manifest)
}

/// Replay a recorded compilation and byte-compare against the recorded
/// digests. Renders in memory — nothing on disk is written or read beyond the
/// manifest and the compiler inputs the recorded config names.
pub fn verify_run(manifest_path: &Path) -> Result<VerifyRunReport, String> {
    let text = std::fs::read_to_string(manifest_path)
        .map_err(|e| format!("cannot read manifest {}: {e}", manifest_path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("{} is not a manifest: {e}", manifest_path.display()))?;
    let version = value
        .get("manifest_version")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if version != u64::from(RUN_MANIFEST_VERSION) {
        return Err(format!(
            "verify-run requires manifest_version {RUN_MANIFEST_VERSION}, found {version} \
             (v1 manifests verify through `verify-provenance` semantics)"
        ));
    }
    let manifest: RunManifest = serde_json::from_value(value)
        .map_err(|e| format!("{} is not a v2 run manifest: {e}", manifest_path.display()))?;
    let drifted = if manifest.command == crate::spec_knowledge::COMPILE_COMMAND {
        // Bundles have several content artifacts; replay each by its
        // bundle-relative path. The manifest artifact itself is derived from
        // the content digests, so content parity covers it.
        let fresh = crate::spec_knowledge::render_bundle_artifacts(&manifest.config)?;
        manifest
            .outputs
            .iter()
            .filter(|output| {
                fresh
                    .iter()
                    .find(|(path, _)| path == &output.path)
                    .is_none_or(|(_, content)| blake3_hex(content.as_bytes()) != output.blake3)
            })
            .map(|output| output.path.clone())
            .collect()
    } else {
        let fresh = render_run_artifact(&manifest.command, &manifest.config)?;
        let fresh_digest = blake3_hex(fresh.as_bytes());
        manifest
            .outputs
            .iter()
            .filter(|output| output.blake3 != fresh_digest)
            .map(|output| output.path.clone())
            .collect()
    };
    Ok(VerifyRunReport {
        command: manifest.command,
        manifest: manifest_path.to_string_lossy().into_owned(),
        drifted,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn make_tree(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("knowledge/requirements")).unwrap();
        fs::create_dir_all(dir.join("specs")).unwrap();
        fs::create_dir_all(dir.join("trace")).unwrap();
        fs::write(
            dir.join("knowledge/requirements/req-r.md"),
            "---\nkind: requirement\nid: REQ-RUN-R\ntitle: \"Run R\"\nstatus: accepted\nliveness: auto\ntags: []\n---\n\n# Run R\n\n## Problem\n\np\n\n## Requirements\n\n[REQ-RUN-R-ONE] The system MUST hold the first obligation.\n\n## Scenarios\n\nScenario: holds\n  Given a precondition\n  When the action runs\n  Then the outcome is observable\n",
        )
        .unwrap();
        fs::write(
            dir.join("specs/task-r.spec.md"),
            "spec: task\nname: \"R\"\nsatisfies: [REQ-RUN-R]\n---\n\n## Intent\n\nx\n\n## Boundaries\n\n### Allowed Changes\n- src/**\n\n## Completion Criteria\n\nScenario: holds\n  Test: test_holds\n  Given a precondition\n  When the action runs\n  Then the outcome is observable\n",
        )
        .unwrap();
        dir
    }

    fn config(entries: &[(&str, &Path)]) -> Vec<RunConfigEntry> {
        entries
            .iter()
            .map(|(flag, value)| RunConfigEntry {
                flag: (*flag).to_string(),
                value: value.to_string_lossy().into_owned(),
            })
            .collect()
    }

    fn record(dir: &Path, command: &str, cfg: &[RunConfigEntry], stem: &str) -> PathBuf {
        let artifact = render_run_artifact(command, cfg).unwrap();
        let out = dir.join(format!("{stem}.json"));
        fs::write(&out, &artifact).unwrap();
        let manifest = dir.join(format!("{stem}.compilation.json"));
        write_run_provenance(command, cfg, &dir.join("knowledge"), &out, &manifest).unwrap();
        manifest
    }

    #[test]
    fn test_provenance_v2_records_build_and_config() {
        let dir = make_tree("runprov-record");
        let cfg = config(&[("knowledge", &dir.join("knowledge"))]);
        let manifest_path = record(&dir, "requirements work-units", &cfg, "units");

        let manifest: RunManifest =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        assert_eq!(manifest.manifest_version, RUN_MANIFEST_VERSION);
        assert_eq!(manifest.schema, RUN_MANIFEST_SCHEMA_ID);
        assert_eq!(manifest.tool.name, "agent-spec");
        assert_eq!(manifest.tool.version, env!("CARGO_PKG_VERSION"));
        let commit = &manifest.tool.build_commit;
        assert!(
            commit == "unknown"
                || (commit.len() == 40 && commit.chars().all(|c| c.is_ascii_hexdigit())),
            "build commit must be a commit hash or the literal unknown: {commit}"
        );
        assert_eq!(manifest.command, "requirements work-units");
        assert_eq!(manifest.config.len(), 1);
        assert_eq!(manifest.config[0].flag, "knowledge");
        assert_eq!(
            manifest.input.blake3,
            corpus_digest(&dir.join("knowledge")).unwrap()
        );
        assert_eq!(manifest.outputs.len(), 1);

        let schema_file = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/intent-compiler/schemas/compilation-provenance-v2.schema.json");
        let schema_text = fs::read_to_string(&schema_file)
            .unwrap_or_else(|e| panic!("{} must exist: {e}", schema_file.display()));
        assert!(schema_text.contains(RUN_MANIFEST_SCHEMA_ID));
        for forbidden in ["actor", "authority", "approval", "policy"] {
            assert!(
                !schema_text.contains(&format!("\"{forbidden}\"")),
                "schema must not define orchestrator field '{forbidden}'"
            );
        }
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_provenance_coverage_for_out_commands() {
        let dir = make_tree("runprov-coverage");
        let knowledge = dir.join("knowledge");
        let specs = dir.join("specs");
        let trace = dir.join("trace");
        let cases: Vec<(&str, Vec<RunConfigEntry>)> = vec![
            ("requirements graph", config(&[("knowledge", &knowledge)])),
            (
                "requirements plan",
                config(&[("knowledge", &knowledge), ("specs", &specs)]),
            ),
            (
                "requirements work-units",
                config(&[("knowledge", &knowledge)]),
            ),
            (
                "requirements test-obligations",
                config(&[("knowledge", &knowledge), ("specs", &specs)]),
            ),
            ("requirements traceability", {
                let mut cfg = config(&[
                    ("knowledge", &knowledge),
                    ("specs", &specs),
                    ("trace-dir", &trace),
                ]);
                cfg.push(RunConfigEntry {
                    flag: "id".to_string(),
                    value: "REQ-RUN-R".to_string(),
                });
                cfg
            }),
        ];
        for (index, (command, cfg)) in cases.iter().enumerate() {
            let manifest_path = record(&dir, command, cfg, &format!("artifact-{index}"));
            let manifest: RunManifest =
                serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
            assert_eq!(&manifest.command, command);
            assert_eq!(manifest.input.blake3, corpus_digest(&knowledge).unwrap());
            let out_bytes = fs::read(Path::new(&manifest.outputs[0].path)).unwrap();
            assert_eq!(
                manifest.outputs[0].blake3,
                blake3_hex(&out_bytes),
                "{command} manifest must digest its artifact"
            );
        }
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_verify_run_passes_on_reproducible_outputs() {
        let dir = make_tree("runprov-replay-ok");
        let cfg = config(&[
            ("knowledge", &dir.join("knowledge")),
            ("specs", &dir.join("specs")),
        ]);
        let manifest_path = record(&dir, "requirements test-obligations", &cfg, "obligations");

        let report = verify_run(&manifest_path).unwrap();
        assert!(
            report.drifted.is_empty(),
            "an unchanged tree must replay byte-identically: {:?}",
            report.drifted
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_verify_run_fails_naming_drifted_files() {
        let dir = make_tree("runprov-replay-drift");
        let cfg = config(&[("knowledge", &dir.join("knowledge"))]);
        let manifest_path = record(&dir, "requirements graph", &cfg, "graph");

        // Edit a clause after recording: the graph artifact serializes clause
        // text, so the replay must not reproduce the recorded digest.
        let req = dir.join("knowledge/requirements/req-r.md");
        let mut text = fs::read_to_string(&req).unwrap();
        text = text.replace("first obligation", "first obligation, revised");
        fs::write(&req, text).unwrap();

        let report = verify_run(&manifest_path).unwrap();
        assert!(
            report.drifted.iter().any(|p| p.ends_with("graph.json")),
            "drifted outputs must be named: {:?}",
            report.drifted
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_verify_run_rejects_missing_manifest() {
        let dir = make_tree("runprov-missing");
        let missing = dir.join("no-such-manifest.json");
        let err = verify_run(&missing).unwrap_err();
        assert!(
            err.contains("no-such-manifest.json"),
            "diagnostic must name the path: {err}"
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_verify_provenance_still_accepts_v1_manifests() {
        let dir = make_tree("runprov-v1-compat");
        let knowledge = dir.join("knowledge");
        let target = dir.join("requirements.yaml");
        let outcome = crate::spec_knowledge::write_export(
            &knowledge,
            &target,
            &crate::spec_knowledge::ExportOptions::default(),
            false,
        )
        .unwrap();
        let manifest_path = dir.join("v1.compilation.json");
        crate::spec_knowledge::write_export_provenance(
            &knowledge,
            &target,
            &outcome.yaml,
            &manifest_path,
        )
        .unwrap();

        let drifted = crate::spec_knowledge::verify_provenance(&manifest_path).unwrap();
        assert!(drifted.is_empty(), "v1 manifests must keep verifying");

        let err = verify_run(&manifest_path).unwrap_err();
        assert!(
            err.contains("manifest_version"),
            "verify-run must direct v1 manifests to verify-provenance: {err}"
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_provenance_hardening_keeps_knowledge_byte_identical() {
        let dir = make_tree("runprov-pure");
        let req = dir.join("knowledge/requirements/req-r.md");
        let before = fs::read_to_string(&req).unwrap();

        let cfg = config(&[("knowledge", &dir.join("knowledge"))]);
        let manifest_path = record(&dir, "requirements graph", &cfg, "graph");
        verify_run(&manifest_path).unwrap();

        assert_eq!(
            fs::read_to_string(&req).unwrap(),
            before,
            "provenance emission and replay must not mutate knowledge"
        );
        fs::remove_dir_all(dir).ok();
    }
}
