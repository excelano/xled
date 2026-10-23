// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

//! The refusals and warnings that name the fix for a common mistake: `header N` written
//! command-first, a column name in the wrong case, and a title row read as the header.

use xled::{exec, io, parser};

fn error(csv: &str, cmd: &str) -> String {
    let mut buf = io::read_str(csv, b',', true).unwrap();
    let result = parser::parse_program(cmd).and_then(|p| exec::run(&mut buf, &p));
    match result {
        Ok(_) => panic!("the script should be refused"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn header_written_command_first_names_the_address_first_form() {
    assert_eq!(
        error("a,b\n1,2\n", "header 1"),
        "`header` takes its row as an address, written first: `1 header`"
    );
}

#[test]
fn a_name_in_the_wrong_case_suggests_the_right_one() {
    let csv = "id,name\n1,a\n";
    let hint = "no column named [Name] — names are case-sensitive: did you mean [name]?";
    assert_eq!(error(csv, "[Name]"), hint);
    assert_eq!(error(csv, "[Name]~/a/"), hint);
    assert_eq!(error(csv, "[x] = [Name]"), hint);
}

#[test]
fn a_name_with_no_near_match_is_refused_plainly() {
    assert_eq!(error("id,name\n1,a\n", "[cost]"), "no column named [cost]");
}

#[test]
fn a_title_row_read_as_the_header_points_at_the_real_one() {
    let buf = io::read_str("Report,,\nGenerated,,\nid,name,cost\n7,a,5\n", b',', true).unwrap();
    assert_eq!(buf.title_as_header(), Some(3));
}

#[test]
fn an_ordinary_header_is_not_a_title() {
    let buf = io::read_str("id,name,cost\n7,a,5\n", b',', true).unwrap();
    assert_eq!(buf.title_as_header(), None);
}

#[test]
fn a_blank_index_header_cell_is_not_a_title() {
    let buf = io::read_str(",a,b\n0,1,2\n1,3,4\n", b',', true).unwrap();
    assert_eq!(buf.title_as_header(), None);
}
