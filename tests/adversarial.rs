//! Adversarial battery (proving-ground Part A) — the seams the happy-path slice tests
//! don't drive at: composition (A1), header addressing (A4), bracket/case rules (A7),
//! ragged rows (A8), negation (A9), coordinate-named columns (A12), default scope (A13).
//! Each case is `fixture + command → expected output (or value)`.

use xled::{exec, io, parser, Buffer};

const PRODUCTS: &str = include_str!("../fixtures/products.csv");
const APP: &str = include_str!("../fixtures/app-portfolio.csv");
const RAGGED: &str = include_str!("../fixtures/ragged.csv");
const TRICKY: &str = include_str!("../fixtures/tricky-headers.csv");
const BRACKET: &str = include_str!("../fixtures/bracket-headers.csv");
const QUOTED: &str = include_str!("../fixtures/quoted-hell.csv");

fn show(csv: &str, cmd: &str) -> String {
    let mut buf = io::read_str(csv, b',', true).unwrap();
    let p = parser::parse_program(cmd).unwrap();
    exec::run(&mut buf, &p).unwrap().output.join("\n")
}

fn run(csv: &str, cmd: &str) -> Buffer {
    let mut buf = io::read_str(csv, b',', true).unwrap();
    let p = parser::parse_program(cmd).unwrap();
    exec::run(&mut buf, &p).unwrap();
    buf
}

// --- A1: named column intersected with a row range -----------------------

#[test]
fn name_and_letter_rows_express_the_same_intersection() {
    // [status] is column D; all three forms must select the same cells.
    let by_name = show(APP, "[status] 2:4");
    let by_letter_split = show(APP, "D 2:4");
    let by_letter_a1 = show(APP, "D2:D4");
    assert_eq!(by_name, "status\nactive\nIN USE\nActive "); // trailing space preserved
    assert_eq!(by_name, by_letter_split);
    assert_eq!(by_name, by_letter_a1);
}

// --- A4: the header is addressable, not just protected -------------------

#[test]
fn header_is_renamable() {
    let b = run(TRICKY, "[notes.txt] rename notes");
    assert_eq!(b.col_name(4), Some("notes"));
    // the data cells under it are untouched
    assert_eq!(b.cell(0, 4), "readme");
}

#[test]
fn ordinary_column_op_leaves_the_header_untouched() {
    let b = run(APP, r"[status] s/.*/X/");
    assert_eq!(b.col_name(3), Some("status")); // header not edited
    assert_eq!(b.cell(0, 3), "X"); // data is
}

// --- A7: bracket edges and the three case rules --------------------------

#[test]
fn header_containing_a_bracket_is_addressable_via_double_close() {
    // header is literally `notes [draft]`; the inner `]` is escaped as `]]`.
    let out = show(BRACKET, "[notes [draft]]]");
    assert_eq!(out, "notes [draft]\nfirst pass\nneeds review\nsigned off");
}

#[test]
fn column_letters_are_case_insensitive() {
    assert_eq!(show(PRODUCTS, "c"), show(PRODUCTS, "C"));
}

#[test]
fn column_names_are_case_sensitive() {
    // header is `userid`; `[userId]` must not resolve to it.
    let mut buf = io::read_str(BRACKET, b',', true).unwrap();
    let p = parser::parse_program("[userId]").unwrap();
    assert!(exec::run(&mut buf, &p).is_err());
}

#[test]
fn regex_case_is_governed_by_the_flag() {
    // /RETIRED/i matches the "Retired" row.
    let out = show(APP, "/RETIRED/i");
    assert!(out.contains("DocVault"));
    assert!(!out.contains("SAP ERP"));
}

// --- A8: ragged rows — column address on a short row ----------------------

#[test]
fn missing_cell_reads_as_empty_string() {
    let buf = io::read_str(RAGGED, b',', true).unwrap();
    assert_eq!(buf.cell(1, 2), ""); // Bob has 2 fields; [dept] (col 2) is missing
    assert_eq!(buf.cell(3, 2), ""); // Dave has 1 field
    assert_eq!(buf.cell(2, 3), "Extra"); // Carol has 5 fields; D (col 3) is present
    assert_eq!(buf.cell(3, 3), ""); // Dave has no col 3 either
}

