//! Slice-1 output flags: `--raw` value-only output (issue #7) and `--number` logical
//! row-number prefixing (issue #4). Process-level tests feed CSV over stdin so the flag
//! parsing and the inspect-render path run end to end. The embedded-newline fixture is the
//! crux of #4: a line-based tool miscounts rows after a multiline cell, but xled's own
//! logical numbering stays in step.

use std::io::Write;
use std::process::{Command, Output, Stdio};

// Row 1's description holds an embedded newline (one logical row, two physical lines); row 3
// holds a comma. Both are the cases plain line/field tools get wrong.
const CSV: &str = "id,description\n001,\"multi\nline value\"\n002,plain\n003,\"has,comma\"\n";

fn run(args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_xled"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn xled");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().expect("wait for xled")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn raw_single_cell_is_just_the_value() {
    // The issue #7 headline: `[col] N` under --raw returns the bare value, no header line.
    let out = run(&["--raw", "[description] 2"], CSV);
    assert!(out.status.success());
    assert_eq!(stdout(&out), "plain\n");
}

#[test]
fn raw_column_is_values_one_per_line_unquoted() {
    let out = run(&["--raw", "[description]"], CSV);
    assert!(out.status.success());
    // No header, embedded newline and comma emitted verbatim (no CSV quoting).
    assert_eq!(stdout(&out), "multi\nline value\nplain\nhas,comma\n");
}

#[test]
fn number_csv_prefixes_logical_row_and_survives_embedded_newline() {
    let out = run(&["--number", "[description]"], CSV);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(
        text.starts_with("row,description\n"),
        "leading row column in header: {text:?}"
    );
    // Row 1's value spans two physical lines, yet row 2 is still numbered 2 and row 3 is 3 —
    // the numbering tracks logical rows, which is the whole point of #4.
    assert!(
        text.contains("\n2,plain\n"),
        "row 2 numbered logically: {text:?}"
    );
    assert!(
        text.contains("3,\"has,comma\""),
        "row 3 numbered logically: {text:?}"
    );
}

#[test]
fn raw_and_number_together_prefix_each_value_line() {
    let out = run(&["--raw", "--number", "[description]"], CSV);
    assert!(out.status.success());
    let text = stdout(&out);
    let mut lines = text.lines();
    assert_eq!(lines.next().unwrap(), "1\tmulti"); // number then tab then the value
    assert!(text.contains("\n2\tplain\n"), "row 2: {text:?}");
    assert!(text.contains("3\thas,comma"), "row 3: {text:?}");
}

#[test]
fn row_function_is_rejected_with_a_trail_to_number_flag() {
    // #4 was built as option 2 (--number) only; row() in a compute is deliberately not a thing,
    // and the error redirects to the flag rather than leaving a bare "unknown function".
    let out = run(&["[n] = row()"], CSV);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("--number"),
        "points at the --number flag: {err:?}"
    );
}

#[test]
fn raw_number_have_no_effect_on_a_mutation_but_do_not_fail() {
    // A mutation writes the whole table; the inspect flags don't apply. xled notes this on
    // stderr and still produces the transformed table on stdout.
    let out = run(&["--raw", "--number", "[id] s/0//g"], CSV);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(
        text.starts_with("id,description\n"),
        "still a full CSV table: {text:?}"
    );
    assert!(text.contains("\n1,"), "the s/0//g edit applied: {text:?}");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("had no effect"),
        "notes that the flags were inert: {err:?}"
    );
}
