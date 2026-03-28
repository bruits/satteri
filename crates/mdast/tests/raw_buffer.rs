//! Integration tests for raw buffer export/import.

use tryckeri_mdast::{
    decode_heading_data, encode_heading_data, BufferError, MdastArena, MdastBuilder, MdastNodeType,
    BUFFER_MAGIC, BUFFER_VERSION, NODE_STRUCT_SIZE,
};

fn build_test_arena() -> MdastArena {
    let mut builder = MdastBuilder::new("# Hello\n\nParagraph.".to_string());

    builder.open_node(MdastNodeType::Root);
    builder.set_position_current(0, 20, 1, 1, 3, 11);

    let heading = builder.open_node(MdastNodeType::Heading);
    builder.set_position_current(0, 7, 1, 1, 1, 8);
    builder.set_data_current(&encode_heading_data(1));

    builder.add_leaf(MdastNodeType::Text); // "Hello"
    builder.close_node(); // heading

    let _para = builder.open_node(MdastNodeType::Paragraph);
    builder.set_position_current(9, 19, 3, 1, 3, 11);
    builder.add_leaf(MdastNodeType::Text); // "Paragraph."
    builder.close_node(); // paragraph

    builder.close_node(); // root
    let arena = builder.finish();
    let _ = heading; // suppress unused warning
    arena
}

#[test]
fn export_and_import_basic() {
    let arena = build_test_arena();
    let buf = arena.to_raw_buffer();
    let view = MdastArena::from_raw_buffer(&buf).expect("should parse successfully");
    assert_eq!(view.node_count(), arena.len() as u32);
}

#[test]
fn header_magic_correct() {
    let arena = build_test_arena();
    let buf = arena.to_raw_buffer();
    assert_eq!(&buf[..4], &BUFFER_MAGIC);
}

#[test]
fn header_version_correct() {
    let arena = build_test_arena();
    let buf = arena.to_raw_buffer();
    let version = u32::from_ne_bytes(buf[4..8].try_into().unwrap());
    assert_eq!(version, BUFFER_VERSION);
}

#[test]
fn header_node_struct_size_correct() {
    let arena = build_test_arena();
    let buf = arena.to_raw_buffer();
    // node_struct_size is the third u32 field (after magic[4] and version u32)
    let nss = u32::from_ne_bytes(buf[8..12].try_into().unwrap());
    assert_eq!(nss as usize, NODE_STRUCT_SIZE);
}

#[test]
fn all_nodes_round_trip() {
    let arena = build_test_arena();
    let buf = arena.to_raw_buffer();
    let view = MdastArena::from_raw_buffer(&buf).unwrap();

    for i in 0..arena.len() as u32 {
        let orig = arena.get_node(i);
        let restored = view.get_node(i);
        assert_eq!(orig.id, restored.id, "node {} id mismatch", i);
        assert_eq!(
            orig.node_type, restored.node_type,
            "node {} type mismatch",
            i
        );
        assert_eq!(orig.parent, restored.parent, "node {} parent mismatch", i);
        assert_eq!(orig.children_count, restored.children_count);
        assert_eq!(orig.data_len, restored.data_len);
    }
}

#[test]
fn children_round_trip() {
    let arena = build_test_arena();
    let buf = arena.to_raw_buffer();
    let view = MdastArena::from_raw_buffer(&buf).unwrap();

    for i in 0..arena.len() as u32 {
        let orig_children = arena.get_children(i);
        let view_children = view.get_children(i);
        assert_eq!(
            orig_children, view_children,
            "children mismatch for node {}",
            i
        );
    }
}

#[test]
fn type_data_round_trip() {
    let arena = build_test_arena();
    let buf = arena.to_raw_buffer();
    let view = MdastArena::from_raw_buffer(&buf).unwrap();

    // Node 1 is the Heading with HeadingData.
    let heading_node = arena.get_node(1);
    assert_eq!(heading_node.node_type, MdastNodeType::Heading as u8);

    let orig_data = &arena.arena_type_data()[heading_node.data_offset as usize..]
        [..heading_node.data_len as usize];
    let view_data = view.get_type_data(1);

    assert_eq!(orig_data, view_data);
    let d = decode_heading_data(view_data);
    assert_eq!(d.depth, 1);
}

#[test]
fn source_round_trip() {
    let arena = build_test_arena();
    let buf = arena.to_raw_buffer();
    let view = MdastArena::from_raw_buffer(&buf).unwrap();
    assert_eq!(view.get_source(), arena.source());
}

#[test]
fn bad_magic_rejected() {
    let arena = build_test_arena();
    let mut buf = arena.to_raw_buffer();
    buf[0] = b'X';
    let err = MdastArena::from_raw_buffer(&buf).unwrap_err();
    assert_eq!(err, BufferError::BadMagic);
}

#[test]
fn too_short_rejected() {
    let err = MdastArena::from_raw_buffer(&[0u8; 4]).unwrap_err();
    assert_eq!(err, BufferError::TooShort);
}

#[test]
fn empty_arena_round_trips() {
    let arena = MdastArena::new(String::new());
    let buf = arena.to_raw_buffer();
    let view = MdastArena::from_raw_buffer(&buf).unwrap();
    assert_eq!(view.node_count(), 0);
    assert_eq!(view.get_source(), "");
}
