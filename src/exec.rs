//! Execute a parsed program against a buffer: resolve each statement's scope, run its
//! command. Slice 1 implements `show` (and the implicit show of a bare reference), which
//! renders the selected cells back to CSV/DSV text.

use crate::ast::{Command, DropAxis, Expr, Positional, RangeRef, Reference, Statement};
use crate::errors::{parse, Result, XledError};
use crate::expr::{self, EvalErr};
use crate::model::Buffer;
use crate::resolver::{self, CellSet};
use crate::subst::{self, Replacement};
use csv::WriterBuilder;
use regex::Regex;
use std::collections::BTreeSet;

/// What running a program produced. `output` is the rendered `show`/`describe` text — the
/// data channel, stdout in a pipe. `notices` are advisory remarks (the cast-skip tally and
/// the like): stderr in a one-shot so piped data stays clean, but shown inline in the REPL
/// and folded into the preview, where the user is watching. Keeping them apart is what lets
/// `xled '…' f.csv | …` stay uncorrupted while the REPL still surfaces the warning.
#[derive(Debug)]
pub struct Outcome {
    pub output: Vec<String>,
    pub notices: Vec<String>,
}

/// Run a program, collecting the text output of every `show` (mutating commands return none)
/// and any advisory notices raised along the way.
pub fn run(buf: &mut Buffer, program: &[Statement]) -> Result<Outcome> {
    let mut output = Vec::new();
    let mut notices = Vec::new();
    for st in program {
        if let Some(text) = run_statement(buf, st, &mut notices)? {
            output.push(text);
        }
    }
    Ok(Outcome { output, notices })
}

fn run_statement(
    buf: &mut Buffer,
    st: &Statement,
    notices: &mut Vec<String>,
) -> Result<Option<String>> {
    // Assignment resolves its target specially (it may create a new column), so it doesn't
    // go through the generic scope resolver.
    if let Some(Command::Assign(e)) = &st.command {
        apply_assign(buf, st.reference.as_ref(), e, notices)?;
        return Ok(None);
    }

    let scope = match &st.reference {
        Some(r) => resolver::resolve(buf, r)?,
        None => resolver::full_table(buf),
    };
    match &st.command {
        None | Some(Command::Show) => Ok(Some(render(buf, &scope))),
        Some(Command::Subst {
            re,
            rep,
            global,
            ci,
            nth,
        }) => {
            apply_subst(buf, &scope, re, rep, *global, *ci, *nth)?;
            Ok(None)
        }
        Some(Command::Del) => {
            do_del(buf, &scope)?;
            Ok(None)
        }
        Some(Command::Crop) => {
            do_crop(buf, &scope)?;
            Ok(None)
        }
        Some(Command::Header) => {
            do_header(buf, &scope)?;
            Ok(None)
        }
        Some(Command::Rename(name)) => {
            do_rename(buf, &scope, name)?;
            Ok(None)
        }
        Some(Command::Fill) => {
            do_fill(buf, &scope);
            Ok(None)
        }
        Some(Command::DropBlanks(axis)) => {
            do_drop_blanks(buf, *axis);
            Ok(None)
        }
        Some(Command::Describe) => Ok(Some(describe(buf))),
        Some(Command::Assign(_)) => unreachable!("handled above"),
    }
}

// --- scope shape (the scope contracts in ebnf.md) ------------------------

fn rows_of(scope: &CellSet) -> BTreeSet<usize> {
    scope.iter().map(|&(r, _)| r).collect()
}
fn cols_of(scope: &CellSet) -> BTreeSet<usize> {
    scope.iter().map(|&(_, c)| c).collect()
}

/// The rows, if the scope is exactly (some rows) × (all columns).
fn as_whole_rows(scope: &CellSet, ncols: usize) -> Option<Vec<usize>> {
    let rows = rows_of(scope);
    if !cols_of(scope).iter().copied().eq(0..ncols) {
        return None;
    }
    if scope.len() == rows.len() * ncols {
        Some(rows.into_iter().collect())
    } else {
        None
    }
}

/// The columns, if the scope is exactly (all rows) × (some columns).
fn as_whole_cols(scope: &CellSet, nrows: usize) -> Option<Vec<usize>> {
    let cols = cols_of(scope);
    if !rows_of(scope).iter().copied().eq(0..nrows) {
        return None;
    }
    if scope.len() == cols.len() * nrows {
        Some(cols.into_iter().collect())
    } else {
        None
    }
}

// --- structural commands -------------------------------------------------

