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
        Reference::ColRegexSel { col, neg, body, ci } => {
            resolve_col_regex(buf, col, *neg, body, *ci)
        }
        Reference::Comparison(e) => resolve_comparison(buf, e),
        // Bounds::Clamp is sed's reading of `2,$`: addressing past the end stops at the end
        // and invents nothing. xshape chooses Strict for the same grammar, because a reshape
        // that quietly does less than asked is worse than one that stops.
        //
        // Rows clamp; a lone column past the width does not. A file with fewer rows than the
        // address asked for is a short file, but a table's column count is its schema, so
        // `ZZ` on a three-column table is a typo, not an empty selection. Checked here
        // rather than in xaddr because xshape wants the uniform rule.
        Reference::Range(spec) => {
            check_columns_in_range(buf, spec)?;
            spec.cells(buf, xaddr::Bounds::Clamp)
                .map_err(|e| XledError::Correction(e.message))
        }
    }
}

/// Refuse a lone column address that lies past the table's width.
///
/// Only `Item::Single` is checked, and only its column half: the end of a span still clamps
/// (`A:ZZ` means "to the last column"), and a row past the end still selects nothing, which is
/// sed's reading and the right one for data. Named columns are left alone — `[new]` on the
/// left of an assignment creates a column, which is documented.
///
/// This is what keeps a mistyped command from passing for an address. Every lowercase word is
/// legal column letters, so without this `xled 'sort'` reads column SORT, finds nothing, and
/// prints an empty result with a success code.
fn check_columns_in_range(buf: &Buffer, spec: &xaddr::Spec) -> Result<()> {
    let ncols = buf.ncols();
    for item in spec.items() {
        let xaddr::Item::Single(pos) = item else {
            continue;
        };
        let idx = match pos {
            xaddr::Pos::Column(xaddr::ColRef::Letters(i)) => *i,
            xaddr::Pos::Cell {
                col: xaddr::ColRef::Letters(i),
                ..
            } => *i,
            _ => continue,
        };
        if idx >= ncols {
            let wrote = xaddr::col_to_letter(idx);
            let widest = xaddr::col_to_letter(ncols.saturating_sub(1));
            return Err(XledError::Correction(format!(
                "column {wrote} is past this table's {ncols} columns (A-{widest}). \
                 A bare word is read as column letters, so if {wrote} was meant as a command \
                 there is no such command — see --help; if it is a column name, write [{wrote}]."
            )));
        }
    }
    Ok(())
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
