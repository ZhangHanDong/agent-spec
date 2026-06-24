//! Knowledge & Liveness Layer (KLL). P1: decisions, satisfies edge, liveness.

pub mod model;

pub use model::{
    DecisionDoc, DecisionStatus, KSection, KnowledgeKind, KnowledgeMeta, Liveness, LivenessDeclared,
};
