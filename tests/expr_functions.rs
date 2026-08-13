//! Compute-library completeness (post-0.3.0 Tier 1): the case, trim, pad, and numeric
//! functions added to `= expr`. Library-level like `slice3_compute.rs` — read an inline CSV,
//! run the program, read the computed column back. New columns append at the original width,
//! so a single `[out] = …` assign lands at column index 2 in these two-column fixtures.

use xled::{exec, io, parser, Buffer};

// A quoted field pins the leading/trailing/interior spaces regardless of CSV trim settings.
const NAMES: &str = "id,name\n1,\"  john  MCDONALD \"\n2,café\n3,o'brien\n";
const ZIPS: &str = "id,zip\n1,501\n2,12345\n3,123456\n";
// row 0: a=-3 b=5 · row 1: a=7 b=0 (the mod-by-zero row) · row 2: a=2.5 b=2
const NUMS: &str = "a,b\n-3,5\n7,0\n2.5,2\n";

fn run(csv: &str, prog: &str) -> Buffer {
    let mut buf = io::read_str(csv, b',', true).unwrap();
    let p = parser::parse_program(prog).unwrap();
    exec::run(&mut buf, &p).unwrap();
    buf
}

fn err(csv: &str, prog: &str) -> String {
    let mut buf = io::read_str(csv, b',', true).unwrap();
    let p = match parser::parse_program(prog) {
        Ok(p) => p,
        Err(e) => return e.to_string(),
    };
    exec::run(&mut buf, &p).unwrap_err().to_string()
}

fn notices(csv: &str, prog: &str) -> Vec<String> {
    let mut buf = io::read_str(csv, b',', true).unwrap();
    let p = parser::parse_program(prog).unwrap();
    exec::run(&mut buf, &p).unwrap().notices
}

// --- case folding --------------------------------------------------------

#[test]
fn upper_and_lower_are_unicode() {
    let b = run(NAMES, r"[out] = upper([name])");
    assert_eq!(b.cell(1, 2), "CAFÉ"); // the É proves it is not ASCII-only
    let b = run(NAMES, r"[out] = lower([name])");
    assert_eq!(b.cell(0, 2), "  john  mcdonald "); // spaces preserved, all lowered
}

#[test]
fn upper_matches_the_s_substitution_case_fold() {
    // The consistency guarantee behind adding these: upper() and `\U&` fold identically,
    // non-ASCII included, so a script can reach for either and never see them disagree.
    let via_fn = run(NAMES, r"[out] = upper([name])");
    let via_sub = run(NAMES, r"[name] s/.*/\U&/");
    assert_eq!(via_fn.cell(1, 2), "CAFÉ");
    assert_eq!(via_fn.cell(1, 2), via_sub.cell(1, 1));
}

#[test]
fn proper_title_cases_and_keeps_the_excel_quirk() {
    let b = run(NAMES, r"[out] = proper([name])");
    // McDonald -> Mcdonald (a non-letter resets the run; interior double space kept)
    assert_eq!(b.cell(0, 2), "  John  Mcdonald ");
    assert_eq!(b.cell(2, 2), "O'Brien"); // the apostrophe resets the run, as in Excel
}

// --- whitespace stripping ------------------------------------------------

#[test]
fn trim_strips_edges_but_not_interior() {
    let b = run(NAMES, r"[out] = trim([name])");
    assert_eq!(b.cell(0, 2), "john  MCDONALD"); // the inner double space survives
}

#[test]
fn ltrim_and_rtrim_are_one_sided() {
    let b = run(NAMES, r"[out] = ltrim([name])");
    assert_eq!(b.cell(0, 2), "john  MCDONALD "); // trailing space kept
    let b = run(NAMES, r"[out] = rtrim([name])");
    assert_eq!(b.cell(0, 2), "  john  MCDONALD"); // leading space kept
}

// --- fixed-width padding -------------------------------------------------

