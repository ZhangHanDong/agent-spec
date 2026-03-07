mod brief;
mod lifecycle;

#[allow(deprecated)]
pub use brief::SpecBrief;
pub use brief::TaskContract;
pub use lifecycle::SpecGateway;
pub use crate::spec_verify::{AiBackend, AiMode};
