//! `trace <decision-id>` report: satisfying specs, their verdicts, liveness.

use crate::spec_core::Verdict;
use crate::spec_knowledge::index::SatisfiesIndex;
use crate::spec_knowledge::liveness::{decision_liveness, spec_rollup};
use crate::spec_knowledge::model::{DecisionDoc, Liveness, LivenessDeclared};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct SpecVerdict {
    pub spec: PathBuf,
    pub verdict: Verdict,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceReport {
    pub decision_id: String,
    pub declared: LivenessDeclared,
    pub specs: Vec<SpecVerdict>,
    pub liveness: Liveness,
}

/// Build a trace report. `verify_fn` runs verification for one spec path and
/// returns its rolled-up verdict — injected so the builder is unit-testable
/// without invoking cargo test.
pub fn build_trace<F>(decision: &DecisionDoc, index: &SatisfiesIndex, mut verify_fn: F) -> TraceReport
where
    F: FnMut(&Path) -> Verdict,
{
    let specs: Vec<SpecVerdict> = index
        .get(&decision.meta.id)
        .map(|paths| {
            paths
                .iter()
                .map(|p| SpecVerdict {
                    spec: p.clone(),
                    verdict: verify_fn(p),
                })
                .collect()
        })
        .unwrap_or_default();

    let verdicts: Vec<Verdict> = specs.iter().map(|s| s.verdict).collect();
    let liveness = decision_liveness(decision.meta.liveness, &verdicts);

    TraceReport {
        decision_id: decision.meta.id.clone(),
        declared: decision.meta.liveness,
        specs,
        liveness,
    }
}

/// Default verify function used by the CLI: run the gateway and roll up.
pub fn verify_spec_rollup(spec_path: &Path, code_path: &Path) -> Verdict {
    match crate::spec_gateway::SpecGateway::load(spec_path) {
        Ok(gw) => match gw.verify(code_path) {
            Ok(report) => spec_rollup(&report.summary),
            Err(_) => Verdict::Uncertain,
        },
        Err(_) => Verdict::Uncertain,
    }
}

pub fn format_trace_text(r: &TraceReport) -> String {
    let mut s = format!("decision {}  liveness={:?}\n", r.decision_id, r.liveness);
    if r.specs.is_empty() {
        s.push_str("  (no spec satisfies this decision)\n");
    }
    for sv in &r.specs {
        s.push_str(&format!("  [{:?}] {}\n", sv.verdict, sv.spec.display()));
    }
    s
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_knowledge::model::{DecisionStatus, KnowledgeKind, KnowledgeMeta};
    use std::collections::BTreeMap;

    fn decision(id: &str, declared: LivenessDeclared) -> DecisionDoc {
        DecisionDoc {
            meta: KnowledgeMeta {
                kind: KnowledgeKind::Decision,
                id: id.into(),
                status: Some(DecisionStatus::Accepted),
                supersedes: None,
                liveness: declared,
            },
            sections: vec![],
            source_path: PathBuf::new(),
        }
    }

    #[test]
    fn test_trace_honored_when_all_specs_pass() {
        let mut idx: SatisfiesIndex = BTreeMap::new();
        idx.insert("ADR-001".into(), vec![PathBuf::from("specs/a.spec.md")]);
        let r = build_trace(&decision("ADR-001", LivenessDeclared::Auto), &idx, |_| {
            Verdict::Pass
        });
        assert_eq!(r.liveness, Liveness::Honored);
    }

    #[test]
    fn test_trace_unproven_when_no_satisfying_spec() {
        let idx: SatisfiesIndex = BTreeMap::new();
        let r = build_trace(&decision("ADR-002", LivenessDeclared::Auto), &idx, |_| {
            Verdict::Pass
        });
        assert_eq!(r.liveness, Liveness::Unproven);
        assert!(r.specs.is_empty());
    }

    #[test]
    fn test_trace_violated_when_a_spec_fails() {
        let mut idx: SatisfiesIndex = BTreeMap::new();
        idx.insert("ADR-003".into(), vec![PathBuf::from("specs/a.spec.md")]);
        let r = build_trace(&decision("ADR-003", LivenessDeclared::Auto), &idx, |_| {
            Verdict::Fail
        });
        assert_eq!(r.liveness, Liveness::Violated);
    }
}
