mod keywords;
mod meta;
mod parser;
mod resolver;

pub use parser::parse_spec;
pub use parser::parse_spec_from_str;
pub use resolver::resolve_spec;
