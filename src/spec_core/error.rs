use super::Span;

/// Unified error type for rua-spec.
#[derive(Debug, thiserror::Error)]
pub enum SpecError {
    #[error("parse error at line {}: {message}", span.start_line)]
    Parse { message: String, span: Span },

    #[error("invalid front-matter: {0}")]
    FrontMatter(String),

    #[error("inheritance error: spec '{name}' not found")]
    InheritanceNotFound { name: String },

    #[error("circular inheritance detected: {chain}")]
    CircularInheritance { chain: String },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("verification error: {0}")]
    Verification(String),
}

pub type SpecResult<T> = Result<T, SpecError>;