fn do_del(buf: &mut Buffer, scope: &CellSet) -> Result<()> {
    if scope.is_empty() {
        return Ok(()); // an out-of-range address selects nothing; deleting nothing is a no-op
    }
    let ncols = buf.ncols();
    let nrows = buf.nrows();
    let whole_rows = as_whole_rows(scope, ncols);
    let whole_cols = as_whole_cols(scope, nrows);
    match (whole_rows, whole_cols) {
        // both = the full table; deleting rows clears the data
        (Some(rows), _) => {
            let drop: BTreeSet<usize> = rows.into_iter().collect();
            let mut i = 0;
            buf.rows.retain(|_| {
                let keep = !drop.contains(&i);
                i += 1;
                keep
            });
            Ok(())
        }
        (None, Some(cols)) => {
            let drop: BTreeSet<usize> = cols.into_iter().collect();
            for row in buf.rows.iter_mut() {
                *row = row
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !drop.contains(i))
                    .map(|(_, s)| s.clone())
                    .collect();
            }
            if let Some(h) = buf.header.as_mut() {
                *h = h
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !drop.contains(i))
                    .map(|(_, s)| s.clone())
                    .collect();
            }
            Ok(())
        }
        (None, None) => Err(XledError::Correction(
            "this address names a rectangle, and a rectangle has no row-or-column to drop. \
             To clear those cells, s/.*// over them; to delete, address whole rows (e.g. 2:4 del) \
             or whole columns (e.g. [status] del)."
                .into(),
        )),
    }
}

fn do_crop(buf: &mut Buffer, scope: &CellSet) -> Result<()> {
    if scope.is_empty() {
        return Err(XledError::Correction(
            "crop needs a region — give it a rectangle, e.g. A5:E8 crop".into(),
        ));
    }
    let min_r = scope.iter().map(|&(r, _)| r).min().unwrap();
    let max_r = scope.iter().map(|&(r, _)| r).max().unwrap();
    let min_c = scope.iter().map(|&(_, c)| c).min().unwrap();
    let max_c = scope.iter().map(|&(_, c)| c).max().unwrap();

    let new_rows: Vec<Vec<String>> = (min_r..=max_r)
        .map(|r| (min_c..=max_c).map(|c| buf.cell(r, c).to_string()).collect())
        .collect();
    buf.rows = new_rows;
    buf.header = None; // the cropped region is pure data; re-establish names with `header`
    Ok(())
}

fn do_header(buf: &mut Buffer, scope: &CellSet) -> Result<()> {
    let ncols = buf.ncols();
    let rows: Vec<usize> = rows_of(scope).into_iter().collect();
    if as_whole_rows(scope, ncols).is_none_or(|r| r.len() != 1) {
        return Err(XledError::Correction(
            "header promotes exactly one row — address a single row, e.g. 1 header".into(),
        ));
    }
    let r = rows[0];
    let names = buf.rows.remove(r);
    buf.header = Some(names);
    Ok(())
}

fn do_rename(buf: &mut Buffer, scope: &CellSet, name: &str) -> Result<()> {
    let nrows = buf.nrows();
    let cols: Vec<usize> = cols_of(scope).into_iter().collect();
    if as_whole_cols(scope, nrows).is_none_or(|c| c.len() != 1) {
        return Err(XledError::Correction(
            "rename takes exactly one column — address a single column, e.g. [old] rename new".into(),
        ));
    }
    let c = cols[0];
    let ncols = buf.ncols();
    let h = buf.header.get_or_insert_with(|| vec![String::new(); ncols]);
    if h.len() <= c {
        h.resize(c + 1, String::new());
    }
    h[c] = name.to_string();
    Ok(())
}

fn do_fill(buf: &mut Buffer, scope: &CellSet) {
    for &c in cols_of(scope).iter() {
        let mut rows: Vec<usize> = scope
            .iter()
            .filter(|&&(_, cc)| cc == c)
            .map(|&(r, _)| r)
            .collect();
        rows.sort_unstable();
        let mut last: Option<String> = None;
        for r in rows {
            let v = buf.cell(r, c).to_string();
            if v.is_empty() {
                if let Some(prev) = &last {
                    buf.set_cell(r, c, prev.clone());
                }
            } else {
                last = Some(v);
            }
        }
    }
}

