//! Knowledge & Liveness Layer (KLL). P1: decisions, satisfies edge, liveness.
//!
//! The re-exports below form the module's public facade. Not every item is
//! wired into the CLI in P1 (P2 consumes more), so unused-import noise is
//! silenced here — mirroring the crate-wide `allow(dead_code)` for the same
//! ahead-of-use reason.
#![allow(unused_imports)]

pub mod index;
pub mod lint;
pub mod liveness;
pub mod model;
pub mod parser;
pub mod scaffold;
pub mod trace;

pub use index::{SatisfiesIndex, build_satisfies_index};
pub use lint::lint_decision;
pub use liveness::{decision_liveness, spec_rollup};
pub use model::{
    DecisionDoc, DecisionStatus, KSection, KnowledgeKind, KnowledgeMeta, Liveness, LivenessDeclared,
};
pub use parser::{parse_decision, parse_decision_str, resolve_decision_id};
pub use scaffold::scaffold_workspace;
pub use trace::{TraceReport, build_trace, format_trace_text, verify_spec_rollup};
