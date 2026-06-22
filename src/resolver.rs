//! Resolve a `Reference` against a buffer to a set of cells (0-based `(row, col)`).
//!
//! The address algebra is set algebra over cells: union/intersect/negate combine cell sets,
//! ranges expand to rectangles or full row/column spans, `/re/` selects whole matching rows.
//! A `BTreeSet` keeps the set in row-major order for natural rendering.

use crate::ast::*;
use crate::errors::{parse, Result, XledError};
use crate::expr::{self, EvalErr, Value};
use crate::model::Buffer;
use regex::Regex;
use std::collections::BTreeSet;

pub type CellSet = BTreeSet<(usize, usize)>;

/// Every cell in the table — the scope of a bare command.
pub fn full_table(buf: &Buffer) -> CellSet {
    let mut set = CellSet::new();
    // ncols() scans every row for the widest; hoist it so this stays O(rows·cols), not O(rows²).
    let ncols = buf.ncols();
    for r in 0..buf.nrows() {
        for c in 0..ncols {
            set.insert((r, c));
        }
    }
    set
}

pub fn resolve(buf: &Buffer, r: &Reference) -> Result<CellSet> {
    match r {
        Reference::Union(parts) => {
            let mut set = CellSet::new();
            for p in parts {
                set.extend(resolve(buf, p)?);
            }
            Ok(set)
        }
        Reference::Intersect(parts) => {
            let mut iter = parts.iter();
            let mut set = resolve(buf, iter.next().unwrap())?;
            for p in iter {
                let other = resolve(buf, p)?;
                set = set.intersection(&other).copied().collect();
            }
            Ok(set)
        }
        Reference::Negate(inner) => {
            let inner = resolve(buf, inner)?;
            Ok(full_table(buf)
                .into_iter()
                .filter(|cell| !inner.contains(cell))
                .collect())
        }
        Reference::RegexSel { body, ci } => resolve_regex(buf, body, *ci),
        Reference::ColRegexSel {
            col,
            neg,
            body,
            ci,
        } => resolve_col_regex(buf, col, *neg, body, *ci),
        Reference::Comparison(e) => resolve_comparison(buf, e),
        Reference::Range(rr) => resolve_range(buf, rr),
    }
}

/// A comparison as scope: select whole rows where the bool-valued expr is true.
/// A cast failure on a row just leaves it unselected (lenient).
fn resolve_comparison(buf: &Buffer, e: &Expr) -> Result<CellSet> {
    let ncols = buf.ncols();
    let mut set = CellSet::new();
    for r in 0..buf.nrows() {
        match expr::eval(buf, r, e) {
            Ok(Value::Bool(true)) => {
                for c in 0..ncols {
                    set.insert((r, c));
                }
            }
            Ok(_) => {}
            Err(EvalErr::Cast) => {}
            Err(EvalErr::Hard(err)) => return Err(err),
        }
    }
    Ok(set)
}

fn resolve_col_regex(buf: &Buffer, col: &str, neg: bool, body: &str, ci: bool) -> Result<CellSet> {
    let c = buf
        .name_to_col(col)
        .ok_or_else(|| XledError::Correction(format!("no column named [{col}]")))?;
    let re = compile(body, ci)?;
    let ncols = buf.ncols();
    let mut set = CellSet::new();
    for r in 0..buf.nrows() {
        let matched = re.is_match(buf.cell(r, c));
        if matched ^ neg {
            for cc in 0..ncols {
                set.insert((r, cc));
            }
        }
    }
    Ok(set)
}

fn compile(body: &str, ci: bool) -> Result<Regex> {
    let pattern = if ci {
        format!("(?i){body}")
    } else {
        body.to_string()
    };
    Regex::new(&pattern).map_err(|e| parse(format!("bad regex /{body}/: {e}")))
}

fn resolve_regex(buf: &Buffer, body: &str, ci: bool) -> Result<CellSet> {
    let re = compile(body, ci)?;
    let ncols = buf.ncols();
    let mut set = CellSet::new();
    for r in 0..buf.nrows() {
        let hit = (0..ncols).any(|c| re.is_match(buf.cell(r, c)));
        if hit {
            for c in 0..ncols {
                set.insert((r, c));
            }
        }
    }
    Ok(set)
}