#[test]
fn assignment_pads_a_short_row() {
    let b = run(RAGGED, r#"[dept] = "Unknown""#);
    assert_eq!(b.cell(1, 2), "Unknown"); // Bob's row padded out to 3 cols
    assert_eq!(b.cell(3, 2), "Unknown"); // Dave's row padded out from 1 col
    assert_eq!(b.cell(2, 3), "Extra"); // Carol's trailing fields survive the write
}

// --- A9: address negation reads as scope ---------------------------------

#[test]
fn negated_regex_selects_the_complement() {
    let out = show(APP, "!/active/i");
    assert!(out.contains("DocVault")); // Retired → kept
    assert!(out.contains("IN USE")); // not literally "active" → kept
    assert!(!out.contains("SAP ERP")); // Active → excluded
    assert!(!out.contains("QA Sandbox")); // active → excluded
}

// --- A12: a column named like a coordinate -------------------------------

#[test]
fn bracketed_coordinate_name_is_the_column_not_the_address() {
    // tricky-headers has a column literally named "B" (col index 2) and one named "2024".
    // [B] is that named column; bare B is the letter (col index 1, the "2024" header).
    assert_eq!(show(TRICKY, "[B]"), "B\nbeta\nbravo\nboron");
    assert_eq!(show(TRICKY, "[2024]"), "2024\n1200\n1450\n980");
    assert_ne!(show(TRICKY, "[B]"), show(TRICKY, "B"));
    assert_eq!(show(TRICKY, "B"), show(TRICKY, "[2024]")); // letter B == column index 1
}

// --- A13: default (omitted) scope ----------------------------------------

#[test]
fn omitted_column_scope_is_every_cell() {
    let b = run(PRODUCTS, "s/o/0/g");
    assert_eq!(b.cell(0, 0), "Widget Pr0"); // col 0 touched
    assert_eq!(b.cell(0, 1), "t00ls"); // col 1 touched
}

// --- regression: a range bound past the end clamps, it doesn't invent rows ---
// `2:12` on a 9-row file once rendered phantom empty rows, and `2:2000000` allocated
// two million of them. Spans now clamp to the table extent (sed's `2,$` reading).

#[test]
fn range_bound_past_the_end_clamps_to_the_extent() {
    // products.csv has 9 data rows; 2: and 2:9999 must select the same cells.
    assert_eq!(show(PRODUCTS, "2:9999"), show(PRODUCTS, "2:"));
    // no trailing empty rows leaked in
    let out = show(PRODUCTS, "2:9999");
    assert!(out.ends_with("Safety Goggles,safety,7.99,SF-0003"));
    assert!(!out.contains("\n,,,"));
}

#[test]
fn single_address_past_the_end_selects_nothing() {
    // row 9999 on a 9-row file → empty; show prints just the header, del is a no-op.
    assert!(!show(PRODUCTS, "9999").contains("Widget"));
    let mut buf = io::read_str(PRODUCTS, b',', true).unwrap();
    let before = buf.nrows();
    let p = parser::parse_program("9999 del").unwrap();
    exec::run(&mut buf, &p).unwrap(); // no error, no change
    assert_eq!(buf.nrows(), before);
}

// --- regression: an s/// whose pattern or replacement holds `<`/`>` ------
// `has_leading_comparison` once mis-scanned past the substitute delimiters and read the
// `>` in the replacement as a top-level comparison, silently dropping the whole edit.

#[test]
fn substitution_with_angle_brackets_is_not_read_as_a_comparison() {
    let prepend = run(QUOTED, r"[note] s/^/>> /");
    assert_eq!(b_note(&prepend, 0), ">> Called re: order. Said \"too slow\" — follow up.");
    // exactly one prefix on the multi-line cell (cell-bounded ^)
    assert!(b_note(&prepend, 1).starts_with(">> Multi-line"));
    assert_eq!(b_note(&prepend, 1).matches(">> ").count(), 1);

    let in_pattern = run(PRODUCTS, "[category] s/o/<>/g");
    assert_eq!(in_pattern.cell(0, 1), "t<><>ls");
}

fn b_note(b: &Buffer, row: usize) -> &str {
    b.cell(row, 3)
}

// --- regression: assigning to a new named column needs a header to hold the name ---
// On a headerless buffer `[tag] = "Z"` once created the column by position but dropped the
// name, and re-running appended *another* unnamed column (non-idempotent). It now refuses.

#[test]
fn naming_a_new_column_without_a_header_is_refused() {
    let mut buf = io::read_str(PRODUCTS, b',', false).unwrap(); // no header overlay
    let p = parser::parse_program(r#"[tag] = "Z""#).unwrap();
    let err = exec::run(&mut buf, &p).unwrap_err();
    assert!(err.to_string().contains("no header to name column [tag]"));
    // the table was not grown by the rejected write
    let cols_before = io::read_str(PRODUCTS, b',', false).unwrap().ncols();
    assert_eq!(buf.ncols(), cols_before);
}

#[test]
fn naming_a_new_column_with_a_header_is_idempotent() {
    // With a header to hold the name, create-by-assign works and re-running is a no-op
    // (the second run finds the existing column instead of appending a new one).
    let once = run(PRODUCTS, r#"[tag] = "Z""#);
    let twice = run(PRODUCTS, "[tag] = \"Z\"\n[tag] = \"Z\"");
    assert_eq!(once.ncols(), twice.ncols());
    assert_eq!(twice.col_name(once.ncols() - 1), Some("tag"));
}
