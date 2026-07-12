//! Compilation provenance manifests for the intent compiler's YAML
//! transformations. A manifest binds direction, input digest, output digests,
//! tool identity, dialect schema version, and a reproducibility result so a
//! verifier can prove the artifact chain by recomputing digests instead of
//! re-trusting the producer. Emission is opt-in and never mutates knowledge.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::intake::RequirementImportError;

pub const PROVENANCE_MANIFEST_VERSION: u32 = 1;
pub const PROVENANCE_DIALECT_SCHEMA: &str = "yaml-frontend-v1.1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DigestEntry {
    pub path: String,
    pub blake3: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProvenanceManifest {
    pub manifest_version: u32,
    pub direction: String,
    pub tool: ToolIdentity,
    pub dialect_schema: String,
    pub input: DigestEntry,
    pub outputs: Vec<DigestEntry>,
    pub reproducible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolIdentity {
    pub name: String,
    pub version: String,
}

fn err(message: String) -> RequirementImportError {
    RequirementImportError { message }
}

pub fn blake3_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn digest_file(path: &Path) -> Result<String, RequirementImportError> {
    let bytes =
        std::fs::read(path).map_err(|e| err(format!("cannot read {}: {e}", path.display())))?;
    Ok(blake3_hex(&bytes))
}

fn tool_identity() -> ToolIdentity {
    ToolIdentity {
        name: "agent-spec".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

fn require_json_target(path: &Path) -> Result<(), RequirementImportError> {
    let ok = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("json"));
    if !ok {
        return Err(err(format!(
            "provenance target must end in .json: {}",
            path.display()
        )));
    }
    Ok(())
}

/// Digest of the knowledge corpus: blake3 over sorted `(path, doc-digest)` pairs.
pub fn corpus_digest(knowledge_dir: &Path) -> Result<String, RequirementImportError> {
    let root = knowledge_dir.join("requirements");
    let mut entries: Vec<(String, String)> = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = read.filter_map(|e| e.ok().map(|e| e.path())).collect();
        paths.sort();
        for path in paths {
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                let digest = digest_file(&path)?;
                entries.push((path.to_string_lossy().replace('\\', "/"), digest));
            }
        }
    }
    entries.sort();
    let combined = entries
        .iter()
        .map(|(p, d)| format!("{p}\n{d}\n"))
        .collect::<String>();
    Ok(blake3_hex(combined.as_bytes()))
}

/// Emit an export-direction manifest. Called after the export target is
/// written; a failure here reports the manifest path and leaves outputs alone.
pub fn write_export_provenance(
    knowledge_dir: &Path,
    export_target: &Path,
    rendered_yaml: &str,
    manifest_path: &Path,
) -> Result<ProvenanceManifest, RequirementImportError> {
    require_json_target(manifest_path)?;
    let output_digest = digest_file(export_target)?;
    let reproducible = blake3_hex(rendered_yaml.as_bytes()) == output_digest;
    let manifest = ProvenanceManifest {
        manifest_version: PROVENANCE_MANIFEST_VERSION,
        direction: "export".to_string(),
        tool: tool_identity(),
        dialect_schema: PROVENANCE_DIALECT_SCHEMA.to_string(),
        input: DigestEntry {
            path: knowledge_dir.to_string_lossy().replace('\\', "/"),
            blake3: corpus_digest(knowledge_dir)?,
        },
        outputs: vec![DigestEntry {
            path: export_target.to_string_lossy().replace('\\', "/"),
            blake3: output_digest,
        }],
        reproducible,
    };
    write_manifest(manifest_path, &manifest)?;
    Ok(manifest)
}

/// Emit an import-direction manifest for the generated documents.
pub fn write_import_provenance(
    source: &Path,
    written: &[PathBuf],
    manifest_path: &Path,
) -> Result<ProvenanceManifest, RequirementImportError> {
    require_json_target(manifest_path)?;
    let mut outputs = Vec::new();
    for path in written {
        outputs.push(DigestEntry {
            path: path.to_string_lossy().replace('\\', "/"),
            blake3: digest_file(path)?,
        });
    }
    outputs.sort_by(|a, b| a.path.cmp(&b.path));
    let manifest = ProvenanceManifest {
        manifest_version: PROVENANCE_MANIFEST_VERSION,
        direction: "import".to_string(),
        tool: tool_identity(),
        dialect_schema: PROVENANCE_DIALECT_SCHEMA.to_string(),
        input: DigestEntry {
            path: source.to_string_lossy().replace('\\', "/"),
            blake3: digest_file(source)?,
        },
        outputs,
        reproducible: true,
    };
    write_manifest(manifest_path, &manifest)?;
    Ok(manifest)
}

fn write_manifest(
    path: &Path,
    manifest: &ProvenanceManifest,
) -> Result<(), RequirementImportError> {
    let mut text = serde_json::to_string_pretty(manifest).map_err(|e| err(e.to_string()))?;
    text.push('\n');
    std::fs::write(path, text)
        .map_err(|e| err(format!("cannot write provenance {}: {e}", path.display())))
}

