//! Integration tests for StringRef and get_str.

use tryckeri_mdast::{
    decode_string_ref_data, encode_string_ref_data, MdastArena, MdastBuilder, MdastNodeType,
    StringRef,
};

#[test]
fn store_and_read_back_string_ref() {
    let source = "Hello, world!";
    let arena = MdastArena::new(source.to_string());

    let sr = StringRef::new(7, 5); // "world"
    assert_eq!(arena.get_str(sr), "world");
}

#[test]
fn multiple_string_refs_same_source() {
    let source = "foo bar baz";
    let arena = MdastArena::new(source.to_string());

    let foo = StringRef::new(0, 3);
    let bar = StringRef::new(4, 3);
    let baz = StringRef::new(8, 3);

    assert_eq!(arena.get_str(foo), "foo");
    assert_eq!(arena.get_str(bar), "bar");
    assert_eq!(arena.get_str(baz), "baz");
}

#[test]
fn empty_string_ref() {
    let arena = MdastArena::new("hello".to_string());
    let empty = StringRef::empty();
    assert_eq!(arena.get_str(empty), "");
    assert!(empty.is_empty());
}

#[test]
fn string_ref_whole_source() {
    let source = "complete source";
    let arena = MdastArena::new(source.to_string());
    let sr = StringRef::new(0, source.len() as u32);
    assert_eq!(arena.get_str(sr), source);
}

#[test]
fn string_ref_encoded_as_type_data() {
    // Text nodes store their content as a StringRef in type_data.
    let source = "hello world";
    let mut builder = MdastBuilder::new(source.to_string());
    builder.open_node(MdastNodeType::Root);
    let text_id = builder.open_node(MdastNodeType::Text);
    let sr = StringRef::new(6, 5);
    builder.set_data_current(&encode_string_ref_data(sr));
    builder.close_node(); // text
    builder.close_node(); // root

    let arena = builder.finish();
    let text_node = arena.get_node(text_id);
    let raw =
        &arena.arena_type_data()[text_node.data_offset as usize..][..text_node.data_len as usize];
    let decoded = decode_string_ref_data(raw);
    assert_eq!(decoded, sr);
    assert_eq!(arena.get_str(decoded), "world");
}

#[test]
fn string_ref_pointing_to_different_substrings() {
    let source = "**bold** and _italic_";
    let arena = MdastArena::new(source.to_string());

    let bold_ref = StringRef::new(2, 4);
    let italic_ref = StringRef::new(14, 6);

    assert_eq!(arena.get_str(bold_ref), "bold");
    assert_eq!(arena.get_str(italic_ref), "italic");
}

#[test]
fn string_ref_is_copy() {
    let sr1 = StringRef::new(0, 10);
    let sr2 = sr1;
    assert_eq!(sr1, sr2);
}
