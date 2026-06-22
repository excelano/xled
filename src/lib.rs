//! xled — sed and awk for tabular data.
//!
//! The library core: parse a program to statements, run them against an in-memory buffer.
//! The binary (`main.rs`) is a thin CLI/REPL over this surface; tests drive it directly.

pub mod ast;
pub mod errors;
pub mod exec;
pub mod expr;
pub mod io;
pub mod model;
pub mod parser;
pub mod resolver;
pub mod session;
pub mod subst;

pub use ast::Statement;
pub use errors::{Result, XledError};
pub use exec::Outcome;
pub use model::Buffer;

/// Parse a program (one statement per non-blank line).
pub fn parse(input: &str) -> Result<Vec<Statement>> {
    parser::parse_program(input)
}

/// Run a parsed program against a buffer, returning its `show` output and any notices.
pub fn run(buf: &mut Buffer, program: &[Statement]) -> Result<Outcome> {
    exec::run(buf, program)
}
