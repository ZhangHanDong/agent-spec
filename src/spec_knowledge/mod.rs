//! Knowledge & Liveness Layer (KLL). decisions, requirements, guidance,
//! proposals; the satisfies edge, liveness, governance lint, and MCP serve.
//!
//! The re-exports below form the module's public facade. Not every item is
//! wired into the CLI yet, so unused-import noise is silenced here — mirroring
//! the crate-wide `allow(dead_code)` for the same ahead-of-use reason.
#![allow(unused_imports)]

pub mod index;
pub mod lint;
pub mod liveness;
pub mod model;
pub mod parser;
pub mod requirement;
pub mod scaffold;
pub mod trace;

pub use index::{SatisfiesIndex, build_satisfies_index};
pub use lint::{lint_decision, lint_requirement};
pub use liveness::{decision_liveness, spec_rollup};
pub use model::{
    DecisionDoc, DecisionStatus, KSection, KnowledgeDoc, KnowledgeKind, KnowledgeMeta, Liveness,
    LivenessDeclared,
};
pub use parser::{
    parse_decision, parse_decision_str, parse_knowledge, parse_knowledge_str, parse_requirement,
    parse_requirement_str, resolve_decision_id,
};
pub use requirement::{NormativeKeyword, RequirementClause, extract_requirements};
pub use scaffold::scaffold_workspace;
pub use trace::{TraceReport, build_trace, format_trace_text, verify_spec_rollup};
