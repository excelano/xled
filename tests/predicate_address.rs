//! A leading function call as a row-set address: `regexmatch([org], "^APP$") del`.
//!
//! Before this, an address had to *be* a comparison — a bare bool-valued call read as column
//! letters and died with an out-of-range error naming a column nobody wrote. The atom is
//! widened to the call form only, so `and`/`or` chaining stays inexpressible rather than
//! merely rejected (`ebnf.md`, the combinator wall).

use xled::{exec, io, parser};

const ORGS: &str = "org,name\nAPP,a\nAPPLE,b\nSCAM,c\nR+D,d\n";

fn show(csv: &str, prog: &str) -> String {
    let mut buf = io::read_str(csv, b',', true).unwrap();
    let p = parser::parse_program(prog).unwrap();
    exec::run(&mut buf, &p).unwrap().output.join("\n")
}

/// The buffer's full contents after a run, so a `del`/assign is visible.
fn after(csv: &str, prog: &str) -> String {
    let mut buf = io::read_str(csv, b',', true).unwrap();
    let p = parser::parse_program(prog).unwrap();
    exec::run(&mut buf, &p).unwrap();
    io::serialize(&buf).unwrap()
}

fn err(csv: &str, prog: &str) -> String {
    let mut buf = io::read_str(csv, b',', true).unwrap();
    match parser::parse_program(prog) {
        Ok(p) => exec::run(&mut buf, &p).unwrap_err().to_string(),
        Err(e) => e.to_string(),
    }
}

// --- the widened atom ----------------------------------------------------

#[test]
fn leading_call_selects_rows() {
    assert_eq!(
        show(ORGS, r#"regexmatch([org], "^APP$") show"#),
        "org,name\nAPP,a"
    );
}

#[test]
fn bare_call_matches_the_explicit_comparison() {
    assert_eq!(
        show(ORGS, r#"regexmatch([org], "^APP$") show"#),
        show(ORGS, r#"regexmatch([org], "^APP$") == true show"#),
    );
}

#[test]
fn leading_call_scopes_a_delete() {
    assert_eq!(
        after(ORGS, r#"regexmatch([org], "^APP") del"#),
        "org,name\nSCAM,c\nR+D,d\n",
    );
}

#[test]
fn negation_applies_to_a_call() {
    assert_eq!(
        show(ORGS, r#"!regexmatch([org], "^APP$") show"#),
        "org,name\nAPPLE,b\nSCAM,c\nR+D,d",
    );
}

#[test]
fn parenthesized_call_is_the_same_atom() {
    assert_eq!(
        show(ORGS, r#"(regexmatch([org], "^APP$")) show"#),
        show(ORGS, r#"regexmatch([org], "^APP$") show"#),
    );
}

#[test]
fn call_intersects_a_column_address() {
    // The predicate picks the rows, the column address picks the column written.
    assert_eq!(
        after(ORGS, r#"regexmatch([org], "^APP$") [name] = "hit""#),
        "org,name\nAPP,hit\nAPPLE,b\nSCAM,c\nR+D,d\n",
    );
}

// --- what the widening must not disturb ----------------------------------

#[test]
fn assignment_still_reads_as_a_command() {
    // The scan meets the `=` before the `if(`, so this is an assign, not a predicate.
    assert_eq!(
        after(ORGS, r#"[keep] = if(regexmatch([org], "^APP$"), "y", "n")"#),
        "org,name,keep\nAPP,a,y\nAPPLE,b,n\nSCAM,c,n\nR+D,d,n\n",
    );
}

#[test]
fn a_reserved_word_is_still_a_command() {
    assert_eq!(after(ORGS, "2 del"), "org,name\nAPP,a\nSCAM,c\nR+D,d\n");
}

#[test]
fn a_reserved_word_with_a_paren_is_not_a_call() {
    // `del(` is a malformed command, not a function named del.
    assert!(err(ORGS, "del(x) show").contains("expected a command"));
}

#[test]
fn a_plain_column_address_is_untouched() {
    assert_eq!(show(ORGS, "A show"), "org\nAPP\nAPPLE\nSCAM\nR+D");
}

// --- the two corrections the widening exposes ----------------------------

#[test]
fn a_capability_word_in_call_position_reaches_its_catalog_entry() {
    // Bare `sum` collects the refusal at command position; `sum(` cannot get there, because
    // the paren denies it the space boundary. Both forms answer alike now.
    let e = err(ORGS, "sum([org]) show");
    assert!(e.contains("aggregation"), "{e}");
    assert!(e.contains("not in xled's scope"), "{e}");
}

#[test]
fn combinator_words_in_call_position_route_to_xql() {
    let e = err(ORGS, r#"and(regexmatch([org], "^APP$"), true) show"#);
    assert!(e.contains("combining conditions"), "{e}");
}

#[test]
fn a_call_that_is_not_a_test_halts_rather_than_selecting_nothing() {
    let e = err(ORGS, "upper([org]) show");
    assert!(e.contains("upper() produces text"), "{e}");
    assert!(e.contains("selects on true or false"), "{e}");
}
