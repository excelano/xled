//! Slice 2 conformance: Part B2 (substitute), B3 (capture/rearrange), B4 (case),
//! B5 (whitespace), and A11 (logical cell value vs raw bytes).

use xled::{exec, io, parser, Buffer};

const APP: &str = include_str!("../fixtures/app-portfolio.csv");
const MONEY: &str = include_str!("../fixtures/messy-money.csv");
const CONTACTS: &str = include_str!("../fixtures/contacts.csv");
const DATES: &str = include_str!("../fixtures/mixed-dates.csv");
const QUOTED: &str = include_str!("../fixtures/quoted-hell.csv");

fn run(csv: &str, delim: u8, prog: &str) -> Buffer {
    let mut buf = io::read_str(csv, delim, true).unwrap();
    let p = parser::parse_program(prog).unwrap();
    exec::run(&mut buf, &p).unwrap();
    buf
}

// --- B2 substitute -------------------------------------------------------

#[test]
fn strip_currency_and_thousands() {
    // amount is column index 2; row 0 = "$5,000.00"
    let b = run(MONEY, b',', r"[amount] s/[$,]//g");
    assert_eq!(b.cell(0, 2), "5000.00");
}

#[test]
fn first_vs_global() {
    let first = run(MONEY, b',', r"[amount] s/0//");
    let all = run(MONEY, b',', r"[amount] s/0//g");
    assert_eq!(first.cell(0, 2), "$5,00.00"); // only the first 0 gone
    assert_eq!(all.cell(0, 2), "$5,.");
}

#[test]
fn case_insensitive_flag() {
    // status column index 3; "Active" matched case-insensitively
    let b = run(APP, b',', r"[status] s/active/done/i");
    assert_eq!(b.cell(0, 3), "done");
}

#[test]
fn fill_blank_cell() {
    // DocVault (row 4) has an empty owner (column 1)
    let b = run(APP, b',', r"[owner] s/^$/Unassigned/");
    assert_eq!(b.cell(4, 1), "Unassigned");
    assert_eq!(b.cell(0, 1), "Finance Dept"); // non-empty left alone
}

#[test]
fn scoped_to_a_column_only() {
    // stripping $ in [amount] must not touch [balance]
    let b = run(MONEY, b',', r"[amount] s/\$//g");
    assert_eq!(b.cell(0, 2), "5,000.00");
    assert_eq!(b.cell(0, 3), "$5,000.00");
}

// --- B3 capture & rearrange ----------------------------------------------

#[test]
fn date_reorder_via_captures() {
    // only the M/D/Y row matches; ISO rows are left alone
    let b = run(DATES, b',', r"[raw_date] s#(..)/(..)/(....)#\3-\1-\2#");
    assert_eq!(b.cell(1, 1), "2024-01-15"); // 01/15/2024 → ISO
    assert_eq!(b.cell(0, 1), "2024-01-15"); // already ISO, unchanged
}

#[test]
fn extract_email_domain() {
    let b = run(CONTACTS, b',', r"[email] s/.*@(.*)/\1/");
    assert_eq!(b.cell(0, 1), "EXAMPLE.com");
}

// --- B4 case -------------------------------------------------------------

#[test]
fn lowercase_then_titlecase() {
    let lower = run(CONTACTS, b',', r"[email] s/.*/\L&/");
    assert_eq!(lower.cell(0, 1), "alice.nguyen@example.com");

    let title = run(CONTACTS, b',', r"[name] s/\b(.)/\U\1/g");
    assert_eq!(title.cell(3, 0), "Dave Park"); // "dave park" → "Dave Park"
}

#[test]
fn normalize_categorical() {
    let b = run(APP, b',', r"[status] s/^(active|in use)$/active/i");
    assert_eq!(b.cell(0, 3), "active"); // Active
    assert_eq!(b.cell(1, 3), "active"); // active
    assert_eq!(b.cell(2, 3), "active"); // IN USE
}

// --- B5 whitespace -------------------------------------------------------

#[test]
fn trim_trailing_space() {
    // TimeTracker (row 2) owner = "IT Operations "
    let b = run(APP, b',', r"[owner] s/^ +| +$//g");
    assert_eq!(b.cell(2, 1), "IT Operations");
}

// --- A11 logical value vs raw bytes --------------------------------------

#[test]
fn substitute_sees_parsed_value_not_quotes() {
    // address column index 2; replace commas only in rows whose address has one
    let b = run(QUOTED, b',', r"[address]~/,/ [address] s/,/;/g");
    assert_eq!(b.cell(0, 2), "123 Main St; Apt 4B; Springfield");
    assert_eq!(b.cell(1, 2), "45 Oak Ave"); // no comma → untouched
}

#[test]
fn embedded_newline_stays_one_cell() {
    let buf = io::read_str(QUOTED, b',', true).unwrap();
    // record 2 (row index 1) note spans three lines but is a single cell
    assert!(buf.cell(1, 3).contains('\n'));
    assert!(buf.cell(1, 3).starts_with("Multi-line"));
}
