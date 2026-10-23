// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

//! `--diff`: one line per changed cell, as a preview on its own and as the record of an
//! in-place write with `-i`.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use xled::{exec, io, parser, report};

const DATA: &str = "id,name,price\n007,ann ,\"$1,200\"\n008,bob,$30\n";

fn tmp(name: &str) -> PathBuf {
    let dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/target/test-tmp"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xled"))
        .args(args)
        .output()
        .expect("spawn xled")
}

fn diff(csv: &str, has_header: bool, cmd: &str) -> Vec<String> {
    let mut buf = io::read_str(csv, b',', has_header).unwrap();
    buf.edits.log = Some(Vec::new());
    exec::run(&mut buf, &parser::parse_program(cmd).unwrap()).unwrap();
    report::diff_lines(&buf)
}

#[test]
fn each_changed_cell_is_one_line_with_its_address() {
    assert_eq!(
        diff(DATA, true, "[price] s/[$,]//g\n[name] = trim([name])"),
        [
            r#"C1 [price]: "$1,200" → "1200""#,
            r#"C2 [price]: "$30" → "30""#,
            r#"B1 [name]: "ann " → "ann""#,
        ]
    );
}

#[test]
fn a_headerless_table_labels_cells_by_letter_only() {
    assert_eq!(diff("a,b\n", false, "B1 s/b/c/"), [r#"B1: "b" → "c""#]);
}

#[test]
fn a_cell_written_its_own_value_is_not_a_change() {
    assert!(diff(DATA, true, "[id] = [id]").is_empty());
}

#[test]
fn diff_without_in_place_previews_and_leaves_the_file() {
    let path = tmp("diff_preview.csv");
    fs::write(&path, DATA).unwrap();
    let out = run(&["--diff", "[price] s/[$,]//g", path.to_str().unwrap()]);
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "C1 [price]: \"$1,200\" → \"1200\"\nC2 [price]: \"$30\" → \"30\"\n"
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "xled: 2 cells changed in [price]\n"
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), DATA);
    fs::remove_file(&path).ok();
}

#[test]
fn diff_with_in_place_writes_and_records() {
    let path = tmp("diff_in_place.csv");
    fs::write(&path, DATA).unwrap();
    let out = run(&["-i", "--diff", "[price] s/[$,]//g", path.to_str().unwrap()]);
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "C1 [price]: \"$1,200\" → \"1200\"\nC2 [price]: \"$30\" → \"30\"\n"
    );
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "id,name,price\n007,ann ,1200\n008,bob,30\n"
    );
    fs::remove_file(&path).ok();
}
