//! The buffer: the in-memory table xled edits in place.
//!
//! Stringly-typed (`Vec<Vec<String>>`) so leading zeros and long IDs survive untouched.
//! The header is an overlay (name→column), promotable from any row via `header N`; it is
//! kept separate from the data rows. Ragged rows are tolerated: a missing cell reads as "".

/// Column-letter ↔ index is bijective base-26: A=0, Z=25, AA=26, …

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
        self.header.as_ref().and_then(|h| h.get(c)).map(|s| s.as_str())
    }

    /// Resolve a bracketed column name to its index. Case-sensitive, exact (`[userId]` ≠ `userid`).
    pub fn name_to_col(&self, name: &str) -> Option<usize> {
        self.header.as_ref()?.iter().position(|h| h == name)
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
