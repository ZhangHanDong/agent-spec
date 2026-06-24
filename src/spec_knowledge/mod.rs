//! Knowledge & Liveness Layer (KLL). P1: decisions, satisfies edge, liveness.

pub mod index;
pub mod lint;
pub mod liveness;
pub mod model;
pub mod parser;

pub use model::{
    DecisionDoc, DecisionStatus, KSection, KnowledgeKind, KnowledgeMeta, Liveness, LivenessDeclared,
};
pub use index::{SatisfiesIndex, build_satisfies_index};
pub use lint::lint_decision;
pub use liveness::{decision_liveness, spec_rollup};
pub use parser::{parse_decision, parse_decision_str, resolve_decision_id};
