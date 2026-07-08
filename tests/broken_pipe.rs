//! A reader that closes the pipe early (the classic `xled … | head`) must not panic.
//! xled should stop quietly like `cat` or `grep`, not print a Rust backtrace. See issue #3.

use std::io::{Read, Write};
use std::process::{Command, Stdio};

#[test]
fn broken_pipe_exits_without_panicking() {
    // Output has to outlast the reader closing the pipe, so the write is still in flight
    // when the read end goes away. A few MB clears the ~64 KB kernel pipe buffer easily.
    let mut input = String::from("id,description\n");
    for i in 0..200_000 {
        input.push_str(&format!("{i},row number {i} with some filler text\n"));
    }

    let mut child = Command::new(env!("CARGO_BIN_EXE_xled"))
        .arg("[description]")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn xled");

    // Feed stdin from a thread so a full stdout pipe can't deadlock the writer.
    let mut stdin = child.stdin.take().unwrap();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(input.as_bytes());
        // drop closes stdin
    });

    // Read a little, then drop stdout to close the read end early — exactly what `head` does.
    {
        let mut stdout = child.stdout.take().unwrap();
        let mut buf = [0u8; 64];
        let _ = stdout.read(&mut buf);
    }

    let output = child.wait_with_output().expect("wait for xled");
    let _ = writer.join();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("panicked"),
        "xled panicked on a broken pipe instead of exiting cleanly:\n{stderr}"
    );
}
