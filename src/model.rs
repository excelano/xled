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
            row[c] = value;
        }
    }
}

/// Column letters → 0-based index. "A"→0, "Z"→25, "AA"→26. Letters are uppercased first.
pub fn letter_to_col(s: &str) -> usize {
    let mut n: usize = 0;
    for ch in s.chars() {
        n = n * 26 + (ch.to_ascii_uppercase() as usize - 'A' as usize + 1);
    }
    n - 1
}

/// 0-based index → column letters. Inverse of [`letter_to_col`].
pub fn col_to_letter(mut c: usize) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (c % 26) as u8);
        if c < 26 {
            break;
        }
        c = c / 26 - 1;
    }
    s.reverse();
    String::from_utf8(s).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_round_trip() {
        for (i, s) in [(0, "A"), (25, "Z"), (26, "AA"), (27, "AB"), (51, "AZ"), (52, "BA")] {
            assert_eq!(letter_to_col(s), i);
            assert_eq!(col_to_letter(i), s);
        }
    }

    #[test]
    fn lowercase_letters_accepted() {
        assert_eq!(letter_to_col("c"), letter_to_col("C"));
    }
}
