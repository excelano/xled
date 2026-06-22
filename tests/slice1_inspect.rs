//! Slice 1 conformance: Part B1 (inspect/select) + B13 round-trip, against fixtures/.
//! Each case is `fixture + command → expected output`.

use xled::{exec, io, parser};

const PRODUCTS: &str = include_str!("../fixtures/products.csv");
const IDS_ZIPS: &str = include_str!("../fixtures/ids-zips.csv");

fn show(csv: &str, delim: u8, cmd: &str) -> String {
    let mut buf = io::read_str(csv, delim, true).unwrap();
    let program = parser::parse_program(cmd).unwrap();
    let out = exec::run(&mut buf, &program).unwrap();
    out.output.join("\n")
}

#[test]
fn column_by_name_equals_by_letter() {
    // B1: print column [price] ≡ column C
    assert_eq!(show(PRODUCTS, b',', "[price]"), show(PRODUCTS, b',', "C"));
}

#[test]
fn column_contents() {
    let expected = "price\n19.99\n9.50\n29.99\n4.25\n54.00\n12.75\n18.00\n7.99";
    assert_eq!(show(PRODUCTS, b',', "[price]"), expected);
}

#[test]
fn explicit_show_equals_bare_reference() {
    assert_eq!(show(PRODUCTS, b',', "[price] show"), show(PRODUCTS, b',', "[price]"));
}

#[test]
fn row_range() {
    // B1: rows 2–4, all columns
    let expected = "\
name,category,price,sku
Gadget,gizmos,9.50,GZ-0101
Pro Hammer,tools,29.99,TL-0099
Micro Sensor,electronics,4.25,EL-0007";
    assert_eq!(show(PRODUCTS, b',', "2:4"), expected);
}

#[test]
fn rectangle() {
    // B1: rectangle B2:C3
    let expected = "\
category,price
gizmos,9.50
tools,29.99";
    assert_eq!(show(PRODUCTS, b',', "B2:C3"), expected);
}

#[test]
fn single_cell() {
    // B1: cell B2
    assert_eq!(show(PRODUCTS, b',', "B2"), "category\ngizmos");
}

#[test]
fn regex_rows() {
    // B1: rows matching /tools/ (any cell)
    let expected = "\
name,category,price,sku
Widget Pro,tools,19.99,TL-0042
Pro Hammer,tools,29.99,TL-0099
Bench Vise,tools,54.00,TL-0153";
    assert_eq!(show(PRODUCTS, b',', "/tools/"), expected);
}

#[test]
fn open_ended_and_last_row() {
    // A5: 2: (to end) and $ (last row)
    let to_end = show(PRODUCTS, b',', "2:");
    assert!(to_end.ends_with("Safety Goggles,safety,7.99,SF-0003"));
    assert!(to_end.contains("Gadget,gizmos,9.50,GZ-0101"));
    assert_eq!(show(PRODUCTS, b',', "$"), "name,category,price,sku\nSafety Goggles,safety,7.99,SF-0003");
}

#[test]
fn negation_excludes_first_row() {
    // A9: every row except the first
    let out = show(PRODUCTS, b',', "!1");
    assert!(!out.contains("Widget Pro"));
    assert!(out.contains("Gadget,gizmos"));
}

#[test]
fn round_trip_preserves_leading_zeros_byte_for_byte() {
    // B13: read → serialize is identical, leading zeros intact
    let buf = io::read_str(IDS_ZIPS, b',', true).unwrap();
    assert_eq!(io::serialize(&buf).unwrap(), IDS_ZIPS);
}

#[test]
fn round_trip_products() {
    let buf = io::read_str(PRODUCTS, b',', true).unwrap();
    assert_eq!(io::serialize(&buf).unwrap(), PRODUCTS);
}
