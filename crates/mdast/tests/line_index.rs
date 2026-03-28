//! Integration tests for LineIndex.

use tryckeri_mdast::LineIndex;

#[test]
fn single_line_offset_zero_is_line1_col1() {
    let idx = LineIndex::from_source("hello");
    assert_eq!(idx.offset_to_line_col(0), (1, 1));
}

#[test]
fn single_line_last_char() {
    let idx = LineIndex::from_source("hello");
    assert_eq!(idx.offset_to_line_col(4), (1, 5));
    assert_eq!(idx.line_count(), 1);
}

#[test]
fn two_lines_newline_boundary() {
    // "hi\nbye"
    // line 1: offsets 0,1  (\n at 2)
    // line 2: offsets 3,4,5
    let idx = LineIndex::from_source("hi\nbye");
    assert_eq!(idx.offset_to_line_col(0), (1, 1)); // 'h'
    assert_eq!(idx.offset_to_line_col(1), (1, 2)); // 'i'
                                                   // offset 2 is '\n' — still line 1
    assert_eq!(idx.offset_to_line_col(2), (1, 3));
    assert_eq!(idx.offset_to_line_col(3), (2, 1)); // 'b'
    assert_eq!(idx.offset_to_line_col(5), (2, 3)); // 'e'
    assert_eq!(idx.line_count(), 2);
}

#[test]
fn multi_line_document() {
    let source = "line1\nline2\nline3";
    let idx = LineIndex::from_source(source);
    // "line1" = 0..4, '\n' at 5
    // "line2" starts at 6, '\n' at 11
    // "line3" starts at 12
    assert_eq!(idx.line_count(), 3);
    assert_eq!(idx.offset_to_line_col(0), (1, 1));
    assert_eq!(idx.offset_to_line_col(4), (1, 5));
    assert_eq!(idx.offset_to_line_col(6), (2, 1));
    assert_eq!(idx.offset_to_line_col(10), (2, 5));
    assert_eq!(idx.offset_to_line_col(12), (3, 1));
    assert_eq!(idx.offset_to_line_col(16), (3, 5));
}

#[test]
fn last_line_no_trailing_newline() {
    let source = "abc\ndef";
    let idx = LineIndex::from_source(source);
    // line 2 starts at offset 4
    assert_eq!(idx.line_count(), 2);
    assert_eq!(idx.offset_to_line_col(4), (2, 1));
    assert_eq!(idx.offset_to_line_col(6), (2, 3));
}

#[test]
fn empty_source() {
    let idx = LineIndex::from_source("");
    // One "line" (the empty document).
    assert_eq!(idx.line_count(), 1);
    assert_eq!(idx.offset_to_line_col(0), (1, 1));
}

#[test]
fn only_newline() {
    let idx = LineIndex::from_source("\n");
    assert_eq!(idx.line_count(), 2);
    assert_eq!(idx.offset_to_line_col(0), (1, 1));
    assert_eq!(idx.offset_to_line_col(1), (2, 1));
}
