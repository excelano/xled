//! Dates in `= expr` — the fourth type (expr-grammar.md, Dates). Library-level like
//! `expr_functions.rs`: read an inline CSV, run the program, read the computed column back.
//! New columns append at the original width, so a single `[out] = …` assign lands at column
//! index 2 in these two-column fixtures.
//!
//! The tests that matter most here are not the happy paths but the two refusals: xled never
//! guesses DD/MM versus MM/DD, and it sorts a bad *value* (skip the cell, tally) from a bad
//! *program* (halt once) rather than treating every failure the same way.

use xled::{exec, io, parser, Buffer};

const ISO: &str = "id,d\n1,2024-03-04\n2,2023-07-15\n";
// row 0 reads either way and disagrees; row 1 reads only as DD/MM; row 2 is not a date at all
const EURO: &str = "id,d\n1,03/04/2024\n2,15/07/2023\n3,sometime\n";

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

// --- the headline: normalize a column, in place, with no formatting call ------------------

#[test]
fn casting_a_column_normalizes_it_to_iso() {
    let b = run(EURO, r#"[d] = date([d], "DD/MM/YYYY")"#);
    assert_eq!(b.cell(0, 1), "2024-04-03"); // DD/MM, as spelled — not March 4th
    assert_eq!(b.cell(1, 1), "2023-07-15");
    assert_eq!(b.cell(2, 1), "sometime"); // unreadable: the cell is left exactly as it was
}

#[test]
fn an_unreadable_value_is_skipped_and_tallied_not_fatal() {
    let n = notices(EURO, r#"[d] = date([d], "DD/MM/YYYY")"#);
    assert_eq!(n.len(), 1);
    assert!(
        n[0].contains("skipped"),
        "the one bad row is tallied: {n:?}"
    );
}

// --- the refusal to guess -----------------------------------------------------------------

#[test]
fn a_bare_cast_reads_iso() {
    let b = run(ISO, r"[out] = date([d])");
    assert_eq!(b.cell(0, 2), "2024-03-04");
}

#[test]
fn an_ambiguous_value_halts_and_names_both_readings() {
    let e = err(EURO, r"[out] = date([d])");
    assert!(e.contains("ambiguous"), "{e}");
    assert!(
        e.contains("DD/MM/YYYY") && e.contains("MM/DD/YYYY"),
        "both readings named: {e}"
    );
}

#[test]
fn a_recognizable_but_unambiguous_layout_still_has_to_be_spelled_out() {
    // 15 is no month, so only DD/MM reads this — but accepting it would mean deciding the
    // layout per row, which is the guess the whole design refuses. Halt, and say so.
    let e = err("id,d\n1,15/07/2023\n", r"[out] = date([d])");
    assert!(e.contains("does not guess"), "{e}");
    assert!(
        e.contains("DD/MM/YYYY"),
        "the fix names the layout that would read it: {e}"
    );
}

#[test]
fn a_value_that_is_no_date_at_all_is_data_not_program() {
    // no layout reads it, so there is nothing to spell out — skip the cell, keep going
    let n = notices("id,d\n1,sometime\n", r"[out] = date([d])");
    assert_eq!(n.len(), 1);
    assert!(n[0].contains("skipped"), "{n:?}");
}

// --- arithmetic ---------------------------------------------------------------------------

#[test]
fn subtracting_two_dates_gives_days() {
    let b = run(
        "id,a,b\n1,2024-03-04,2024-01-01\n",
        r"[out] = date([a]) - date([b])",
    );
    assert_eq!(b.cell(0, 3), "63"); // 2024 is a leap year: 31 + 29 + 3
}

#[test]
fn offsetting_a_date_by_days_gives_a_date() {
    let b = run(ISO, r"[out] = date([d]) + 90");
    assert_eq!(b.cell(0, 2), "2024-06-02");
    let b = run(ISO, r"[out] = date([d]) - 90");
    assert_eq!(b.cell(0, 2), "2023-12-05");
}

#[test]
fn adding_two_dates_is_meaningless_and_skips() {
    let n = notices(ISO, r"[out] = date([d]) + date([d])");
    assert_eq!(n.len(), 1);
    assert!(n[0].contains("skipped"), "{n:?}");
}

#[test]
fn comparison_between_dates_is_chronological() {
    // the string-wise default would agree here by luck (ISO sorts), so compare across a
    // boundary where only real ordering is safe: same value, cast versus not
    let b = run(ISO, r#"[out] = date([d]) > date("2024-01-01")"#);
    assert_eq!(b.cell(0, 2), "true"); // 2024-03-04
    assert_eq!(b.cell(1, 2), "false"); // 2023-07-15
}

// --- components and formatting --------------------------------------------------------------

#[test]
fn components_come_out_as_numbers() {
    let b = run(ISO, r"[out] = year(date([d]))");
    assert_eq!(b.cell(0, 2), "2024");
    let b = run(ISO, r"[out] = month(date([d]))");
    assert_eq!(b.cell(0, 2), "3");
    let b = run(ISO, r"[out] = day(date([d]))");
    assert_eq!(b.cell(0, 2), "4");
}

#[test]
fn weekday_is_iso_one_is_monday() {
    // 2024-03-04 is a Monday, 2023-07-15 a Saturday. Under Excel's 1 = Sunday these would
    // be 2 and 7; xled is ISO, matching its own serialization.
    let b = run(ISO, r"[out] = weekday(date([d]))");
    assert_eq!(b.cell(0, 2), "1");
    assert_eq!(b.cell(1, 2), "6");
}

#[test]
fn text_renders_a_date_under_an_excel_format() {
    let b = run(ISO, r#"[out] = text(date([d]), "DDDD, D MMMM YYYY")"#);
    assert_eq!(b.cell(0, 2), "Monday, 4 March 2024");
    let b = run(ISO, r#"[out] = text(date([d]), "MM/DD/YY")"#);
    assert_eq!(b.cell(0, 2), "03/04/24");
}

#[test]
fn a_round_trip_through_text_and_back_is_lossless() {
    let b = run(
        ISO,
        r#"[out] = date(text(date([d]), "DD/MM/YYYY"), "DD/MM/YYYY")"#,
    );
    assert_eq!(b.cell(0, 2), "2024-03-04");
}

// --- the program errors ---------------------------------------------------------------------

#[test]
fn a_missing_cast_halts_with_the_corrected_form() {
    let e = err(ISO, r"[out] = year([d])");
    assert!(
        e.contains("year(date([col]))"),
        "the fix is shown, not described: {e}"
    );
}

#[test]
fn text_on_a_number_says_not_yet_rather_than_unknown() {
    let e = err(ISO, r#"[out] = text(1234.5, "0.00")"#);
    assert!(e.contains("later xled"), "{e}");
    assert!(e.contains("round("), "it points at what exists today: {e}");
}

#[test]
fn num_on_a_date_refuses_serial_numbers() {
    let e = err(ISO, r"[out] = num(date([d]))");
    assert!(e.contains("serial number"), "{e}");
    assert!(
        e.contains("year()"),
        "it names what the caller actually wanted: {e}"
    );
}

#[test]
fn a_format_with_no_year_is_rejected_before_any_row_runs() {
    let e = err(ISO, r#"[out] = date([d], "DD/MM")"#);
    assert!(e.contains("no year"), "{e}");
}

#[test]
fn date_rejects_wrong_arity() {
    let e = err(ISO, r"[out] = date()");
    assert!(e.contains("1 or 2 arguments"), "{e}");
}

// --- today() ----------------------------------------------------------------------------------

#[test]
fn today_is_one_date_for_the_whole_run() {
    // Not a clock assertion — the property is that every row gets the *same* answer, so a
    // run spanning midnight can't stamp two different days into one column.
    let b = run("id\n1\n2\n3\n", r"[out] = today()");
    assert_eq!(b.cell(0, 1), b.cell(1, 1));
    assert_eq!(b.cell(1, 1), b.cell(2, 1));
    assert_eq!(
        b.cell(0, 1).len(),
        10,
        "ISO, like every other date: {}",
        b.cell(0, 1)
    );
}
