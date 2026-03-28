//! Integration tests for type-specific data codec.

use tryckeri_mdast::{
    decode_code_data, decode_heading_data, decode_link_data, decode_list_data, decode_table_data,
    encode_code_data, encode_heading_data, encode_link_data, encode_list_data, encode_table_data,
    ColumnAlign, MdastBuilder, MdastNodeType, StringRef,
};

#[test]
fn encode_decode_heading_data() {
    for depth in 1u8..=6 {
        let bytes = encode_heading_data(depth);
        let d = decode_heading_data(&bytes);
        assert_eq!(d.depth, depth, "depth {} failed", depth);
    }
}

#[test]
fn encode_decode_link_data_with_url() {
    let _source = "https://example.com title text";
    let url = StringRef::new(0, 19);
    let title = StringRef::new(20, 10);
    let bytes = encode_link_data(url, title);
    let d = decode_link_data(&bytes);
    assert_eq!(d.url, url);
    assert_eq!(d.title, title);
    assert!(!d.title.is_empty());
}

#[test]
fn encode_decode_link_data_no_title() {
    let url = StringRef::new(0, 15);
    let title = StringRef::empty();
    let bytes = encode_link_data(url, title);
    let d = decode_link_data(&bytes);
    assert_eq!(d.url, url);
    assert!(d.title.is_empty(), "title should be absent (len==0)");
}

#[test]
fn encode_decode_code_data() {
    let lang = StringRef::new(0, 2);
    let meta = StringRef::new(3, 8);
    let bytes = encode_code_data(lang, meta, StringRef::empty(), b'`');
    let d = decode_code_data(&bytes);
    assert_eq!(d.lang, lang);
    assert_eq!(d.meta, meta);
    assert_eq!(d.fence_char, b'`');
}

#[test]
fn encode_decode_list_data_ordered() {
    let bytes = encode_list_data(true, 3, false);
    let d = decode_list_data(&bytes);
    assert!(d.ordered);
    assert_eq!(d.start, 3);
    assert!(!d.spread);
}

#[test]
fn encode_decode_list_data_unordered() {
    let bytes = encode_list_data(false, 0, true);
    let d = decode_list_data(&bytes);
    assert!(!d.ordered);
    assert_eq!(d.start, 0);
    assert!(d.spread);
}

#[test]
fn encode_decode_table_data_alignments() {
    let aligns = vec![
        ColumnAlign::None,
        ColumnAlign::Left,
        ColumnAlign::Right,
        ColumnAlign::Center,
    ];
    let bytes = encode_table_data(&aligns);
    let (hdr, decoded) = decode_table_data(&bytes);
    assert_eq!(hdr.align_count, 4);
    assert_eq!(decoded, aligns);
}

#[test]
fn encode_decode_table_data_empty() {
    let bytes = encode_table_data(&[]);
    let (hdr, decoded) = decode_table_data(&bytes);
    assert_eq!(hdr.align_count, 0);
    assert!(decoded.is_empty());
}

#[test]
fn type_data_stored_in_arena() {
    let mut builder = MdastBuilder::new("# Title".to_string());
    builder.open_node(MdastNodeType::Root);
    let heading = builder.open_node(MdastNodeType::Heading);
    builder.set_data_current(&encode_heading_data(2));
    builder.add_leaf(MdastNodeType::Text);
    builder.close_node();
    builder.close_node();
    let arena = builder.finish();

    let node = arena.get_node(heading);
    let raw = &arena.arena_type_data()[node.data_offset as usize..][..node.data_len as usize];
    let d = decode_heading_data(raw);
    assert_eq!(d.depth, 2);
}
