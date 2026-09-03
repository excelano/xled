//! Read and write CSV/DSV. Tolerant on input (ragged rows, flexible widths); the `csv`
//! crate handles quoting and embedded newlines. Values are kept as strings so leading
//! zeros and long IDs survive a round-trip untouched.

use crate::errors::{Result, XledError};
use crate::model::{Buffer, Edits};
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

    Ok(Buffer {
        header,
        rows,
        delim,
        edits: Edits::default(),
    })
}

/// Read a file, choosing the delimiter from its extension unless one is given.
///
/// The path is opened exactly once and everything downstream works from that buffer.
/// Sniffing used to re-open it, which is invisible on a regular file and fatal on any
/// other kind: a process substitution or a piped `/dev/stdin` was drained by the sniff
/// and read back as an empty table at exit 0, and a named pipe blocked on the second
/// open waiting for a writer that had already finished.
pub fn read_file(path: &str, delim: Option<u8>, has_header: bool) -> Result<Buffer> {
    // Name the file in the error. A bare "No such file or directory" is useless
    // in a script that reads several, and the OS error alone does not carry it.
    let bytes = std::fs::read(path).map_err(|e| XledError::Io(format!("{path}: {e}")))?;
    sniff_and_warn(path, &bytes);
    // The sniff above has already warned about a non-UTF-8 encoding and named the
    // iconv fix, so this error is the fallback for bytes it could not characterize.
    let data = String::from_utf8(bytes)
        .map_err(|_| XledError::Io(format!("{path}: not valid UTF-8 text")))?;
    // UTF-8 BOM from Excel "Save as CSV UTF-8" — strip it so the first column
    // name doesn't carry a U+FEFF character.
    let trimmed = data.strip_prefix('\u{FEFF}').unwrap_or(&data);
    let delim = delim.unwrap_or_else(|| default_delim(path));
    read_str(trimmed, delim, has_header)
}

/// Sniff the already-read bytes for non-UTF-8 encodings and emit a one-line iconv
/// hint when warranted. Takes the bytes rather than the path so the input is read
/// once — see `read_file`. `path` is still needed to compose the hint.
fn sniff_and_warn(path: &str, bytes: &[u8]) {
    let s = encsniff::sniff_bytes(bytes);
    if !s.is_warning() {
        return;
    }
    // Two shapes of warning. A signature match can name the encoding; a window
    // that is merely not valid UTF-8 cannot, and saying so plainly beats the
    // old behaviour, which was to call the file usable and let the read fail
    // with "stream did not contain valid UTF-8" — a message naming neither the
    // problem nor the fix.
    match s.encoding {
        Some(enc) => eprintln!("xled: warning: {path} appears to be {enc} encoded."),
        None => eprintln!(
            "xled: warning: {path} is not valid UTF-8, and its encoding could not be identified."
        ),
    }
    // `sniff_bytes` leaves `hint` empty because it has no path to name; compose it
    // here from the same encoding verdict `sniff_file` would have used.
    let hint = match s.encoding {
        Some(enc) => encsniff::iconv_command(enc, Path::new(path)),
        None => Some(encsniff::iconv_guess_command(Path::new(path))),
    };
    if let Some(hint) = hint {
        eprintln!("hint: {hint}");
    }
}

/// Parse a `--delim` value: one ASCII character, or the escape `\t` for tab.
/// The escape earns its keep because a literal tab is awkward to type and most
/// shells swallow it; the rest of the family accepts it for the same reason.
pub fn parse_delim(s: &str) -> std::result::Result<u8, String> {
    let c = if s == "\\t" || s == "\t" {
        '\t'
    } else {
        let mut chars = s.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => c,
            _ => {
                return Err(format!(
                    "expected one character (or \\t for tab), got {s:?}"
                ))
            }
        }
    };
    if !c.is_ascii() {
        return Err(format!("expected an ASCII character, got {c:?}"));
    }
    Ok(c as u8)
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

#[cfg(test)]
mod tests {
    use super::parse_delim;

    #[test]
    fn tab_is_reachable_by_escape_and_literally() {
        assert_eq!(parse_delim("\\t"), Ok(b'\t'));
        assert_eq!(parse_delim("\t"), Ok(b'\t'));
    }

    #[test]
    fn ordinary_single_characters_pass_through() {
        assert_eq!(parse_delim(","), Ok(b','));
        assert_eq!(parse_delim("|"), Ok(b'|'));
        assert_eq!(parse_delim(";"), Ok(b';'));
    }

    #[test]
    fn multi_character_and_non_ascii_are_refused() {
        // The csv reader takes one byte, so a multi-byte char can't be a delimiter.
        assert!(parse_delim("ab").is_err());
        assert!(parse_delim("").is_err());
        assert!(parse_delim("§").is_err());
    }
}
