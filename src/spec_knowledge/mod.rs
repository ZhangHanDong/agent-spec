//! Knowledge & Liveness Layer (KLL). decisions, requirements, guidance,
//! proposals; the satisfies edge, liveness, governance lint, and MCP serve.
//!
//! The re-exports below form the module's public facade. Not every item is
//! wired into the CLI yet, so unused-import noise is silenced here — mirroring
//! the crate-wide `allow(dead_code)` for the same ahead-of-use reason.
#![allow(unused_imports)]

pub mod context;
pub mod governance;
pub mod guidance;
pub mod index;
pub mod lint;
pub mod liveness;
pub mod model;
pub mod parser;
pub mod project;
pub mod proposal;
pub mod requirement;
pub mod sarif;
pub mod scaffold;
pub mod trace;

pub use context::{list_context, read_context, safe_join};
pub use governance::{collect_knowledge, lint_corpus, lint_doc};
pub use guidance::{applies_to, applies_to_path, applies_to_stack, glob_match, skills};
pub use index::{SatisfiesIndex, build_satisfies_index};
pub use lint::{lint_decision, lint_guidance, lint_requirement};
pub use liveness::{decision_liveness, spec_rollup};
pub use model::{
    DecisionDoc, DecisionStatus, KSection, KnowledgeDoc, KnowledgeKind, KnowledgeMeta, Liveness,
    LivenessDeclared,
};
pub use parser::{
    parse_decision, parse_decision_str, parse_knowledge, parse_knowledge_str, parse_requirement,
    parse_requirement_str, resolve_decision_id,
};
pub use project::{collect_guidance, render_guidance_md};
pub use proposal::{lint_proposal, produces};
pub use requirement::{NormativeKeyword, RequirementClause, extract_requirements};
pub use sarif::{Finding, render_sarif};
pub use scaffold::scaffold_workspace;
pub use trace::{TraceReport, build_trace, format_trace_text, verify_spec_rollup};
