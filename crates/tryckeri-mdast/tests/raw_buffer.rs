//! Integration tests for raw buffer export.

use tryckeri_arena::{ArenaBuilder, NODE_STRUCT_SIZE};
use tryckeri_mdast::{encode_heading_data, MdastNodeType};

fn build_test_arena() -> tryckeri_arena::Arena {
    let mut builder = ArenaBuilder::new("# Hello\n\nParagraph.".to_string());

    builder.open_node(MdastNodeType::Root as u8);
    builder.set_position_current(0, 20, 1, 1, 3, 11);

    let heading = builder.open_node(MdastNodeType::Heading as u8);
    builder.set_position_current(0, 7, 1, 1, 1, 8);
    builder.set_data_current(&encode_heading_data(1));

    builder.add_leaf(MdastNodeType::Text as u8); // "Hello"
    builder.close_node(); // heading

    let _para = builder.open_node(MdastNodeType::Paragraph as u8);
    builder.set_position_current(9, 19, 3, 1, 3, 11);
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
    let arena = tryckeri_arena::Arena::new(String::new());
    let buf = arena.to_raw_buffer();
    assert!(!buf.is_empty());
}
