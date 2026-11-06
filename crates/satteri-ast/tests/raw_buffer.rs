use satteri_arena::{
    Arena, ArenaBuilder, ArenaKind, FORMAT_VERSION, Hast, Mdast, NODE_STRUCT_SIZE, NodePosition,
    OffsetUnit,
};
use satteri_ast::hast::codec::{decode_element_tag, encode_element_data};
use satteri_ast::hast::{HastNodeType, mdast_arena_to_hast_arena};
use satteri_ast::mdast::{MdastNodeType, encode_heading_data};
use satteri_ast::patch::{Patch, PatchContent, apply_patches_strict};

const MIXED_WIDTH_MDX: &str = "import Card from './card.js'\nexport const answer = 42\n\n# Héllo *wörld* 😀\n\nünïcödé € Café {answer + 1}\n\n<Card title=\"Café\" client:load count={2}>\n  Inside **the card**.\n</Card>\n\n- [ ] one\n- [x] twö\n\n[the guide](/en/) and `cöde`\n\n```js title=\"é.js\"\nconst x = '😀';\n```\n";

fn assert_restored<K: ArenaKind>(arena: &Arena<K>) {
    for unit in [OffsetUnit::Utf16, OffsetUnit::Byte] {
        let buf = arena.to_raw_buffer_in(unit);
        let read = Arena::<K>::from_raw_buffer(&buf).unwrap();
        assert_eq!(read.nodes, arena.nodes, "{unit:?}: nodes");
        assert_eq!(read.children, arena.children, "{unit:?}: children");
        assert_eq!(read.type_data, arena.type_data, "{unit:?}: type_data");
        assert_eq!(read.source_len, arena.source_len, "{unit:?}: source_len");
        assert_eq!(read.node_data, arena.node_data, "{unit:?}: node_data");
        assert_eq!(read.to_raw_buffer_in(unit), buf, "{unit:?}: re-serialized");
    }
}

fn parse_mixed_width_hast() -> Arena<Hast> {
    let (mdast, _) =
        satteri_pulldown_cmark::parse(MIXED_WIDTH_MDX, satteri_pulldown_cmark::MDX_OPTIONS);
    mdast_arena_to_hast_arena(&mdast)
}

#[test]
fn parsed_mdast_round_trips() {
    let (mdast, _) =
        satteri_pulldown_cmark::parse(MIXED_WIDTH_MDX, satteri_pulldown_cmark::MDX_OPTIONS);
    assert_restored(&mdast);
}

#[test]
fn converted_hast_round_trips() {
    assert_restored(&parse_mixed_width_hast());
}

#[test]
fn plugin_appended_elements_round_trip() {
    let mut hast = parse_mixed_width_hast();
    let mut label = ArenaBuilder::<Hast>::new(String::new());
    label.open_node(HastNodeType::Element as u8);
    let tag = label.alloc_string("label");
    label.set_data_current(&encode_element_data(tag, &[]));
    label.open_node(HastNodeType::Element as u8);
    let tag = label.alloc_string("span");
    label.set_data_current(&encode_element_data(tag, &[]));
    label.close_node();
    label.close_node();
    apply_patches_strict(
        &mut hast,
        &[Patch::AppendChild {
            node_id: 0,
            child_tree: PatchContent::Tree(label.finish()),
        }],
    )
    .unwrap();

    assert_restored(&hast);
    let read = Arena::<Hast>::from_raw_buffer(&hast.to_raw_buffer()).unwrap();
    let label_id = *read.get_children(0).last().unwrap();
    let span_id = read.get_children(label_id)[0];
    assert_eq!(
        read.get_str(decode_element_tag(read.get_type_data(label_id))),
        "label"
    );
    assert_eq!(
        read.get_str(decode_element_tag(read.get_type_data(span_id))),
        "span"
    );
}

fn build_test_arena() -> satteri_arena::Arena<Mdast> {
    let mut builder = ArenaBuilder::<Mdast>::new("# Hello\n\nParagraph.".to_string());

    builder.open_node(MdastNodeType::Root as u8);
    builder.set_position_current(NodePosition {
        start_offset: 0,
        end_offset: 20,
        start_line: 1,
        start_column: 1,
        end_line: 3,
        end_column: 11,
    });

    let heading = builder.open_node(MdastNodeType::Heading as u8);
    builder.set_position_current(NodePosition {
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
    builder.set_position_current(NodePosition {
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
fn header_format_version_has_a_fixed_offset() {
    let arena = build_test_arena();
    let buf = arena.to_raw_buffer();
    let version = u32::from_le_bytes(buf[8..12].try_into().unwrap());
    assert_eq!(version, FORMAT_VERSION);
}

#[test]
fn header_node_struct_size_correct() {
    let arena = build_test_arena();
    let buf = arena.to_raw_buffer();
    let nss = u32::from_ne_bytes(buf[16..20].try_into().unwrap());
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
