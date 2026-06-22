//! In-scope battery, the families the slice tests left thin: B6 anchors & boundaries,
//! B7 extraction/projection, B8 split & join. Oracle is the sed/awk equivalent noted per case.

use xled::{exec, io, parser, Buffer};

const PRODUCTS: &str = include_str!("../fixtures/products.csv");
const HEADERLESS: &str = include_str!("../fixtures/headerless.csv");
const CONTACTS: &str = include_str!("../fixtures/contacts.csv");
const QUOTED: &str = include_str!("../fixtures/quoted-hell.csv");

fn run(csv: &str, has_header: bool, cmd: &str) -> Buffer {
    let mut buf = io::read_str(csv, b',', has_header).unwrap();
    let p = parser::parse_program(cmd).unwrap();
    exec::run(&mut buf, &p).unwrap();
    buf
}

// --- B6 anchors & boundaries ---------------------------------------------

#[test]
fn whole_cell_anchor() {
    // ^tools$ matches only the cell that is exactly "tools", not "gizmos".
    let b = run(PRODUCTS, true, "[category] s/^tools$/T/");
    assert_eq!(b.cell(0, 1), "T"); // "tools" → "T"
    assert_eq!(b.cell(1, 1), "gizmos"); // untouched
}

#[test]
fn prefix_anchor() {
    // ^TL- only rewrites the SKUs that start with TL-.
    let b = run(HEADERLESS, false, "D s/^TL-/X-/");
    assert_eq!(b.cell(0, 3), "X-0042"); // TL-0042 → X-0042
    assert_eq!(b.cell(1, 3), "GZ-0101"); // not a TL- prefix → untouched
}

#[test]
fn word_boundary_anchor() {
    // \bInc\b matches "Widgets Inc" but the form selects the row.
    let b = run(CONTACTS, true, r"[company]~/\bInc\b/ [company] s/.*/MATCH/");
    assert_eq!(b.cell(1, 3), "MATCH"); // Widgets Inc
    assert_eq!(b.cell(0, 3), "Acme Corp"); // no \bInc\b → untouched
}

#[test]
fn anchors_bind_to_the_cell_not_the_line() {
    // The note in record 2 spans three physical lines; ^ and $ see one cell, so each
    // anchored insert happens exactly once, not once per embedded line.
    let b = run(QUOTED, true, r"[note] s/^/>> /");
    let multiline = b.cell(1, 3);
    assert!(multiline.starts_with(">> Multi-line"));
    assert_eq!(multiline.matches(">> ").count(), 1);
    assert!(multiline.contains("\nnote here")); // interior line not re-anchored
}

// --- B7 extraction / projection ------------------------------------------

#[test]
fn extract_a_capture_in_place() {
    // last four digits of the phone, via a full-cell match keeping only the group.
    let b = run(CONTACTS, true, r"[phone] s/.*(\d{4}).*/\1/");
    assert_eq!(b.cell(0, 2), "4567"); // (555) 123-4567
    assert_eq!(b.cell(3, 2), "7890"); // 5554567890
}

#[test]
fn keep_only_the_digits() {
    let b = run(CONTACTS, true, r"[phone] s/\D//g");
    assert_eq!(b.cell(0, 2), "5551234567");
    assert_eq!(b.cell(4, 2), "555567890112"); // "555-567-8901 ext 12" → digits only
}

// --- B8 split & join -----------------------------------------------------

#[test]
fn join_two_columns_with_a_separator() {
    // join is in scope: assignment concatenates into one column.
    let b = run(CONTACTS, true, r#"[label] = [name] & " <" & [company] & ">""#);
    assert_eq!(b.col_name(4), Some("label"));
    assert_eq!(b.cell(0, 4), "Alice Nguyen <Acme Corp>");
}

#[test]
fn split_to_columns_has_no_verb() {
    // splitting one cell into N columns is reshaping — out of scope, so there is no `split`
    // command; it falls through to the unknown-command error rather than silently reshaping.
    let mut buf = io::read_str(QUOTED, b',', true).unwrap();
    let parsed = parser::parse_program("[customer] split");
    // either the parse rejects it outright, or execution does — never a silent reshape.
    let rejected = match parsed {
        Err(_) => true,
        Ok(p) => exec::run(&mut buf, &p).is_err(),
    };
    assert!(rejected);
}