/// (row, col) extents a positional contributes, each 0-based; `None` on an axis it doesn't pin.
fn axes(buf: &Buffer, p: &Positional) -> Result<(Option<usize>, Option<usize>)> {
    Ok(match p {
        Positional::Cell { col, row } => (Some(row.saturating_sub(1)), Some(*col)),
        Positional::Column(c) => (None, Some(*c)),
        Positional::Row(n) => (Some(n.saturating_sub(1)), None),
        Positional::LastRow => (Some(buf.nrows().saturating_sub(1)), None),
        Positional::Name(name) => {
            let c = buf
                .name_to_col(name)
                .ok_or_else(|| XledError::Correction(format!("no column named [{name}]")))?;
            (None, Some(c))
        }
    })
}

fn resolve_range(buf: &Buffer, rr: &RangeRef) -> Result<CellSet> {
    let nrows = buf.nrows();
    let ncols = buf.ncols();

    if !rr.is_range {
        return resolve_single(buf, rr.start.as_ref().unwrap());
    }

    let (sr, sc) = match &rr.start {
        Some(p) => axes(buf, p)?,
        None => (None, None),
    };
    let (er, ec) = match &rr.end {
        Some(p) => axes(buf, p)?,
        None => (None, None),
    };

    let has_row = sr.is_some() || er.is_some();
    let has_col = sc.is_some() || ec.is_some();
    let mut set = CellSet::new();

    // Spans are clamped to the table extent (sed's `2,$` reading: addressing past the end
    // stops at the end, it does not invent phantom rows). A span wholly past the end is empty.
    if has_row && has_col {
        // rectangle (cell:cell)
        let (r1, r2) = ordered(sr.unwrap_or(0), er.unwrap_or(nrows.saturating_sub(1)));
        let (c1, c2) = ordered(sc.unwrap_or(0), ec.unwrap_or(ncols.saturating_sub(1)));
        if let (Some((r1, r2)), Some((c1, c2))) =
            (clamp_span(r1, r2, nrows), clamp_span(c1, c2, ncols))
        {
            for r in r1..=r2 {
                for c in c1..=c2 {
                    set.insert((r, c));
                }
            }
        }
    } else if has_row {
        // row span — all columns
        let (r1, r2) = ordered(sr.unwrap_or(0), er.unwrap_or(nrows.saturating_sub(1)));
        if let Some((r1, r2)) = clamp_span(r1, r2, nrows) {
            for r in r1..=r2 {
                for c in 0..ncols {
                    set.insert((r, c));
                }
            }
        }
    } else if has_col {
        // column span — all rows
        let (c1, c2) = ordered(sc.unwrap_or(0), ec.unwrap_or(ncols.saturating_sub(1)));
        if let Some((c1, c2)) = clamp_span(c1, c2, ncols) {
            for r in 0..nrows {
                for c in c1..=c2 {
                    set.insert((r, c));
                }
            }
        }
    } else {
        return Err(parse("empty range"));
    }
    Ok(set)
}

fn resolve_single(buf: &Buffer, p: &Positional) -> Result<CellSet> {
    let nrows = buf.nrows();
    let ncols = buf.ncols();
    let mut set = CellSet::new();
    // A coordinate past the end of its axis selects nothing — no phantom rows or columns.
    match axes(buf, p)? {
        (Some(r), Some(c)) => {
            if r < nrows && c < ncols {
                set.insert((r, c));
            }
        }
        (Some(r), None) => {
            if r < nrows {
                for c in 0..ncols {
                    set.insert((r, c));
                }
            }
        }
        (None, Some(c)) => {
            if c < ncols {
                for r in 0..nrows {
                    set.insert((r, c));
                }
            }
        }
        (None, None) => unreachable!("a positional always pins at least one axis"),
    }
    Ok(set)
}

fn ordered(a: usize, b: usize) -> (usize, usize) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Clamp an inclusive `[lo, hi]` span to an axis of length `len`; `None` if it starts past the end.
fn clamp_span(lo: usize, hi: usize, len: usize) -> Option<(usize, usize)> {
    if len == 0 || lo >= len {
        None
    } else {
        Some((lo, hi.min(len - 1)))
    }
}
