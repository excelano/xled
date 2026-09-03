//! Slice 3 conformance: Part B9 (compute/derive), B10 (conditional/blank),
//! A3 (single comparison as scope; combinators refused), A6 (create column by assign).

use xled::{exec, io, parser, Buffer};

const PRODUCTS: &str = include_str!("../fixtures/products.csv");
const INVENTORY: &str = include_str!("../fixtures/inventory.tsv");
const CONTACTS: &str = include_str!("../fixtures/contacts.csv");
const APP: &str = include_str!("../fixtures/app-portfolio.csv");

fn run(csv: &str, delim: u8, prog: &str) -> Buffer {
    let mut buf = io::read_str(csv, delim, true).unwrap();
    let p = parser::parse_program(prog).unwrap();
    exec::run(&mut buf, &p).unwrap();
    buf
}

fn notices(csv: &str, delim: u8, prog: &str) -> Vec<String> {
    let mut buf = io::read_str(csv, delim, true).unwrap();
    let p = parser::parse_program(prog).unwrap();
    exec::run(&mut buf, &p).unwrap().notices
}

fn err(csv: &str, delim: u8, prog: &str) -> String {
    let mut buf = io::read_str(csv, delim, true).unwrap();
    let p = match parser::parse_program(prog) {
        Ok(p) => p,
        Err(e) => return e.to_string(),
    };
    exec::run(&mut buf, &p).unwrap_err().to_string()
}

// --- the lenient cast-skip tally rides the result channel, not stderr ----

#[test]
fn uncomputable_cells_raise_a_notice_not_a_stderr_aside() {
    // [status] is all words; num() can't cast any of them, so every row is skipped. The
    // tally comes back as a notice on the Outcome (a real result), not an eprintln! that
    // bypasses the caller. [score] is new here, so the skipped cells are empty, not kept.
    let n = notices(APP, b',', r"[score] = num([status])");
    assert_eq!(n.len(), 1);
    assert!(n[0].contains("skipped"));
    assert!(n[0].contains("left empty"), "got: {}", n[0]);
}

#[test]
fn a_skip_into_an_existing_column_keeps_what_was_there() {
    // The other half of the same notice: assigning into a column that already holds
    // values leaves those values, so the wording has to say so. Getting these two
    // confused is what made a skipped new column look harmless.
    let n = notices(APP, b',', r"[criticality] = num([status])");
    assert_eq!(n.len(), 1);
    assert!(n[0].contains("left unchanged"), "got: {}", n[0]);
}

#[test]
fn a_fully_computable_assign_raises_no_notice() {
    let n = notices(PRODUCTS, b',', r"[bump] = num([price]) * 2");
    assert!(n.is_empty());
}

// --- B9 compute / A6 create ----------------------------------------------

#[test]
fn arithmetic_creates_a_new_column() {
    // inventory: sku,location,qty,reorder,supplier — [total] is column index 5 (new)
    let b = run(INVENTORY, b'\t', r"[total] = num([qty]) * num([reorder])");
    assert_eq!(b.cell(0, 5), "6000"); // 120 * 50, integral → no decimal point
    assert_eq!(b.col_name(5), Some("total"));
}

#[test]
fn boolean_column() {
    // qty < reorder → a bool column; numeric order needs the num() cast
    let b = run(INVENTORY, b'\t', r"[low] = num([qty]) < num([reorder])");
    assert_eq!(b.cell(0, 5), "false"); // 120 < 50
    assert_eq!(b.cell(1, 5), "true"); // 8 < 25
}

#[test]
fn create_by_letter_past_width() {
    // products is 4 columns (A..D); assigning to E creates column index 4
    let b = run(PRODUCTS, b',', r"E = round(num([price]) * 1.1, 2)");
    assert_eq!(b.cell(0, 4), "21.99"); // 19.99 * 1.1, rounded
}

