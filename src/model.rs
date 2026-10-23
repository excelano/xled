// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

//! The buffer: the in-memory table xled edits in place.
//!
//! Stringly-typed (`Vec<Vec<String>>`) so leading zeros and long IDs survive untouched.
//! The header is an overlay (name→column), promotable from any row via `N header`; it is
//! kept separate from the data rows. Ragged rows are tolerated: a missing cell reads as "".

use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct Buffer {
    /// Column-name overlay. `None` when the file has no header row.
    pub header: Option<Vec<String>>,
    /// Data rows only (the header, if any, lives in `header`).
    pub rows: Vec<Vec<String>>,
    /// Field delimiter (`,` for CSV, `\t` for TSV).
    pub delim: u8,
    /// Cells this run actually changed. Counted at `set_cell`, the single write
    /// path, so every value-level command is measured the same way and writing
    /// a cell its existing value is not reported as an edit.
    ///
    /// Structural commands — `crop`, `del`, `header`, `fill`, `drop blanks` —
    /// do not pass through `set_cell`. Comparing the buffer's dimensions before
    /// and after is what covers those.
    pub edits: Edits,
    /// How the source text was laid out, so writing it back changes only what was edited.
    pub layout: Layout,
}

/// The parts of a CSV file's text that its parsed cells do not record.
#[derive(Clone)]
pub struct Layout {
    /// A UTF-8 byte-order mark preceded the text. Excel needs it to read UTF-8.
    pub bom: bool,
    /// The line terminator: `\n`, `\r\n`, or `\r`.
    pub terminator: &'static str,
    /// The last record ended with a terminator.
    pub final_newline: bool,
    /// The original text of each record that xled would encode differently, such as one
    /// with a needlessly quoted field, keyed by xled's encoding of it. A row still holding
    /// those fields is written as it was.
    pub verbatim: Arc<HashMap<String, String>>,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            bom: false,
            terminator: "\n",
            final_newline: true,
            verbatim: Arc::default(),
        }
    }
}

/// A tally of value-level changes, for the `-i` summary.
#[derive(Clone, Default)]
pub struct Edits {
    /// Cells whose value differs from what was there before.
    pub cells: usize,
    /// The columns those cells were in, deduplicated and in column order.
    pub cols: std::collections::BTreeSet<usize>,
}

impl Buffer {
    /// Number of data rows.
    pub fn nrows(&self) -> usize {
        self.rows.len()
    }

    /// Logical width: the widest of the header and any data row.
    pub fn ncols(&self) -> usize {
        let h = self.header.as_ref().map(|h| h.len()).unwrap_or(0);
        let r = self.rows.iter().map(|r| r.len()).max().unwrap_or(0);
        h.max(r)
    }

    /// Cell value at 0-based (row, col); "" if the row is short or out of range (ragged).
    pub fn cell(&self, r: usize, c: usize) -> &str {
        self.rows
            .get(r)
            .and_then(|row| row.get(c))
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    /// The header label for a column, if a header overlay exists.
    pub fn col_name(&self, c: usize) -> Option<&str> {
        self.header
            .as_ref()
            .and_then(|h| h.get(c))
            .map(|s| s.as_str())
    }

    /// Resolve a bracketed column name to its index. Case-sensitive, exact (`[userId]` ≠ `userid`).
    pub fn name_to_col(&self, name: &str) -> Option<usize> {
        self.header.as_ref()?.iter().position(|h| h == name)
    }

    /// How many cells of row `r` hold a value.
    pub fn filled(&self, r: usize) -> usize {
        self.rows
            .get(r)
            .map_or(0, |row| row.iter().filter(|s| !s.is_empty()).count())
    }

    /// The filled-cell count most data rows share: the table's working width.
    pub fn body_width(&self) -> usize {
        let mut freq: std::collections::BTreeMap<usize, usize> = Default::default();
        for r in 0..self.nrows() {
            let w = self.filled(r);
            if w > 0 {
                *freq.entry(w).or_default() += 1;
            }
        }
        freq.iter()
            .max_by_key(|(_, &n)| n)
            .map(|(&w, _)| w)
            .unwrap_or(0)
    }

    /// When the header is a lone title in its first cell above a wider table, the
    /// 1-based file line of the first data row that reaches the table's width: where
    /// the real header most likely sits.
    pub fn title_as_header(&self) -> Option<usize> {
        let header = self.header.as_ref()?;
        let named: Vec<usize> = (0..header.len())
            .filter(|&c| !header[c].is_empty())
            .collect();
        let body = self.body_width();
        if named != [0] || body < 2 {
            return None;
        }
        (0..self.nrows())
            .find(|&r| self.filled(r) >= body)
            .map(|r| r + 2)
    }

    /// The refusal for a bracketed name the header lacks, naming a match that differs
    /// only in case.
    pub fn unknown_column(&self, name: &str) -> String {
        let folded = name.to_lowercase();
        match self
            .header
            .iter()
            .flatten()
            .find(|h| h.to_lowercase() == folded)
        {
            Some(h) => {
                format!("no column named [{name}] — names are case-sensitive: did you mean [{h}]?")
            }
            None => format!("no column named [{name}]"),
        }
    }

    /// Widen row `r` to at least `width` cells, filling with empties.
    ///
    /// Used when an assignment materializes a column: every row has to gain the field,
    /// whether or not the expression produced a value for it, or the rows disagree with
    /// the header. Not counted as an edit — an empty cell in a column that did not exist
    /// a moment ago is that column's ground state, not a change worth reporting.
    pub fn pad_row(&mut self, r: usize, width: usize) {
        if let Some(row) = self.rows.get_mut(r) {
            if row.len() < width {
                row.resize(width, String::new());
            }
        }
    }

    /// Write a cell, padding the row with empty cells if it is short (pad-on-write, rule 8).
    pub fn set_cell(&mut self, r: usize, c: usize, value: String) {
        if let Some(row) = self.rows.get_mut(r) {
            if row.len() <= c {
                row.resize(c + 1, String::new());
            }
            if row[c] != value {
                self.edits.cells += 1;
                self.edits.cols.insert(c);
            }
            row[c] = value;
        }
    }
}

/// Column letters ↔ index lives in `xaddr` now, so xled and xshape cannot drift apart on it.
pub use xaddr::col_to_letter;

/// Everything `xaddr` needs to resolve an address against this buffer.
///
/// `name_to_col` is left to the trait's default, which is the same exact, case-sensitive
/// position match the inherent method does.
impl xaddr::Grid for Buffer {
    fn nrows(&self) -> usize {
        Buffer::nrows(self)
    }

    fn ncols(&self) -> usize {
        Buffer::ncols(self)
    }

    fn header(&self) -> Option<&[String]> {
        self.header.as_deref()
    }
}
