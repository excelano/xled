//! Slice 4 conformance: Part B11 (structural edits) + Part C (structural intake).
//! Intake fixtures are read header-less — the real header isn't row 1.

use xled::{exec, io, parser, Buffer};

const PRODUCTS: &str = include_str!("../fixtures/products.csv");
const PREAMBLE: &str = include_str!("../fixtures/messy/preamble.csv");
const STACKED: &str = include_str!("../fixtures/messy/stacked.csv");
const FILL_DOWN: &str = include_str!("../fixtures/messy/fill-down.csv");
const SPACER: &str = include_str!("../fixtures/messy/spacer-column.csv");
const SIDE: &str = include_str!("../fixtures/messy/side-by-side.csv");

fn run(csv: &str, delim: u8, has_header: bool, prog: &str) -> Buffer {
    let mut buf = io::read_str(csv, delim, has_header).unwrap();
    let p = parser::parse_program(prog).unwrap();
    exec::run(&mut buf, &p).unwrap();
    buf
}

fn run_out(csv: &str, delim: u8, has_header: bool, prog: &str) -> String {
    let mut buf = io::read_str(csv, delim, has_header).unwrap();
    let p = parser::parse_program(prog).unwrap();
    exec::run(&mut buf, &p).unwrap().output.join("\n")
}

fn err(csv: &str, delim: u8, prog: &str) -> String {
    let mut buf = io::read_str(csv, delim, true).unwrap();
    let p = parser::parse_program(prog).unwrap();
    exec::run(&mut buf, &p).unwrap_err().to_string()
}

// --- B11 structural edits ------------------------------------------------

#[test]
fn delete_rows_and_columns() {
    let by_num = run(PRODUCTS, b',', true, "3 del");
    assert_eq!(by_num.nrows(), 7); // 8 → 7
    assert_eq!(by_num.cell(2, 0), "Micro Sensor"); // row 3 (Pro Hammer) gone

    let by_match = run(PRODUCTS, b',', true, "/gizmos/ del");
    assert_eq!(by_match.nrows(), 6); // two gizmo rows removed

    let drop_col = run(PRODUCTS, b',', true, "[sku] del");
    assert_eq!(drop_col.ncols(), 3);
    assert_eq!(drop_col.col_name(2), Some("price"));
}

#[test]
fn partial_rectangle_delete_is_refused() {
    let msg = err(PRODUCTS, b',', "B2:C3 del");
    assert!(msg.contains("rectangle has no row-or-column"), "got: {msg}");
}

#[test]
fn rename_header() {
    let b = run(PRODUCTS, b',', true, "[sku] rename product_code");
    assert_eq!(b.col_name(3), Some("product_code"));
}

#[test]
fn duplicate_column_via_assign() {
    let b = run(PRODUCTS, b',', true, "[sku_copy] = [sku]");
    assert_eq!(b.cell(0, 4), "TL-0042");
}

// --- Part C structural intake --------------------------------------------

#[test]
fn crop_then_header() {
    // C1: carve the real table out of preamble, then promote its header row
    let b = run(PREAMBLE, b',', false, "A5:E8 crop\n1 header");
    assert_eq!(b.col_name(0), Some("ID"));
    assert_eq!(b.col_name(1), Some("Application"));
    assert_eq!(b.nrows(), 3); // R1, R2, R3
    assert_eq!(b.cell(0, 0), "R1");
}

#[test]
fn drop_trailing_blank_rows() {
    // C3: two trailing blank rows go; the interior blank (row 4) stays
    let b = run(PREAMBLE, b',', false, "drop blanks rows");
    assert_eq!(b.nrows(), 8); // 10 → 8
    assert!(b.cell(7, 0).starts_with("R3"));
}

#[test]
fn delete_totals_row_by_match() {
    // C6: drop the interleaved "Total" row
    let b = run(STACKED, b',', false, "/^Total/i del");
    let has_total = (0..b.nrows()).any(|r| b.cell(r, 0) == "Total");
    assert!(!has_total);
}

#[test]
fn fill_down_grouping_column() {
    // C5: forward-fill the merged grouping column
    let b = run(FILL_DOWN, b',', true, "[Vendor] fill");
    assert_eq!(b.cell(1, 0), "Acme");
    assert_eq!(b.cell(2, 0), "Acme");
    assert_eq!(b.cell(4, 0), "Globex");
}

#[test]
fn drop_blank_cols_keeps_index_column() {
    // C4: trailing empty cols go; blank-header col A (holds the index) stays
    let b = run(SPACER, b',', false, "drop blanks cols");
    assert_eq!(b.ncols(), 3); // 6 → 3 (D,E,F dropped)
    assert_eq!(b.cell(1, 0), "1"); // index column kept, reached by letter
}

#[test]
fn crop_before_drop_keeps_side_by_side_separate() {
    // C8: crop the left table first so the spacer column never fuses the two
    let b = run(SIDE, b',', false, "A1:C4 crop");
    assert_eq!(b.ncols(), 3);
    assert_eq!(b.cell(0, 0), "Code");
    assert_eq!(b.cell(0, 1), "Department");
    assert_eq!(b.cell(3, 2), "MW"); // Metro Water acronym, left table only
}

#[test]
fn describe_is_advisory() {
    // C11: describe reports regions and suspected total rows, never mutates
    let out = run_out(STACKED, b',', false, "describe");
    assert!(out.contains("rows ×"), "got: {out}");
    assert!(out.contains("suspected total"), "got: {out}");
    // and the buffer is untouched
    let b = run(STACKED, b',', false, "describe");
    assert_eq!(b.nrows(), 9);
}

#[test]
fn describe_flags_a_buried_header() {
    // preamble.csv read header-less: title block rows 1-3, blank row 4, real header row 5.
    // The narrow preamble defeats leading-blank-count; the modal-width heuristic catches it.
    let out = run_out(PREAMBLE, b',', false, "describe");
    assert!(out.contains("suspected header row: 5"), "got: {out}");
    assert!(out.contains("5 header"), "should suggest the actionable verb, got: {out}");
}

#[test]
fn describe_does_not_invent_a_header_on_a_clean_table() {
    // A table whose header is genuinely row 1 must not get a buried-header guess.
    let out = run_out(PRODUCTS, b',', true, "describe");
    assert!(!out.contains("suspected header row"), "false positive: {out}");
}
