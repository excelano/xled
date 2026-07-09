//! Compute-library completeness (post-0.3.0 Tier 1): the case, trim, pad, and numeric
//! functions added to `= expr`. Library-level like `slice3_compute.rs` — read an inline CSV,
//! run the program, read the computed column back. New columns append at the original width,
//! so a single `[out] = …` assign lands at column index 2 in these two-column fixtures.

use xled::{exec, io, parser, Buffer};

// A quoted field pins the leading/trailing/interior spaces regardless of CSV trim settings.
const NAMES: &str = "id,name\n1,\"  john  MCDONALD \"\n2,café\n3,o'brien\n";
const ZIPS: &str = "id,zip\n1,501\n2,12345\n3,123456\n";

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
    assert!(e.contains("2 or 3 arguments"), "arity error names the shape: {e}");
}
