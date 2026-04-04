//! Integration tests for LineIndex (cursor-based API).

use tryckeri_arena::LineIndex;

#[test]
fn single_line_offset_zero_is_line1_col1() {
    let idx = LineIndex::from_source("hello");
    let mut c = idx.cursor();
    assert_eq!(c.offset_to_line_col(0), (1, 1));
}

#[test]
fn single_line_last_char() {
    let idx = LineIndex::from_source("hello");
    let mut c = idx.cursor();
    assert_eq!(c.offset_to_line_col(4), (1, 5));
}

#[test]
fn two_lines_newline_boundary() {
    let idx = LineIndex::from_source("hi\nbye");
    let mut c = idx.cursor();
    assert_eq!(c.offset_to_line_col(0), (1, 1));
    assert_eq!(c.offset_to_line_col(1), (1, 2));
    assert_eq!(c.offset_to_line_col(2), (1, 3));
    assert_eq!(c.offset_to_line_col(3), (2, 1));
    assert_eq!(c.offset_to_line_col(5), (2, 3));
}

#[test]
fn multi_line_document() {
    let source = "line1\nline2\nline3";
    let idx = LineIndex::from_source(source);
    let mut c = idx.cursor();
    assert_eq!(c.offset_to_line_col(0), (1, 1));
    assert_eq!(c.offset_to_line_col(4), (1, 5));
    assert_eq!(c.offset_to_line_col(6), (2, 1));
    assert_eq!(c.offset_to_line_col(10), (2, 5));
    assert_eq!(c.offset_to_line_col(12), (3, 1));
    assert_eq!(c.offset_to_line_col(16), (3, 5));
}

#[test]
fn last_line_no_trailing_newline() {
    let source = "abc\ndef";
    let idx = LineIndex::from_source(source);
    let mut c = idx.cursor();
    assert_eq!(c.offset_to_line_col(4), (2, 1));
    assert_eq!(c.offset_to_line_col(6), (2, 3));
}

#[test]
fn empty_source() {
    let idx = LineIndex::from_source("");
    let mut c = idx.cursor();
    assert_eq!(c.offset_to_line_col(0), (1, 1));
}

#[test]
fn only_newline() {
    let idx = LineIndex::from_source("\n");
    let mut c = idx.cursor();
    assert_eq!(c.offset_to_line_col(0), (1, 1));
    assert_eq!(c.offset_to_line_col(1), (2, 1));
}
