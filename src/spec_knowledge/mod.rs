//! Knowledge & Liveness Layer (KLL). P1: decisions, satisfies edge, liveness.

pub mod lint;
pub mod model;
pub mod parser;

pub use model::{
    DecisionDoc, DecisionStatus, KSection, KnowledgeKind, KnowledgeMeta, Liveness, LivenessDeclared,
};
pub use lint::lint_decision;
pub use parser::{parse_decision, parse_decision_str, resolve_decision_id};
