//! Non-seekable FILE arguments: a named pipe, and `/dev/stdin` fed by a pipe.
//!
//! These used to read as an empty table at exit 0 (a FIFO did not return at all),
//! because the encoding sniff opened the path, drained it, and the real read then
//! opened a second time. Process-level tests: only a spawned binary can be handed
//! a FIFO path or a piped `/dev/stdin`. See issue #22.

#![cfg(unix)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

const DATA: &str = "id\n1\n2\n3\n";

/// How long a read of a three-line pipe may take before we call it a hang. Generous
/// enough for a loaded CI box; the regression it guards blocks forever, not briefly.
const PATIENCE: Duration = Duration::from_secs(10);

fn tmp(name: &str) -> PathBuf {
    let dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/target/test-tmp"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn xled() -> Command {
    Command::new(env!("CARGO_BIN_EXE_xled"))
}

/// Create a fresh FIFO at `path` and start a writer that feeds it `DATA`.
///
/// The writer is a shell so it survives independently of this thread: opening a FIFO
/// for writing blocks until a reader arrives, and the reader is the child under test.
fn fifo_with_writer(path: &Path) -> Child {
    fs::remove_file(path).ok();
    let ok = Command::new("mkfifo")
        .arg(path)
        .status()
        .expect("spawn mkfifo")
        .success();
    assert!(ok, "mkfifo {} failed", path.display());

    let mut w = Command::new("sh")
        .arg("-c")
        .arg(format!("cat > {}", shell_quote(path)))
        .stdin(Stdio::piped())
        .spawn()
        .expect("spawn fifo writer");
    w.stdin
        .take()
        .expect("writer stdin")
        .write_all(DATA.as_bytes())
        .expect("feed fifo writer");
    w
}

fn shell_quote(p: &Path) -> String {
    format!("'{}'", p.display().to_string().replace('\'', r"'\''"))
}

/// Wait for `child`, failing the test instead of hanging if it outlives `PATIENCE`.
fn wait_impatiently(mut child: Child, what: &str) -> Output {
    let deadline = Instant::now() + PATIENCE;
    while Instant::now() < deadline {
        if child.try_wait().expect("try_wait").is_some() {
            return child.wait_with_output().expect("collect output");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child.kill().ok();
    child.wait().ok();
    panic!("{what} did not return within {PATIENCE:?} — it is blocked on a second open");
}

#[test]
fn a_fifo_reads_the_same_rows_as_a_regular_file() {
    let path = tmp("nonseekable_fifo.csv");
    let mut writer = fifo_with_writer(&path);

    let child = xled()
        .args(["--count", "/[0-9]/", path.to_str().unwrap()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn xled");
    let out = wait_impatiently(child, "xled reading a FIFO");

    writer.wait().ok();
    fs::remove_file(&path).ok();

    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "3");
}

#[test]
fn dev_stdin_from_a_pipe_matches_piped_stdin() {
    // The same bytes addressed two ways must give one answer.
    let piped = feed_stdin(&["--count", "/[0-9]/"]);
    let named = feed_stdin(&["--count", "/[0-9]/", "/dev/stdin"]);
    assert_eq!(
        String::from_utf8_lossy(&piped.stdout).trim(),
        "3",
        "piped stdin"
    );
    assert_eq!(
        String::from_utf8_lossy(&named.stdout).trim(),
        "3",
        "/dev/stdin named as the FILE argument"
    );
}

fn feed_stdin(args: &[&str]) -> Output {
    let mut child = xled()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn xled");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(DATA.as_bytes())
        .expect("write child stdin");
    wait_impatiently(child, "xled reading stdin")
}

#[test]
fn in_place_refuses_a_fifo() {
    // Reading a pipe works; rewriting one does not — fs::write would block until a
    // reader appeared. Refuse with a correction rather than hang.
    let path = tmp("nonseekable_inplace.csv");
    let mut writer = fifo_with_writer(&path);

    let child = xled()
        .args(["-i", "[id] = num([id]) * 2", path.to_str().unwrap()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn xled");
    let out = wait_impatiently(child, "xled -i on a FIFO");

    writer.kill().ok();
    writer.wait().ok();
    let still_a_fifo = fs::metadata(&path).map(|m| !m.is_file()).unwrap_or(false);
    fs::remove_file(&path).ok();

    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not a regular file"),
        "stderr should name the constraint, got: {stderr}"
    );
    assert!(still_a_fifo, "-i must not replace the FIFO with a file");
}
