//! Slice-1 flags: `-i`/`--in-place` (edit the file like `sed -i`, optional backup suffix)
//! and `-f`/`--file` (read the script from a file like `sed -f`). Process-level tests: they
//! spawn the built binary so argv parsing, the `-i.bak` attached-suffix normalization, and
//! the file writes are all exercised end to end. See issues #5 and #6.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

const DATA: &str = "id,price\n001,$1,200\n002,$3\n";
const STRIP: &str = "[price] s/[$,]//g"; // $1,200 -> 1,200 ; $3 -> 3
const EXPECTED: &str = "id,price\n001,1,200\n002,3\n";

/// A unique path under `target/test-tmp` so parallel tests never collide on a file name.
fn tmp(name: &str) -> PathBuf {
    let dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/target/test-tmp"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn xled() -> Command {
    Command::new(env!("CARGO_BIN_EXE_xled"))
}

fn run(args: &[&str]) -> Output {
    xled().args(args).output().expect("spawn xled")
}

#[test]
fn in_place_edits_file_without_backup() {
    let path = tmp("inplace_nobackup.csv");
    fs::write(&path, DATA).unwrap();
    let out = run(&["-i", STRIP, path.to_str().unwrap()]);

    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(out.stdout.is_empty(), "-i must not also print to stdout");
    assert_eq!(fs::read_to_string(&path).unwrap(), EXPECTED);
    assert!(!tmp("inplace_nobackup.csv.bak").exists(), "no suffix → no backup");
    fs::remove_file(&path).ok();
}

#[test]
fn in_place_attached_suffix_keeps_original() {
    // sed's `-i.bak` idiom: the suffix is attached to the flag with no `=`.
    let path = tmp("inplace_bak.csv");
    fs::write(&path, DATA).unwrap();
    let out = run(&["-i.bak", STRIP, path.to_str().unwrap()]);

    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(fs::read_to_string(&path).unwrap(), EXPECTED, "target edited");
    let backup = tmp("inplace_bak.csv.bak");
    assert_eq!(fs::read_to_string(&backup).unwrap(), DATA, "backup holds the original");
    fs::remove_file(&path).ok();
    fs::remove_file(&backup).ok();
}

#[test]
fn in_place_long_form_with_equals_suffix() {
    let path = tmp("inplace_long.csv");
    fs::write(&path, DATA).unwrap();
    let out = run(&["--in-place=.orig", STRIP, path.to_str().unwrap()]);

    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(fs::read_to_string(&path).unwrap(), EXPECTED);
    let backup = tmp("inplace_long.csv.orig");
    assert_eq!(fs::read_to_string(&backup).unwrap(), DATA);
    fs::remove_file(&path).ok();
    fs::remove_file(&backup).ok();
}

#[test]
fn script_from_file() {
    let script = tmp("strip.xled");
    fs::write(&script, STRIP).unwrap();
    let data = tmp("scriptfile_data.csv");
    fs::write(&data, DATA).unwrap();

    let out = run(&["-f", script.to_str().unwrap(), data.to_str().unwrap()]);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout), EXPECTED, "-f runs the file's script to stdout");
    fs::remove_file(&script).ok();
    fs::remove_file(&data).ok();
}

#[test]
fn script_from_file_edited_in_place() {
    // #5 and #6 compose: read the script from a file, edit the data file in place.
    let script = tmp("strip2.xled");
    fs::write(&script, STRIP).unwrap();
    let data = tmp("scriptfile_inplace.csv");
    fs::write(&data, DATA).unwrap();

    let out = run(&["-i", "-f", script.to_str().unwrap(), data.to_str().unwrap()]);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(fs::read_to_string(&data).unwrap(), EXPECTED);
    fs::remove_file(&script).ok();
    fs::remove_file(&data).ok();
}

#[test]
fn in_place_refuses_query_script_and_leaves_file_intact() {
    // A read-only script under -i would overwrite the file with inspect output — refuse.
    let path = tmp("inplace_query.csv");
    fs::write(&path, DATA).unwrap();
    let out = run(&["-i", "[price]", path.to_str().unwrap()]);

    assert!(!out.status.success(), "must refuse an inspect-only script under -i");
    assert_eq!(fs::read_to_string(&path).unwrap(), DATA, "file left untouched");
    fs::remove_file(&path).ok();
}

#[test]
fn in_place_over_stdin_is_an_error() {
    let out = xled()
        .args(["-i", STRIP])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut c| {
            use std::io::Write;
            c.stdin.take().unwrap().write_all(DATA.as_bytes()).unwrap();
            c.wait_with_output()
        })
        .expect("run xled");
    assert!(!out.status.success(), "-i with no file (piped stdin) must error");
}

#[test]
fn script_file_conflicts_with_inline_script() {
    let script = tmp("conflict.xled");
    fs::write(&script, STRIP).unwrap();
    let data = tmp("conflict_data.csv");
    fs::write(&data, DATA).unwrap();

    let out = run(&["-f", script.to_str().unwrap(), STRIP, data.to_str().unwrap()]);
    assert!(!out.status.success(), "-f plus an inline script must be rejected");
    fs::remove_file(&script).ok();
    fs::remove_file(&data).ok();
}
