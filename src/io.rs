//! Read and write CSV/DSV. Tolerant on input (ragged rows, flexible widths); the `csv`
//! crate handles quoting and embedded newlines. Values are kept as strings so leading
//! zeros and long IDs survive a round-trip untouched.

use crate::errors::{Result, XledError};
use crate::model::Buffer;
use csv::{ReaderBuilder, WriterBuilder};
use std::path::Path;

fn io_err(e: impl std::fmt::Display) -> XledError {
    XledError::Io(e.to_string())
}

/// Parse CSV/DSV text into a buffer. When `has_header`, the first record becomes the
/// name overlay; otherwise every record is data (columns reachable only by letter).
pub fn read_str(data: &str, delim: u8, has_header: bool) -> Result<Buffer> {
    let mut rdr = ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());

    let mut records: Vec<Vec<String>> = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(io_err)?;
        records.push(rec.iter().map(|s| s.to_string()).collect());
    }

    let (header, rows) = if has_header && !records.is_empty() {
        let h = records.remove(0);
        (Some(h), records)
    } else {
        (None, records)
    };

    Ok(Buffer { header, rows, delim })
}

/// Read a file, choosing the delimiter from its extension unless one is given.
pub fn read_file(path: &str, delim: Option<u8>, has_header: bool) -> Result<Buffer> {
    let data = std::fs::read_to_string(path)?;
    let delim = delim.unwrap_or_else(|| default_delim(path));
    read_str(&data, delim, has_header)
}

/// `\t` for `.tsv`, otherwise `,`.
pub fn default_delim(path: &str) -> u8 {
    match Path::new(path).extension().and_then(|e| e.to_str()) {
        Some("tsv") => b'\t',
        _ => b',',
    }
}

/// Serialize the whole buffer back to CSV/DSV text (header overlay first, then data rows).
pub fn serialize(buf: &Buffer) -> Result<String> {
    let mut wtr = WriterBuilder::new()
        .delimiter(buf.delim)
        .flexible(true)
        .from_writer(Vec::new());

    if let Some(h) = &buf.header {
        wtr.write_record(h).map_err(io_err)?;
    }
    for row in &buf.rows {
        wtr.write_record(row).map_err(io_err)?;
    }

    let bytes = wtr.into_inner().map_err(io_err)?;
    String::from_utf8(bytes).map_err(io_err)
}
