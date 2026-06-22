//! Slice 5 conformance: Part B12 (REPL behaviour) — preview, undo, deliberate save,
//! dirty tracking, and one-shot/REPL parity.

use xled::{io, parser, session::Session};

const PRODUCTS: &str = include_str!("../fixtures/products.csv");

fn session() -> Session {
    Session::new(io::read_str(PRODUCTS, b',', true).unwrap(), None)
}

fn prog(src: &str) -> Vec<xled::Statement> {
    parser::parse_program(src).unwrap()
}

#[test]
fn preview_does_not_commit() {
    let sess = session();
    let before = sess.buf.cell(0, 2).to_string();
    let out = sess.preview(&prog(r"[price] s/.*/CHANGED/")).unwrap();
    assert!(out.contains("CHANGED")); // the preview shows the effect
    assert_eq!(sess.buf.cell(0, 2), before); // …but the buffer is untouched
    assert!(!sess.dirty);
}

#[test]
fn preview_shows_a_cast_skip_notice_above_the_result() {
    // num([category]) can't cast any row; the preview surfaces the skip tally as a banner
    // so the warning is seen *before* deciding to commit — its designated home.
    let sess = session();
    let out = sess.preview(&prog(r"[score] = num([category])")).unwrap();
    assert!(out.contains("skipped"));
    assert!(!sess.dirty); // still a no-op on the live buffer
}

#[test]
fn run_then_undo_restores() {
    let mut sess = session();
    let before = sess.buf.cell(0, 2).to_string();
    sess.run(&prog(r"[price] s/.*/CHANGED/")).unwrap();
    assert_eq!(sess.buf.cell(0, 2), "CHANGED");
    assert!(sess.dirty);

    assert!(sess.undo());
    assert_eq!(sess.buf.cell(0, 2), before);
    assert!(!sess.undo()); // nothing left
}

#[test]
fn inspect_does_not_dirty_or_snapshot() {
    let mut sess = session();
    sess.run(&prog("[price]")).unwrap(); // show
    assert!(!sess.dirty);
    assert!(!sess.undo()); // show created no undo point
}

#[test]
fn failed_mutation_leaves_buffer_intact() {
    let mut sess = session();
    let before = sess.buf.nrows();
    // partial-rectangle delete errors; the buffer must be unchanged
    assert!(sess.run(&prog("B2:C3 del")).is_err());
    assert_eq!(sess.buf.nrows(), before);
    assert!(!sess.dirty);
}

#[test]
fn deliberate_save_writes_and_clears_dirty() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/target/test-tmp");
    std::fs::create_dir_all(dir).unwrap();
    let path = format!("{dir}/save_roundtrip.csv");

    let mut sess = session();
    sess.run(&prog(r"[price] s/\..*//")).unwrap(); // strip the decimals
    assert!(sess.dirty);

    let written = sess.save(Some(&path)).unwrap();
    assert_eq!(written, path);
    assert!(!sess.dirty); // source adopted, now clean

    // read it back: the edit is on disk and round-trips
    let reread = io::read_str(&std::fs::read_to_string(&path).unwrap(), b',', true).unwrap();
    assert_eq!(reread.cell(0, 2), "19");
    std::fs::remove_file(&path).ok();
}

#[test]
fn oneshot_matches_repl_sequence() {
    // a two-line program run as a batch equals the same two lines run one at a time
    let batch = {
        let mut b = io::read_str(PRODUCTS, b',', true).unwrap();
        xled::run(&mut b, &prog("[price] s/[.]//g\n[sku] del")).unwrap();
        io::serialize(&b).unwrap()
    };
    let stepwise = {
        let mut s = session();
        s.run(&prog("[price] s/[.]//g")).unwrap();
        s.run(&prog("[sku] del")).unwrap();
        io::serialize(&s.buf).unwrap()
    };
    assert_eq!(batch, stepwise);
}
