// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

//! What a run changed: the one-line summary `-i` prints, and the per-cell lines `--diff`
//! and the REPL's `preview` print.

use crate::model::{col_to_letter, Buffer};

/// The parts of a buffer a summary compares against. Structural commands never reach
/// `set_cell`, so the cell tally alone cannot report them.
pub struct Before {
    shape: (usize, usize),
    header: Option<Vec<String>>,
}

impl Before {
    pub fn of(buf: &Buffer) -> Self {
        Before {
            shape: (buf.nrows(), buf.ncols()),
            header: buf.header.clone(),
        }
    }
}

pub struct Summary {
    cells: usize,
    /// A bracketed header name where there is one, the column letter otherwise.
    cols: Vec<String>,
    shape_before: (usize, usize),
    shape_after: (usize, usize),
    header_changed: bool,
}

impl Summary {
    pub fn since(before: &Before, buf: &Buffer) -> Self {
        Summary {
            cells: buf.edits.cells,
            cols: buf.edits.cols.iter().map(|&c| col_label(buf, c)).collect(),
            shape_before: before.shape,
            shape_after: (buf.nrows(), buf.ncols()),
            header_changed: buf.header != before.header,
        }
    }

    /// True when the run left the table as it found it.
    pub fn is_noop(&self) -> bool {
        self.cells == 0 && self.shape_before == self.shape_after && !self.header_changed
    }

    /// What changed, as a clause: `12 cells changed in [price], now 41 rows × 8 columns
    /// (was 44 × 8)`. Empty for a no-op.
    pub fn changes(&self) -> String {
        let mut parts = Vec::new();
        if self.cells > 0 {
            let where_ = match self.cols.len() {
                0 => String::new(),
                1 => format!(" in {}", self.cols[0]),
                n if n <= 3 => format!(" across {}", self.cols.join(", ")),
                n => format!(" across {n} columns"),
            };
            parts.push(format!(
                "{} cell{} changed{where_}",
                self.cells,
                if self.cells == 1 { "" } else { "s" }
            ));
        }
        if self.shape_before != self.shape_after {
            let (br, bc) = self.shape_before;
            let (ar, ac) = self.shape_after;
            parts.push(format!(
                "now {ar} row{} × {ac} column{} (was {br} × {bc})",
                if ar == 1 { "" } else { "s" },
                if ac == 1 { "" } else { "s" }
            ));
        }
        if self.header_changed {
            parts.push("header changed".to_string());
        }
        parts.join(", ")
    }

    /// The stderr line for a run that wrote nothing, minus the `xled: ` prefix.
    pub fn preview(&self) -> String {
        if self.is_noop() {
            NOOP.to_string()
        } else {
            self.changes()
        }
    }

    /// The stderr line for a successful in-place write, minus the `xled: ` prefix.
    pub fn written(&self, path: &str, backup: Option<&str>) -> String {
        if self.is_noop() {
            return format!("{path} rewritten unchanged — {NOOP}");
        }
        let backup = backup
            .map(|b| format!(", original kept as {b}"))
            .unwrap_or_default();
        format!("wrote {path} — {}{backup}", self.changes())
    }
}

/// Does not claim the address matched nothing: a compute that wrote back the values
/// already there lands here too, and the two are indistinguishable from the outside.
const NOOP: &str = "the address matched no cells, or the script wrote the values already there";

/// One line per logged change: `C5 [price]: "$1,200" → "1200"`. The row is the one the
/// cell had when the change was made, in xled's own row numbering.
pub fn diff_lines(buf: &Buffer) -> Vec<String> {
    buf.edits
        .log
        .iter()
        .flatten()
        .map(|ch| {
            let name = buf
                .col_name(ch.col)
                .filter(|n| !n.trim().is_empty())
                .map(|n| format!(" [{n}]"))
                .unwrap_or_default();
            format!(
                "{}{}{name}: {:?} → {:?}",
                col_to_letter(ch.col),
                ch.row + 1,
                ch.old,
                ch.new
            )
        })
        .collect()
}

fn col_label(buf: &Buffer, c: usize) -> String {
    match buf.col_name(c) {
        Some(name) if !name.trim().is_empty() => format!("[{name}]"),
        _ => col_to_letter(c),
    }
}