fn do_drop_blanks(buf: &mut Buffer, axis: DropAxis) {
    if matches!(axis, DropAxis::Both | DropAxis::Rows) {
        let row_empty = |row: &Vec<String>| row.iter().all(|s| s.is_empty());
        while buf.rows.first().is_some_and(&row_empty) {
            buf.rows.remove(0);
        }
        while buf.rows.last().is_some_and(&row_empty) {
            buf.rows.pop();
        }
    }
    if matches!(axis, DropAxis::Both | DropAxis::Cols) {
        let ncols = buf.ncols();
        let nrows = buf.nrows();
        let col_empty = |c: usize| (0..nrows).all(|r| buf.cell(r, c).is_empty());
        let mut first = 0;
        while first < ncols && col_empty(first) {
            first += 1;
        }
        if first == ncols {
            return; // every column empty — nothing to keep
        }
        let mut last = ncols - 1;
        while last > first && col_empty(last) {
            last -= 1;
        }
        if first == 0 && last == ncols - 1 {
            return; // no edge columns to trim
        }
        for row in buf.rows.iter_mut() {
            *row = (first..=last)
                .map(|c| row.get(c).cloned().unwrap_or_default())
                .collect();
        }
        if let Some(h) = buf.header.as_mut() {
            *h = (first..=last).map(|c| h.get(c).cloned().unwrap_or_default()).collect();
        }
    }
}

/// Advisory region report — best guess, never acts (the human turns it into crop/header/del).
fn describe(buf: &Buffer) -> String {
    let nrows = buf.nrows();
    let ncols = buf.ncols();
    let row_empty = |r: usize| (0..ncols).all(|c| buf.cell(r, c).is_empty());

    let leading = (0..nrows).take_while(|&r| row_empty(r)).count();
    let trailing = (0..nrows).rev().take_while(|&r| row_empty(r)).count();
    let total_re = regex::Regex::new(r"(?i)^(sub)?total").unwrap();
    let totals: Vec<usize> = (0..nrows)
        .filter(|&r| (0..ncols).any(|c| total_re.is_match(buf.cell(r, c))))
        .map(|r| r + 1)
        .collect();

    // Suspected buried header: a full-width row sitting under a *narrower* non-blank
    // preamble (a title block like "RISK LOG" / "PM:"). The body's modal fill-count is the
    // table width; a header is the first row to reach it. `leading blank rows` can't catch
    // this because the preamble rows aren't blank — they're just narrow.
    let width = |r: usize| (0..ncols).filter(|&c| !buf.cell(r, c).is_empty()).count();
    let mut freq: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
    for r in 0..nrows {
        let w = width(r);
        if w > 0 {
            *freq.entry(w).or_default() += 1;
        }
    }
    let body_width = freq.iter().max_by_key(|(_, &n)| n).map(|(&w, _)| w).unwrap_or(0);
    let header_guess = (body_width >= 2)
        .then(|| (0..nrows).find(|&r| width(r) >= body_width))
        .flatten()
        .filter(|&h| h > leading); // a narrow non-blank row precedes the first full-width row

    let mut out = format!("{nrows} rows × {ncols} cols.");
    out.push_str(&format!(" leading blank rows: {leading}."));
    out.push_str(&format!(" trailing blank rows: {trailing}."));
    if let Some(h) = header_guess {
        out.push_str(&format!(
            " suspected header row: {} (narrower preamble above — try `{} header`).",
            h + 1,
            h + 1
        ));
    }
    if !totals.is_empty() {
        out.push_str(&format!(
            " suspected total/section rows: {}.",
            join_rows(&totals)
        ));
    }
    out.push_str(" (advisory — turn this into crop/header/del yourself.)");
    out
}

/// Assign `= expr` into exactly one column over a row scope, creating the column if new.
fn apply_assign(
    buf: &mut Buffer,
    reference: Option<&Reference>,
    e: &Expr,
    notices: &mut Vec<String>,
) -> Result<()> {
    let reference = reference.ok_or_else(|| {
        XledError::Correction("assignment needs a target column, e.g. `[total] = …`".into())
    })?;
    let (col, rows, create) = assign_target(buf, reference)?;

    // Materialize a new column: extend the header overlay (if any) with its name.
    if let Some(name) = create {
        if let Some(h) = buf.header.as_mut() {
            if h.len() <= col {
                h.resize(col + 1, String::new());
            }
            h[col] = name;
        }
    }

    let mut skipped: Vec<usize> = Vec::new();
    for r in rows {
        match expr::eval(buf, r, e) {
            Ok(v) => buf.set_cell(r, col, v.into_string()),
            Err(EvalErr::Cast) => skipped.push(r + 1), // 1-based for the message
            Err(EvalErr::Hard(err)) => return Err(err),
        }
    }
    if !skipped.is_empty() {
        // Lenient cast-failure tally (errors.md correction voice), raised as an advisory
        // notice so it rides the result channel — folded into the preview, inline in the
        // REPL, on stderr in a one-shot — instead of bypassing it via stderr unconditionally.
        notices.push(format!(
            "{} cell(s) skipped (not computable): rows {} — left unchanged.",
            skipped.len(),
            join_rows(&skipped)
        ));
    }
    Ok(())
}