/// Recompute every digest in the manifest; returns the drifted paths.
pub fn verify_provenance(manifest_path: &Path) -> Result<Vec<String>, RequirementImportError> {
    let text = std::fs::read_to_string(manifest_path)
        .map_err(|e| err(format!("cannot read {}: {e}", manifest_path.display())))?;
    let manifest: ProvenanceManifest =
        serde_json::from_str(&text).map_err(|e| err(e.to_string()))?;
    let mut drifted = Vec::new();
    let input_path = Path::new(&manifest.input.path);
    let input_now = if manifest.direction == "export" {
        corpus_digest(input_path)?
    } else {
        digest_file(input_path)?
    };
    if input_now != manifest.input.blake3 {
        drifted.push(manifest.input.path.clone());
    }
    for output in &manifest.outputs {
        if digest_file(Path::new(&output.path))? != output.blake3 {
            drifted.push(output.path.clone());
        }
    }
    Ok(drifted)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;

    fn make_knowledge(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("requirements")).unwrap();
        fs::write(
            dir.join("requirements/req-alpha.md"),
            "---\nkind: requirement\nid: REQ-ALPHA\ntitle: \"Alpha\"\nstatus: accepted\nliveness: auto\ntags: []\n---\n\n# Alpha\n\n## Problem\n\nAlpha problem.\n\n## Requirements\n\n[REQ-ALPHA-ONE] The system MUST do the first thing.\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn test_export_provenance_manifest_binds_digests() {
        // known blake3 vector: independent anchor so digest asserts are not
        // self-referential against a broken hash implementation
        assert_eq!(
            blake3_hex(b""),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
        let dir = make_knowledge("prov-export");
        let target = dir.join("requirements.yaml");
        let outcome = crate::spec_knowledge::write_export(
            &dir,
            &target,
            &crate::spec_knowledge::ExportOptions::default(),
            false,
        )
        .unwrap();
        let manifest_path = dir.join("requirements.compilation.json");
        let manifest =
            write_export_provenance(&dir, &target, &outcome.yaml, &manifest_path).unwrap();

        assert_eq!(manifest.direction, "export");
        assert_eq!(manifest.manifest_version, PROVENANCE_MANIFEST_VERSION);
        assert_eq!(manifest.tool.name, "agent-spec");
        assert_eq!(manifest.tool.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(manifest.dialect_schema, PROVENANCE_DIALECT_SCHEMA);
        assert!(manifest.reproducible);
        assert_eq!(manifest.input.blake3, corpus_digest(&dir).unwrap());
        assert_eq!(manifest.outputs.len(), 1);
        assert_eq!(
            manifest.outputs[0].blake3,
            blake3_hex(fs::read(&target).unwrap().as_slice())
        );
        assert!(manifest_path.exists());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_import_provenance_manifest_binds_digests() {
        let base = make_knowledge("prov-import");
        let source = base.join("source.yaml");
        fs::write(
            &source,
            "requirements:\n  - id: beta\n    title: \"Beta\"\n    type: FOLDER\n    children:\n      - id: one\n        title: \"One\"\n        type: ATOMIC\n        statement: \"The system MUST do beta one.\"\n",
        )
        .unwrap();
        let out = base.join("generated");
        let docs = crate::spec_knowledge::import_requirements_yaml(
            &fs::read_to_string(&source).unwrap(),
            "source.yaml",
        )
        .unwrap();
        let written = crate::spec_knowledge::write_generated_docs(&out, &docs).unwrap();
        let manifest_path = base.join("import.compilation.json");
        let manifest = write_import_provenance(&source, &written, &manifest_path).unwrap();

        assert_eq!(manifest.direction, "import");
        assert!(manifest.reproducible);
        assert_eq!(
            manifest.input.blake3,
            blake3_hex(&fs::read(&source).unwrap())
        );
        assert_eq!(manifest.outputs.len(), written.len());
        for output in &manifest.outputs {
            assert_eq!(
                output.blake3,
                blake3_hex(&fs::read(Path::new(&output.path)).unwrap())
            );
        }
        fs::remove_dir_all(base).ok();
    }

    #[test]
    fn test_provenance_verify_detects_drift() {
        let dir = make_knowledge("prov-verify");
        let target = dir.join("requirements.yaml");
        let outcome = crate::spec_knowledge::write_export(
            &dir,
            &target,
            &crate::spec_knowledge::ExportOptions::default(),
            false,
        )
        .unwrap();
        let manifest_path = dir.join("requirements.compilation.json");
        write_export_provenance(&dir, &target, &outcome.yaml, &manifest_path).unwrap();

        assert!(
            verify_provenance(&manifest_path).unwrap().is_empty(),
            "fresh manifest must verify clean"
        );

        let mut text = fs::read_to_string(&target).unwrap();
        text.push_str("# tampered\n");
        fs::write(&target, text).unwrap();
        let drifted = verify_provenance(&manifest_path).unwrap();
        assert!(
            drifted.iter().any(|p| p.ends_with("requirements.yaml")),
            "tampered output must be reported: {drifted:?}"
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_provenance_rejects_non_json_target() {
        let dir = make_knowledge("prov-nonjson");
        let target = dir.join("requirements.yaml");
        let outcome = crate::spec_knowledge::write_export(
            &dir,
            &target,
            &crate::spec_knowledge::ExportOptions::default(),
            false,
        )
        .unwrap();
        let bad = dir.join("manifest.yaml");
        let err = write_export_provenance(&dir, &target, &outcome.yaml, &bad).unwrap_err();
        assert!(err.to_string().contains(".json"), "{err}");
        assert!(!bad.exists());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_provenance_write_failure_keeps_outputs() {
        let dir = make_knowledge("prov-write-failure");
        let target = dir.join("requirements.yaml");
        let outcome = crate::spec_knowledge::write_export(
            &dir,
            &target,
            &crate::spec_knowledge::ExportOptions::default(),
            false,
        )
        .unwrap();
        let before = fs::read_to_string(&target).unwrap();
        let missing_dir = dir.join("no-such-dir/manifest.json");
        let err = write_export_provenance(&dir, &target, &outcome.yaml, &missing_dir).unwrap_err();
        assert!(err.to_string().contains("no-such-dir"), "{err}");
        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            before,
            "export output must remain intact"
        );
        fs::remove_dir_all(dir).ok();
    }
}
