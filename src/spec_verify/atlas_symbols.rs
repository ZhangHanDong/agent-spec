//! Atlas symbol verifier — the first Intent-Code Linker slice. Task
//! Contracts reference Atlas nodes in a `### Symbols` boundary subsection
//! (`- rust-atlas: <canonical::path>`); this verifier validates every
//! reference against a fresh graph.
//!
//! Semantics: no declared symbols → the verifier is invisible (non-Rust
//! projects never need a graph). A stale or missing graph fails with
//! `atlas-stale` before any symbol lookup, so a lagging graph can never
//! produce false `atlas-symbol-missing` diagnostics. Atlas access is
//! read-only (frozen queries); the graph store is never mutated.

use crate::spec_core::{
    BoundaryCategory, Evidence, ScenarioResult, Section, SpecResult, StepVerdict, Verdict,
};
use std::path::PathBuf;

use super::{VerificationContext, Verifier};

pub struct AtlasSymbolVerifier;

const PROVIDER: &str = "rust-atlas";
pub const DIAG_SYMBOL_MISSING: &str = "atlas-symbol-missing";
pub const DIAG_STALE: &str = "atlas-stale";

fn declared_symbols(ctx: &VerificationContext) -> Vec<String> {
    let mut symbols = Vec::new();
    for section in &ctx.resolved_spec.task.sections {
        let Section::Boundaries { items, .. } = section else {
            continue;
        };
        for item in items {
            if item.category != BoundaryCategory::Symbols {
                continue;
            }
            if let Some((provider, symbol)) = item.text.split_once(':')
                && provider.trim() == PROVIDER
            {
                symbols.push(symbol.trim().to_string());
            }
        }
    }
    symbols.sort();
    symbols.dedup();
    symbols
}

fn stale_result(detail: String, locations: Vec<String>) -> ScenarioResult {
    ScenarioResult {
        scenario_name: "[atlas-symbols] contract symbols resolve in a fresh graph".into(),
        verdict: Verdict::Fail,
        step_results: vec![StepVerdict {
            step_text: "graph freshness".into(),
            verdict: Verdict::Fail,
            reason: detail,
        }],
        evidence: vec![Evidence::PatternMatch {
            pattern: DIAG_STALE.into(),
            matched: true,
            locations,
        }],
        duration_ms: 0,
        provenance: None,
    }
}

impl Verifier for AtlasSymbolVerifier {
    fn name(&self) -> &str {
        "atlas-symbols"
    }

    fn verify(&self, ctx: &VerificationContext) -> SpecResult<Vec<ScenarioResult>> {
        let symbols = declared_symbols(ctx);
        if symbols.is_empty() {
            return Ok(Vec::new());
        }
        let code_root: PathBuf = ctx
            .code_paths
            .first()
            .cloned()
            .unwrap_or_else(|| PathBuf::from("."));
        let graph_dir = code_root.join(".agent-spec/graph");

        // Freshness gate first: a lagging (or absent) graph must never
        // produce false symbol-missing diagnostics.
        if !graph_dir.is_dir() {
            return Ok(vec![stale_result(
                format!(
                    "{DIAG_STALE}: no graph at {}; run `agent-spec atlas build` before validating symbols",
                    graph_dir.display()
                ),
                vec![graph_dir.to_string_lossy().into_owned()],
            )]);
        }
        let stale = match rust_atlas::check(&code_root, &graph_dir) {
            Ok(stale) => stale,
            Err(error) => {
                return Ok(vec![stale_result(
                    format!("{DIAG_STALE}: cannot check graph freshness: {error}"),
                    Vec::new(),
                )]);
            }
        };
        if !stale.is_empty() {
            return Ok(vec![stale_result(
                format!(
                    "{DIAG_STALE}: graph lags the code for {}; rebuild before validating symbols",
                    stale.join(", ")
                ),
                stale,
            )]);
        }

        let mut step_results = Vec::new();
        let mut evidence = Vec::new();
        let mut missing = Vec::new();
        for symbol in &symbols {
            match rust_atlas::query(
                &code_root,
                &graph_dir,
                symbol,
                &rust_atlas::QueryOptions { frozen: true },
            ) {
                Ok(_) => step_results.push(StepVerdict {
                    step_text: symbol.clone(),
                    verdict: Verdict::Pass,
                    reason: "resolved in fresh graph".into(),
                }),
                Err(error) => {
                    missing.push(symbol.clone());
                    step_results.push(StepVerdict {
                        step_text: symbol.clone(),
                        verdict: Verdict::Fail,
                        reason: format!("{DIAG_SYMBOL_MISSING}: {error}"),
                    });
                }
            }
        }
        if missing.is_empty() {
            // Every reference resolved against a fresh graph: stay silent so
            // symbol-free reports keep their exact shape.
            return Ok(Vec::new());
        }
        evidence.push(Evidence::PatternMatch {
            pattern: DIAG_SYMBOL_MISSING.into(),
            matched: true,
            locations: missing,
        });
        Ok(vec![ScenarioResult {
            scenario_name: "[atlas-symbols] contract symbols resolve in a fresh graph".into(),
            verdict: Verdict::Fail,
            step_results,
            evidence,
            duration_ms: 0,
            provenance: None,
        }])
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_gateway::SpecGateway;
    use std::fs;
    use std::path::Path;

    fn make_code(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("code/src")).unwrap();
        fs::write(
            dir.join("code/Cargo.toml"),
            "[package]\nname = \"linker_demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        fs::write(
            dir.join("code/src/lib.rs"),
            "pub struct SlotStore;\n\npub fn reserve() -> bool {\n    true\n}\n",
        )
        .unwrap();
        dir
    }

