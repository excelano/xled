//! The error router (errors.md): every refusal names where the capability lives.
//!
//! Boundary refusals carry their full catalog message verbatim (the messages in
//! errors.md are self-contained), so Display prints them as-is. Parse/Io get a short
//! prefix. The four refusal verbs map to the four permanent/transient destinations.

use std::fmt;

pub type Result<T> = std::result::Result<T, XledError>;

#[derive(Debug)]
pub enum XledError {
    Parse(String),
    Io(String),
    /// a different tool's job (query, not edit) → xql / DuckDB
    NotInScope(String),
    /// reshaping — changes the table's shape → upstream / in-place s///
    NotSupported(String),
    /// the data was destroyed before xled saw it → upstream re-export
    NotRecoverable(String),
    /// a real xled feature, still being designed → a later xled
    NotAvailableYet(String),
    /// legal form, unintended meaning — name what they wrote, then the intended form
    Correction(String),
}

impl fmt::Display for XledError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XledError::Parse(m) => write!(f, "parse error: {m}"),
            XledError::Io(m) => write!(f, "io error: {m}"),
            XledError::NotInScope(m)
            | XledError::NotSupported(m)
            | XledError::NotRecoverable(m)
            | XledError::NotAvailableYet(m)
            | XledError::Correction(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for XledError {}

impl From<std::io::Error> for XledError {
    fn from(e: std::io::Error) -> Self {
        XledError::Io(e.to_string())
    }
}

/// Shorthand for a parse error.
pub fn parse(msg: impl Into<String>) -> XledError {
    XledError::Parse(msg.into())
}