/// Determine the single target column, the rows to write, and a name if the column is new.
fn assign_target(buf: &Buffer, reference: &Reference) -> Result<(usize, Vec<usize>, Option<String>)> {
    let all_rows = || (0..buf.nrows()).collect::<Vec<_>>();
    match reference {
        // bare [name]: existing column, or create a new one
        Reference::Range(RangeRef {
            start: Some(Positional::Name(name)),
            end: None,
            is_range: false,
        }) => match buf.name_to_col(name) {
            Some(c) => Ok((c, all_rows(), None)),
            // A new column can only carry a name if there's a header row to hold it.
            // Without one the name has nowhere to live: it would be silently dropped and
            // each re-run would append another unnamed column. Refuse instead.
            None if buf.header.is_none() => Err(XledError::Correction(format!(
                "no header to name column [{name}] — promote one first (`1 header`) or assign by letter (`{} = …`).",
                crate::model::col_to_letter(buf.ncols())
            ))),
            None => Ok((buf.ncols(), all_rows(), Some(name.clone()))),
        },
        // bare column letter: existing or new-by-letter (no name)
        Reference::Range(RangeRef {
            start: Some(Positional::Column(c)),
            end: None,
            is_range: false,
        }) => Ok((*c, all_rows(), None)),
        // general: resolve, require exactly one column
        other => {
            let set = resolver::resolve(buf, other)?;
            let cols: BTreeSet<usize> = set.iter().map(|&(_, c)| c).collect();
            if cols.len() != 1 {
                return Err(XledError::Correction(format!(
                    "this address spans {} columns; assignment writes exactly one — assign to one column, or run two commands.",
                    cols.len()
                )));
            }
            let col = *cols.iter().next().unwrap();
            let rows: Vec<usize> = set
                .iter()
                .map(|&(r, _)| r)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            Ok((col, rows, None))
        }
    }
}

/// Format a row-number list for the cast-failure tally, abbreviating long runs.
fn join_rows(rows: &[usize]) -> String {
    const MAX: usize = 8;
    if rows.len() <= MAX {
        rows.iter().map(|r| r.to_string()).collect::<Vec<_>>().join(", ")
    } else {
        let head: Vec<String> = rows[..MAX].iter().map(|r| r.to_string()).collect();
        format!("{}, … (+{} more)", head.join(", "), rows.len() - MAX)
    }
}

/// Apply `s///` to every cell in scope, writing back only the cells it changes.
fn apply_subst(
    buf: &mut Buffer,
    scope: &CellSet,
    re: &str,
    rep: &str,
    global: bool,
    ci: bool,
    nth: Option<usize>,
) -> Result<()> {
    let pattern = if ci {
        format!("(?i){re}")
    } else {
        re.to_string()
    };
    let regex = Regex::new(&pattern).map_err(|e| parse(format!("bad regex /{re}/: {e}")))?;
    let replacement = Replacement::parse(rep);

    for &(r, c) in scope {
        let old = buf.cell(r, c).to_string();
        let new = subst::substitute(&regex, &replacement, &old, global, nth);
        if new != old {
            buf.set_cell(r, c, new);
        }
    }
    Ok(())
}

/// Render a cell set as CSV/DSV: the present columns (with header, if any), and for each
/// present row the selected cells (cells outside the set render empty). Exact for rectangles
/// and column/row selections; a reasonable projection for arbitrary unions.
pub fn render(buf: &Buffer, scope: &CellSet) -> String {
    let cols: Vec<usize> = scope
        .iter()
        .map(|&(_, c)| c)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let rows: Vec<usize> = scope
        .iter()
        .map(|&(r, _)| r)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let mut wtr = WriterBuilder::new()
        .delimiter(buf.delim)
        .flexible(true)
        .from_writer(Vec::new());

    if buf.header.is_some() {
        let rec: Vec<&str> = cols.iter().map(|&c| buf.col_name(c).unwrap_or("")).collect();
        wtr.write_record(&rec).unwrap();
    }
    for &r in &rows {
        let rec: Vec<&str> = cols
            .iter()
            .map(|&c| {
                if scope.contains(&(r, c)) {
                    buf.cell(r, c)
                } else {
                    ""
                }
            })
            .collect();
        wtr.write_record(&rec).unwrap();
    }

    let bytes = wtr.into_inner().unwrap();
    let text = String::from_utf8(bytes).unwrap();
    text.trim_end_matches('\n').to_string()
}
