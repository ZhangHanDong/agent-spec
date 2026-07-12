//! Knowledge & Liveness Layer (KLL). decisions, requirements, guidance,
//! proposals; the satisfies edge, liveness, governance lint, and MCP serve.
//!
//! The re-exports below form the module's public facade. Not every item is
//! wired into the CLI yet, so unused-import noise is silenced here — mirroring
//! the crate-wide `allow(dead_code)` for the same ahead-of-use reason.
#![allow(unused_imports)]

pub mod context;
pub mod draft_specs;
pub mod governance;
pub mod guidance;
pub mod index;
pub mod intake;
pub mod lint;
pub mod liveness;
pub mod model;
pub mod parser;
pub mod project;
pub mod proposal;
pub mod provenance;
pub mod questions;
pub mod requirement;
pub mod requirement_graph;
pub mod requirement_plan;
pub mod sarif;
pub mod scaffold;
pub mod test_obligations;
pub mod trace;
pub mod trace_ledger;
pub mod transitions;
pub mod work_units;
pub mod worktrees;
pub mod yaml_export;
pub mod yaml_frontend;

pub use context::{list_context, read_context, safe_join};
pub use draft_specs::{DraftSpec, draft_spec_filename, render_draft_spec};
pub use governance::{
    KnowledgeCollection, KnowledgeParseError, collect_knowledge, collect_knowledge_checked,
    lint_corpus, lint_doc,
};
pub use guidance::{applies_to, applies_to_path, applies_to_stack, glob_match, skills};
pub use index::{SatisfiesIndex, build_satisfies_index};
pub use intake::{
    RequirementImportBlock, RequirementImportError, parse_requirement_blocks,
    render_requirement_artifact, requirement_artifact_filename,
};
pub use lint::{lint_decision, lint_guidance, lint_requirement};
pub use liveness::{decision_liveness, spec_rollup};
pub use model::{
    DecisionDoc, DecisionStatus, KSection, KnowledgeDoc, KnowledgeKind, KnowledgeMeta, Liveness,
    LivenessDeclared,
};
pub use parser::{
    parse_decision, parse_decision_str, parse_knowledge, parse_knowledge_str, parse_requirement,
    parse_requirement_str, resolve_decision_id, validate_knowledge_id,
};
pub use project::{collect_guidance, collect_guidance_checked, render_guidance_md};
pub use proposal::{lint_proposal, produces};
pub use provenance::{
    ProvenanceManifest, blake3_hex, corpus_digest, verify_provenance, write_export_provenance,
    write_import_provenance,
};
pub use questions::{
    ClarificationDiagnostic, ClarificationQuestion, build_clarification_questions,
    collect_clarification_lint_diagnostics,
};
pub use requirement::{NormativeKeyword, RequirementClause, extract_requirements};
pub use requirement_graph::{
    KnowledgeParseErrorView, RequirementClauseView, RequirementGraph, RequirementGraphDiagnostic,
    RequirementNode, RequirementScenario, RequirementStep, build_requirement_graph,
    validate_requirement_graph,
};
pub use requirement_plan::{
    RequirementPlan, RequirementPlanBatch, RequirementPlanDiagnostic, RequirementPlanEdge,
    RequirementPlanEdgeKind, RequirementPlanNode, RequirementPlanSpecNode, RequirementPlanStatus,
    RequirementSpecCoverage, build_requirement_plan, validate_requirement_plan,
};
pub use sarif::{Finding, render_sarif};
pub use scaffold::scaffold_workspace;
pub use test_obligations::{
    TestObligation, TestObligationDiagnostic, TestObligationSet, build_test_obligations,
};
pub use trace::{TraceReport, build_trace, format_trace_text, verify_spec_rollup};
pub use trace_ledger::{
    RequirementFailureExplanation, RequirementTraceDiagnostic, RequirementTraceEvidence,
    RequirementTraceLedger, RequirementTraceRecord, RequirementTraceRecordInput,
    RequirementTraceRunInput, explain_requirement_failure, format_requirement_failure_text,
    format_requirement_replay_text, format_requirement_trace_mermaid,
    format_requirement_trace_text, latest_requirement_trace_records,
    read_requirement_trace_ledgers, record_requirement_trace_run, replay_requirement_trace,
    write_requirement_trace_ledger,
};
pub use transitions::{
    GovernanceError, TransitionOutcome, supersede_requirement, transition_requirement,
};
pub use work_units::{WorkUnit, WorkUnitMode, WorkUnitSet, WorkUnitStatus, build_work_units};
pub use worktrees::{WorktreeDiagnostic, WorktreeEntry, WorktreeManifest, build_worktree_manifest};
pub use yaml_export::{ExportOptions, ExportOutcome, export_requirements_yaml, write_export};
pub use yaml_frontend::{
    GeneratedRequirementDoc, YAML_PROVENANCE_KEY, import_requirements_yaml, write_generated_docs,
};