    fn build_graph(dir: &Path) {
        rust_atlas::build(
            &dir.join("code"),
            &dir.join("code/.agent-spec/graph"),
            &rust_atlas::BuildOptions::default(),
        )
        .unwrap();
    }

    fn write_spec(dir: &Path, symbols: &[&str]) -> std::path::PathBuf {
        let mut boundaries = String::from("## Boundaries\n\n### Allowed Changes\n- src/**\n");
        if !symbols.is_empty() {
            boundaries.push_str("\n### Symbols\n");
            for symbol in symbols {
                boundaries.push_str(&format!("- rust-atlas: {symbol}\n"));
            }
        }
        let spec = dir.join("task.spec.md");
        fs::write(
            &spec,
            format!("spec: task\nname: \"Linker Demo\"\n---\n\n## Intent\n\nx\n\n{boundaries}"),
        )
        .unwrap();
        spec
    }

    fn atlas_results(report: &crate::spec_core::VerificationReport) -> Vec<String> {
        report
            .results
            .iter()
            .filter(|r| r.scenario_name.starts_with("[atlas-symbols]"))
            .flat_map(|r| {
                r.evidence.iter().filter_map(|e| match e {
                    Evidence::PatternMatch { pattern, .. } => Some(pattern.clone()),
                    _ => None,
                })
            })
            .collect()
    }

    #[test]
    fn test_lifecycle_reports_missing_atlas_contract_symbol() {
        let dir = make_code("linker-missing");
        build_graph(&dir);
        let spec = write_spec(&dir, &["linker_demo::GhostType"]);
        let gateway = SpecGateway::load(&spec).unwrap();
        let report = gateway.verify(dir.join("code")).unwrap();
        let patterns = atlas_results(&report);
        assert!(
            patterns.iter().any(|p| p == DIAG_SYMBOL_MISSING),
            "missing symbol must surface {DIAG_SYMBOL_MISSING}: {patterns:?}"
        );
        assert!(!gateway.is_passing(&report));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_lifecycle_reports_stale_atlas_graph() {
        let dir = make_code("linker-stale");
        build_graph(&dir);
        // Modify the source after the build: the graph now lags.
        let lib = dir.join("code/src/lib.rs");
        let mut text = fs::read_to_string(&lib).unwrap();
        text.push_str("\npub fn cancel() {}\n");
        fs::write(&lib, text).unwrap();

        let spec = write_spec(&dir, &["linker_demo::GhostType"]);
        let gateway = SpecGateway::load(&spec).unwrap();
        let report = gateway.verify(dir.join("code")).unwrap();
        let patterns = atlas_results(&report);
        assert!(
            patterns.iter().any(|p| p == DIAG_STALE),
            "stale graph must surface {DIAG_STALE}: {patterns:?}"
        );
        assert!(
            !patterns.iter().any(|p| p == DIAG_SYMBOL_MISSING),
            "a stale graph must not produce false symbol-missing diagnostics"
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_lifecycle_accepts_valid_atlas_contract_symbols() {
        let dir = make_code("linker-valid");
        build_graph(&dir);
        let spec = write_spec(&dir, &["linker_demo::SlotStore", "linker_demo::reserve"]);
        let gateway = SpecGateway::load(&spec).unwrap();
        let report = gateway.verify(dir.join("code")).unwrap();
        assert!(
            atlas_results(&report).is_empty(),
            "valid references must emit no atlas diagnostics"
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_non_rust_lifecycle_without_atlas_symbols_does_not_require_graph() {
        let dir = make_code("linker-nonrust");
        // No graph built, no Symbols declared: the verifier must be invisible.
        let spec = write_spec(&dir, &[]);
        let gateway = SpecGateway::load(&spec).unwrap();
        let report = gateway.verify(dir.join("code")).unwrap();
        assert!(
            atlas_results(&report).is_empty(),
            "a contract without symbols must not require a graph"
        );
        fs::remove_dir_all(dir).ok();
    }
}