#[test]
fn lpad_restores_leading_zeros_and_never_truncates() {
    let b = run(ZIPS, r#"[out] = lpad([zip], 5, "0")"#);
    assert_eq!(b.cell(0, 2), "00501"); // padded up to width
    assert_eq!(b.cell(1, 2), "12345"); // already exact — unchanged
    assert_eq!(b.cell(2, 2), "123456"); // already wider — NOT truncated
}

#[test]
fn rpad_left_aligns_and_default_fill_is_space() {
    let b = run(ZIPS, r"[out] = rpad([zip], 5)");
    assert_eq!(b.cell(0, 2), "501  "); // space-padded on the right
}

#[test]
fn pad_fill_repeats_and_empty_fill_passes_through() {
    let b = run(ZIPS, r#"[out] = lpad([zip], 6, "ab")"#);
    assert_eq!(b.cell(0, 2), "aba501"); // fill repeats, cut to the 3-char deficit
    let b = run(ZIPS, r#"[out] = lpad([zip], 6, "")"#);
    assert_eq!(b.cell(0, 2), "501"); // empty fill can't pad — value unchanged
}

#[test]
fn pad_rejects_wrong_arity() {
    let e = err(ZIPS, r"[out] = lpad([zip])");
    assert!(
        e.contains("2 or 3 arguments"),
        "arity error names the shape: {e}"
    );
}

// --- numeric helpers -----------------------------------------------------

#[test]
fn abs_floor_ceil_operate_on_num_cast_values() {
    assert_eq!(run(NUMS, r"[out] = abs([a])").cell(0, 2), "3"); // |-3|
    assert_eq!(run(NUMS, r"[out] = floor([a])").cell(2, 2), "2"); // floor 2.5
    assert_eq!(run(NUMS, r"[out] = ceil([a])").cell(2, 2), "3"); // ceil 2.5
}

#[test]
fn mod_takes_the_dividend_sign_and_skips_divide_by_zero() {
    let b = run(NUMS, r"[out] = mod([a], [b])");
    assert_eq!(b.cell(0, 2), "-3"); // mod(-3, 5) follows the dividend, not Excel's +2
    assert_eq!(b.cell(2, 2), "0.5"); // mod(2.5, 2)
    assert_eq!(b.cell(1, 2), ""); // mod(7, 0): the row is skipped, the new cell stays empty
    let n = notices(NUMS, r"[out] = mod([a], [b])");
    assert_eq!(n.len(), 1);
    assert!(
        n[0].contains("skipped"),
        "the divide-by-zero row is tallied: {n:?}"
    );
}

#[test]
fn min_and_max_are_numeric_and_variadic() {
    assert_eq!(run(NUMS, r"[out] = min([a], [b])").cell(0, 2), "-3"); // min(-3, 5)
    assert_eq!(run(NUMS, r"[out] = max([a], [b])").cell(0, 2), "5"); // max(-3, 5)
                                                                     // A third (literal) arg proves variadic: max(-3, 5, 100) = 100
    assert_eq!(run(NUMS, r"[out] = max([a], [b], 100)").cell(0, 2), "100");
}

#[test]
fn min_rejects_zero_args() {
    let e = err(NUMS, r"[out] = min()");
    assert!(
        e.contains("at least one argument"),
        "arity error is explicit: {e}"
    );
}

// --- regex as a value ----------------------------------------------------
//
// `s///` can only write back into the column it reads, so a pattern that derives one column
// from another had no expression to say it in. These two close that (#18).

// A stacked org prefix, a single one, and a row with none — the shape that motivated this.
const REQ: &str =
    "src,out\nT&D/CE/SO&AT - DeAnna Ervin/Chris Bell,\nAPP - Jane Doe,\nNoPrefix Person,\n";

#[test]
fn regexreplace_writes_into_a_different_column() {
    let b = run(
        REQ,
        r#"[out] = regexreplace([src], "^(?:(?:APP|CE|SO&AT|T&D)\s*[-/\\ ]\s*)+", "")"#,
    );
    // A `+` on the alternation collapses the stacked prefixes in one pass — no loop needed.
    assert_eq!(b.cell(0, 1), "DeAnna Ervin/Chris Bell");
    assert_eq!(b.cell(1, 1), "Jane Doe");
    // No match leaves the value whole rather than emptying it.
    assert_eq!(b.cell(2, 1), "NoPrefix Person");
    // The source column is untouched — this reads, it does not rewrite.
    assert_eq!(b.cell(0, 0), "T&D/CE/SO&AT - DeAnna Ervin/Chris Bell");
}

/// The replacement is xled's own sed dialect, not the regex crate's `$1`, because it is the
/// same `Replacement` parser `s///` uses. One dialect, so the two cannot drift.
#[test]
fn regexreplace_uses_the_sed_replacement_dialect() {
    let csv = "a,b\n\"Doe, Jane\",\n";
    let b = run(
        csv,
        r#"[b] = regexreplace([a], "^(\w+), (\w+)$", "\U\1\E, \u\2")"#,
    );
    assert_eq!(b.cell(0, 1), "DOE, Jane");
    let b = run(csv, r#"[b] = regexreplace([a], "\w+", "<&>")"#);
    assert_eq!(b.cell(0, 1), "<Doe>, <Jane>");
}

/// Every match, unlike `s///` without `g`. This is the spreadsheet family's REGEXREPLACE and
/// keeps their contract, where `s///` keeps sed's.
#[test]
fn regexreplace_replaces_every_match() {
    let b = run("a,b\na-b-c,\n", r#"[b] = regexreplace([a], "-", "+")"#);
    assert_eq!(b.cell(0, 1), "a+b+c");
}

#[test]
fn regexmatch_branches_inside_if() {
    let b = run(
        REQ,
        r#"[out] = if(regexmatch([src], "/"), "multi", "single")"#,
    );
    assert_eq!(b.cell(0, 1), "multi");
    assert_eq!(b.cell(1, 1), "single");
    assert_eq!(b.cell(2, 1), "single");
}

/// The pattern is an ordinary argument, so it may be a column and vary row to row. The
/// compile cache is keyed on the pattern text for exactly this reason.
#[test]
fn the_pattern_may_come_from_a_column() {
    let b = run(
        "txt,pat,out\nfoobar,o+,\nbazzz,z+,\n",
        r#"[out] = regexreplace([txt], [pat], "-")"#,
    );
    assert_eq!(b.cell(0, 2), "f-bar");
    assert_eq!(b.cell(1, 2), "ba-");
}

/// A bad pattern is wrong on every row, so it halts rather than tallying one skip per row —
/// the same line rule 6 draws for a cast failure, on the other side of it.
#[test]
fn a_bad_pattern_halts_and_shows_the_engine_message() {
    let e = err("a,b\nx,\n", r#"[b] = regexreplace([a], "(", "y")"#);
    assert!(e.contains("bad regex"), "got: {e}");
    assert!(e.contains("regex parse error"), "got: {e}");
}

#[test]
fn regex_functions_state_their_arity() {
    let e = err("a,b\nx,\n", r#"[b] = regexreplace([a], "x")"#);
    assert!(e.contains("takes 3 argument(s), got 2"), "got: {e}");
    let e = err("a,b\nx,\n", r#"[b] = regexmatch([a])"#);
    assert!(e.contains("takes 2 argument(s), got 1"), "got: {e}");
}
