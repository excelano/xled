// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

//! Read and write CSV/DSV. Tolerant on input (ragged rows, flexible widths); the `csv`
//! crate handles quoting and embedded newlines. Values are kept as strings so leading
//! zeros and long IDs survive a round-trip untouched.

use crate::errors::{Result, XledError};
use crate::model::{Buffer, Edits, Layout};
use csv::ReaderBuilder;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

fn io_err(e: impl std::fmt::Display) -> XledError {
    XledError::Io(e.to_string())
}

/// Parse CSV/DSV text into a buffer. When `has_header`, the first record becomes the
/// name overlay; otherwise every record is data (columns reachable only by letter).
///
/// A blank line is a row with no fields, so row numbers match the file's lines and
/// writing the buffer back keeps the line. The `csv` crate skips blank lines, so they
/// are recovered from the gaps between record positions.
pub fn read_str(data: &str, delim: u8, has_header: bool) -> Result<Buffer> {
    let bom = data.starts_with('\u{FEFF}');
    let data = data.strip_prefix('\u{FEFF}').unwrap_or(data);

    let mut rdr = ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());

    // A record's reported position can fall short of its text, inside the line breaks
    // that end the record before it, so each start skips forward past them.
    let mut starts: Vec<usize> = Vec::new();
    let mut parsed: Vec<Vec<String>> = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(io_err)?;
        let pos = rec.position().map_or(0, |p| p.byte() as usize);
        starts.push(data.len() - data[pos..].trim_start_matches(['\r', '\n']).len());
        parsed.push(rec.iter().map(|s| s.to_string()).collect());
    }

    let mut layout = Layout {
        bom,
        ..Layout::default()
    };
    let mut verbatim = HashMap::new();
    let mut records: Vec<Vec<String>> = Vec::with_capacity(parsed.len());
    let first_start = starts.first().copied().unwrap_or(data.len());
    records.extend(std::iter::repeat_n(
        Vec::new(),
        line_breaks(&data[..first_start]),
    ));

    let mut encoded = String::new();
    let last = parsed.len().saturating_sub(1);
    for (i, fields) in parsed.into_iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(data.len());
        let text = &data[starts[i]..end];
        let raw = text.trim_end_matches(['\r', '\n']);
        let gap = &text[raw.len()..];
        if i == 0 {
            if let Some(t) = terminator(gap) {
                layout.terminator = t;
            }
        }
        encoded.clear();
        encode_into(&mut encoded, &fields, delim);
        if raw != encoded {
            verbatim.insert(encoded.clone(), raw.to_string());
        }
        records.push(fields);
        let breaks = line_breaks(gap);
        if i == last {
            layout.final_newline = breaks > 0;
        }
        records.extend(std::iter::repeat_n(Vec::new(), breaks.saturating_sub(1)));
    }
    layout.verbatim = Arc::new(verbatim);

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
        layout,
    })
}

/// The first line terminator in `gap`, if it holds one.
fn terminator(gap: &str) -> Option<&'static str> {
    if gap.starts_with("\r\n") {
        Some("\r\n")
    } else if gap.starts_with('\n') {
        Some("\n")
    } else if gap.starts_with('\r') {
        Some("\r")
    } else {
        None
    }
}

/// How many line terminators `gap` holds, counting `\r\n` as one.
fn line_breaks(gap: &str) -> usize {
    gap.matches('\n').count() + gap.matches('\r').count() - gap.matches("\r\n").count()
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
    let delim = delim.unwrap_or_else(|| default_delim(path));
    read_str(&data, delim, has_header)
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

/// Serialize the whole buffer back to CSV/DSV text (header overlay first, then data rows),
/// in the source's layout. A row still holding a record's original fields is written as
/// that record's original text.
pub fn serialize(buf: &Buffer) -> String {
    let layout = &buf.layout;
    let mut out = String::new();
    if layout.bom {
        out.push('\u{FEFF}');
    }
    let mut encoded = String::new();
    let mut lines = buf.header.iter().chain(&buf.rows).peekable();
    while let Some(row) = lines.next() {
        encoded.clear();
        encode_into(&mut encoded, row, buf.delim);
        match layout.verbatim.get(&encoded) {
            Some(raw) => out.push_str(raw),
            None => out.push_str(&encoded),
        }
        if lines.peek().is_some() || layout.final_newline {
            out.push_str(layout.terminator);
        }
    }
    out
}

/// Append one record's text, quoting only the fields that need it. A lone empty field is
/// quoted, since unquoted it would read back as a blank line.
fn encode_into(out: &mut String, row: &[String], delim: u8) {
    if let [only] = row {
        if only.is_empty() {
            out.push_str("\"\"");
            return;
        }
    }
    let delim = delim as char;
    for (i, field) in row.iter().enumerate() {
        if i > 0 {
            out.push(delim);
        }
        if field.contains([delim, '"', '\r', '\n']) {
            out.push('"');
            out.push_str(&field.replace('"', "\"\""));
            out.push('"');
        } else {
            out.push_str(field);
        }
    }
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
