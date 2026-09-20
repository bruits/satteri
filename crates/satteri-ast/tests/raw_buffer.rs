//! Integration tests for raw buffer export.

use satteri_arena::{ArenaBuilder, Mdast, NODE_STRUCT_SIZE};
use satteri_ast::mdast::{MdastNodeType, encode_heading_data};

fn build_test_arena() -> satteri_arena::Arena<Mdast> {
    let mut builder = ArenaBuilder::<Mdast>::new("# Hello\n\nParagraph.".to_string());

    builder.open_node(MdastNodeType::Root as u8);
    builder.set_position_current(satteri_arena::NodePosition {
        start_offset: 0,
        end_offset: 20,
        start_line: 1,
        start_column: 1,
        end_line: 3,
        end_column: 11,
    });

    let heading = builder.open_node(MdastNodeType::Heading as u8);
    builder.set_position_current(satteri_arena::NodePosition {
        start_offset: 0,
        end_offset: 7,
        start_line: 1,
        start_column: 1,
        end_line: 1,
        end_column: 8,
    });
    builder.set_data_current(&encode_heading_data(1));

    builder.add_leaf(MdastNodeType::Text as u8); // "Hello"
    builder.close_node(); // heading

    let _para = builder.open_node(MdastNodeType::Paragraph as u8);
    builder.set_position_current(satteri_arena::NodePosition {
        start_offset: 9,
        end_offset: 19,
        start_line: 3,
        start_column: 1,
        end_line: 3,
        end_column: 11,
    });
    builder.add_leaf(MdastNodeType::Text as u8); // "Paragraph."
    builder.close_node(); // paragraph

    builder.close_node(); // root
    let arena = builder.finish();
    let _ = heading; // suppress unused warning
    arena
}

#[test]
fn header_magic_correct() {
    let arena = build_test_arena();
    let buf = arena.to_raw_buffer();
    assert_eq!(&buf[..4], b"MDAR");
}

#[test]
fn header_kind_tag_correct() {
    let arena = build_test_arena();
    let buf = arena.to_raw_buffer();
    // Kind tag for Mdast = 1 (see ArenaKind::KIND_TAG).
    let kind = u32::from_ne_bytes(buf[4..8].try_into().unwrap());
    assert_eq!(kind, 1);
}

#[test]
fn header_node_struct_size_correct() {
    let arena = build_test_arena();
    let buf = arena.to_raw_buffer();
    let nss = u32::from_ne_bytes(buf[8..12].try_into().unwrap());
    assert_eq!(nss as usize, NODE_STRUCT_SIZE);
}

#[test]
fn export_produces_non_empty_buffer() {
    let arena = build_test_arena();
    let buf = arena.to_raw_buffer();
    assert!(buf.len() > 12); // at least header + some data
}

#[test]
fn empty_arena_exports() {
    let arena = satteri_arena::Arena::<Mdast>::new(String::new());
    let buf = arena.to_raw_buffer();
    assert!(!buf.is_empty());
}