#[test]
fn concatenation() {
    let b = run(CONTACTS, b',', r#"[full] = [name] & " - " & [company]"#);
    assert_eq!(b.cell(0, 4), "Alice Nguyen - Acme Corp");
}

#[test]
fn string_functions() {
    let b = run(PRODUCTS, b',', r"[init] = left([name], 1)");
    assert_eq!(b.cell(0, 4), "W");
    let b = run(PRODUCTS, b',', r"[n] = len([name])");
    assert_eq!(b.cell(0, 4), "10"); // "Widget Pro"
}

// --- B10 conditional / blank ---------------------------------------------

#[test]
fn default_blank() {
    let b = run(APP, b',', r#"[owner] = default([owner], "Unassigned")"#);
    assert_eq!(b.cell(4, 1), "Unassigned"); // DocVault had no owner
    assert_eq!(b.cell(0, 1), "Finance Dept"); // present → untouched
}

#[test]
fn if_expression() {
    let b = run(
        INVENTORY,
        b'\t',
        r#"[flag] = if(num([qty]) < num([reorder]), "REORDER", "ok")"#,
    );
    assert_eq!(b.cell(0, 5), "ok"); // 120 < 50 false
    assert_eq!(b.cell(1, 5), "REORDER"); // 8 < 25 true
}

// --- A3 comparison as scope, combinators refused -------------------------

#[test]
fn comparison_scopes_an_edit() {
    let b = run(
        INVENTORY,
        b'\t',
        r#"num([qty]) < num([reorder]) [supplier] = "REORDER""#,
    );
    assert_eq!(b.cell(1, 4), "REORDER"); // qty 8 < 25 → edited
    assert_eq!(b.cell(0, 4), "Northwind"); // qty 120 not < 50 → untouched
}

#[test]
fn combinator_is_refused() {
    let msg = err(
        INVENTORY,
        b'\t',
        r"num([qty])<num([reorder]) and [supplier]~/Contoso/ show",
    );
    assert!(msg.contains("not in xled's scope"), "got: {msg}");
    assert!(msg.contains("xql"), "got: {msg}");
}

#[test]
fn assign_to_two_columns_is_refused() {
    let msg = err(PRODUCTS, b',', r#"[name],[sku] = "x""#);
    assert!(msg.contains("assignment writes exactly one"), "got: {msg}");
}

// --- B12 multi-step chaining ---------------------------------------------

#[test]
fn chain_scrub_then_derive() {
    // strip currency in two columns, then qty-style derive from the cleaned value
    let prog = "[price] s/[$,]//g\n[bump] = round(num([price]) * 2, 2)";
    let b = run(PRODUCTS, b',', prog);
    assert_eq!(b.cell(0, 4), "39.98"); // 19.99 * 2
}

// --- a materialized column reaches every row (#21) ------------------------

/// Field counts per serialized line, header first. A table is rectangular when they
/// are all equal; the bugs behind #21 were visible only at this level, because both
/// the buffer and `describe` reported the widest row as the column count.
fn widths(b: &Buffer) -> Vec<usize> {
    io::serialize(b)
        .unwrap()
        .lines()
        .map(|l| l.split(',').count())
        .collect()
}

#[test]
fn a_partly_skipped_new_column_still_reaches_every_row() {
    // round() over a column with one uncastable value: row 2 skips. It must still get
    // the field, or the header claims a column the row does not have.
    let csv = "sku,price\nP1,10.00\nP2,n/a\n";
    let b = run(csv, b',', r"[total] = round(num([price]) * 2, 2)");
    assert_eq!(widths(&b), vec![3, 3, 3]);
    assert_eq!(b.cell(0, 2), "20");
    assert_eq!(b.cell(1, 2), "", "a skipped new cell is empty, not absent");
}

#[test]
fn a_wholly_skipped_new_column_still_reaches_every_row() {
    // The forgotten-num() case: arithmetic on raw text skips every row. The column is
    // created and empty — the one shape where nothing computed at all.
    let csv = "sku,price\nP1,10.00\nP2,n/a\n";
    let b = run(csv, b',', r"[total] = [price] * 2");
    assert_eq!(widths(&b), vec![3, 3, 3]);
    assert_eq!(b.col_name(2), Some("total"));
    assert_eq!(b.cell(0, 2), "");
    assert_eq!(b.cell(1, 2), "");
}

#[test]
fn a_new_column_addressed_by_letter_widens_the_header_too() {
    // The mirror-image raggedness: a letter target never carried a name, so it grew the
    // rows and left the header a column short — on the path where every row computed.
    let csv = "sku,price\nP1,10\nP2,20\n";
    let b = run(csv, b',', r"C = num([price]) * 2");
    assert_eq!(widths(&b), vec![3, 3, 3]);
    assert_eq!(b.col_name(2), Some(""), "a letter target names nothing");
    assert_eq!(b.cell(0, 2), "20");
    assert_eq!(b.cell(1, 2), "40");
}

#[test]
fn a_skip_into_an_existing_column_leaves_a_short_row_short() {
    // The fix is scoped to a column being created. Ragged input assigned into a column
    // that already exists is left as it was found — xled does not silently rectangularize
    // a file it was not asked to reshape.
    let csv = "a,b,c\n1,2,3\n4,5\n";
    let b = run(csv, b',', r"[b] = num([b]) * 2");
    assert_eq!(widths(&b), vec![3, 3, 2]);
}
