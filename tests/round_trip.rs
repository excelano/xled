// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

//! Writing a table back changes only the rows a script edited: line terminators, quoting
//! style, blank lines, a missing final newline and a BOM all survive.

use xled::{exec, io, parser};

fn run(csv: &str, has_header: bool, cmd: &str) -> String {
    let mut buf = io::read_str(csv, b',', has_header).unwrap();
    let p = parser::parse_program(cmd).unwrap();
    exec::run(&mut buf, &p).unwrap();
    io::serialize(&buf)
}

fn round_trip(csv: &str) -> String {
    io::serialize(&io::read_str(csv, b',', true).unwrap())
}

#[test]
fn crlf_needless_quotes_and_blank_lines_round_trip() {
    let csv = "a,b\r\n\"x\",1\r\n,\r\n\r\nq,2\r\n";
    assert_eq!(round_trip(csv), csv);
}

#[test]
fn an_edit_rewrites_only_its_own_row() {
    let csv = "a,b\r\n\"x\",1\r\n\r\n\"q\",2\r\n";
    assert_eq!(
        run(csv, true, "[b]~/2/ s/2/3/"),
        "a,b\r\n\"x\",1\r\n\r\nq,3\r\n"
    );
}

#[test]
fn missing_final_newline_and_bom_survive() {
    assert_eq!(round_trip("a,b\n1,2"), "a,b\n1,2");
    assert_eq!(round_trip("\u{FEFF}id\n007\n"), "\u{FEFF}id\n007\n");
}

#[test]
fn bom_is_not_part_of_the_first_header_name() {
    let buf = io::read_str("\u{FEFF}id,n\n1,2\n", b',', true).unwrap();
    assert_eq!(buf.name_to_col("id"), Some(0));
}

#[test]
fn blank_lines_are_rows_so_addresses_match_the_file() {
    let csv = "Title,,\n\nid,name,cost\n007,ann,5\n";
    assert_eq!(
        run(csv, false, "A3:C4 crop\n1 header"),
        "id,name,cost\n007,ann,5\n"
    );
}

#[test]
fn single_column_empty_values_keep_their_lines() {
    let csv = "v\nx\n\ny\n";
    assert_eq!(run(csv, true, "[v]~/x/ s/x/z/"), "v\nz\n\ny\n");
}

#[test]
fn embedded_newline_in_a_quoted_field_round_trips() {
    let csv = "\"a\n b\",c\r\n1,2\r\n";
    assert_eq!(round_trip(csv), csv);
}
