//! Integration tests for arena rebuild.
//!
//! Tests apply patches to the "# Hello\n\nWorld" arena and verify the resulting structure.

use satteri_arena::{Arena, ArenaBuilder, Hast, Mdast};
use satteri_ast::hast::HastNodeType;
use satteri_ast::mdast::MdastNodeType;
use satteri_ast::rebuild::{rebuild as rebuild_raw, Patch};

fn rebuild(arena: &Arena<Mdast>, patches: &[Patch<Mdast>]) -> Arena<Mdast> {
    rebuild_raw(arena, patches).expect("rebuild failed")
}

fn rebuild_hast(arena: &Arena<Hast>, patches: &[Patch<Hast>]) -> Arena<Hast> {
    rebuild_raw(arena, patches).expect("rebuild failed")
}

/// Tree structure:
///   Root (0)
///     Heading depth=1 (1)
///       Text "Hello" (2)
///     Paragraph (3)
///       Text "World" (4)
fn build_hello_world() -> Arena<Mdast> {
    use satteri_arena::StringRef;
    use satteri_ast::mdast::codec::{encode_heading_data, encode_string_ref_data};

    let source = "# Hello\n\nWorld".to_string();
    let mut b = ArenaBuilder::<Mdast>::new(source);

    b.open_node(MdastNodeType::Root as u8);
    b.set_position_current(0, 14, 1, 1, 2, 6);

    b.open_node(MdastNodeType::Heading as u8);
    b.set_position_current(0, 7, 1, 1, 1, 8);
    b.set_data_current(&encode_heading_data(1));

    b.open_node(MdastNodeType::Text as u8);
    b.set_position_current(2, 7, 1, 3, 1, 8);
    b.set_data_current(&encode_string_ref_data(StringRef::new(2, 5)));
    b.close_node();

    b.close_node(); // heading

    b.open_node(MdastNodeType::Paragraph as u8);
    b.set_position_current(9, 14, 2, 1, 2, 6);

    b.open_node(MdastNodeType::Text as u8);
    b.set_position_current(9, 14, 2, 1, 2, 6);
    b.set_data_current(&encode_string_ref_data(StringRef::new(9, 5)));
    b.close_node();

    b.close_node(); // paragraph
    b.close_node(); // root

    b.finish()
}

fn single_node_arena(node_type: MdastNodeType) -> Arena<Mdast> {
    let mut b = ArenaBuilder::<Mdast>::new(String::new());
    b.open_node(node_type as u8);
    b.close_node();
    b.finish()
}

/// A `Root`-wrapped arena, mimicking what the parser produces for a raw
/// markdown / HTML payload: `Root > [block, ...]`.
fn root_wrapped_arena(blocks: &[MdastNodeType]) -> Arena<Mdast> {
    let mut b = ArenaBuilder::<Mdast>::new(String::new());
    b.open_node(MdastNodeType::Root as u8);
    for &block in blocks {
        b.open_node(block as u8);
        b.close_node();
    }
    b.close_node();
    b.finish()
}

/// Empty patches → same structure (all nodes preserved, just fresh IDs).
#[test]
fn empty_patches_preserves_all_nodes() {
    let orig = build_hello_world();
    let rebuilt = rebuild(&orig, &[]);

    assert_eq!(rebuilt.len(), orig.len(), "node count unchanged");

    assert_eq!(rebuilt.get_children(0).len(), 2);
    let h = rebuilt.get_children(0)[0];
    let p = rebuilt.get_children(0)[1];
    assert_eq!(rebuilt.get_node(h).node_type, MdastNodeType::Heading as u8);
    assert_eq!(
        rebuilt.get_node(p).node_type,
        MdastNodeType::Paragraph as u8
    );
    assert_eq!(rebuilt.get_children(h).len(), 1);
    assert_eq!(rebuilt.get_children(p).len(), 1);
}

#[test]
fn remove_leaf_node() {
    let orig = build_hello_world();
    let heading_id = orig.get_children(0)[0];
    let text_in_heading = orig.get_children(heading_id)[0];

    let rebuilt = rebuild(
        &orig,
        &[Patch::Remove {
            node_id: text_in_heading,
        }],
    );

    assert_eq!(rebuilt.len(), 4, "one node removed");
    let new_h = rebuilt.get_children(0)[0];
    assert_eq!(
        rebuilt.get_node(new_h).node_type,
        MdastNodeType::Heading as u8
    );
    assert_eq!(
        rebuilt.get_children(new_h).len(),
        0,
        "heading has no children now"
    );

    // Paragraph + its Text are still present
    let new_p = rebuilt.get_children(0)[1];
    assert_eq!(
        rebuilt.get_node(new_p).node_type,
        MdastNodeType::Paragraph as u8
    );
    assert_eq!(rebuilt.get_children(new_p).len(), 1);
}

#[test]
fn remove_non_leaf_removes_subtree() {
    let orig = build_hello_world();
    let heading_id = orig.get_children(0)[0];

    let rebuilt = rebuild(
        &orig,
        &[Patch::Remove {
            node_id: heading_id,
        }],
    );

    assert_eq!(rebuilt.len(), 3, "heading + its text child removed");
    let root_children = rebuilt.get_children(0);
    assert_eq!(root_children.len(), 1);
    assert_eq!(
        rebuilt.get_node(root_children[0]).node_type,
        MdastNodeType::Paragraph as u8
    );
}

#[test]
fn replace_leaf_node() {
    let orig = build_hello_world();
    let heading_id = orig.get_children(0)[0];
    let text_id = orig.get_children(heading_id)[0];

    let replacement = single_node_arena(MdastNodeType::ThematicBreak);
    let rebuilt = rebuild(
        &orig,
        &[Patch::Replace {
            node_id: text_id,
            new_tree: replacement,
            keep_children: false,
        }],
    );

    assert_eq!(
        rebuilt.len(),
        orig.len(),
        "same node count (1-for-1 replacement)"
    );
    let new_h = rebuilt.get_children(0)[0];
    let child_of_h = rebuilt.get_children(new_h)[0];
    assert_eq!(
        rebuilt.get_node(child_of_h).node_type,
        MdastNodeType::ThematicBreak as u8
    );
}

#[test]
fn replace_root_child_with_different_type() {
    let orig = build_hello_world();
    let heading_id = orig.get_children(0)[0];

    let replacement = single_node_arena(MdastNodeType::Paragraph);
    let rebuilt = rebuild(
        &orig,
        &[Patch::Replace {
            node_id: heading_id,
            new_tree: replacement,
            keep_children: false,
        }],
    );

    // The heading + its text child (2 nodes) are replaced by 1 Paragraph
    // So: Root + new Paragraph + original Paragraph + Text(World) = 4 nodes
    assert_eq!(rebuilt.len(), 4);
    let root_children = rebuilt.get_children(0);
    assert_eq!(root_children.len(), 2);
    assert_eq!(
        rebuilt.get_node(root_children[0]).node_type,
        MdastNodeType::Paragraph as u8
    );
    assert_eq!(
        rebuilt.get_node(root_children[1]).node_type,
        MdastNodeType::Paragraph as u8
    );
}

#[test]
fn insert_before_node() {
    let orig = build_hello_world();
    let para_id = orig.get_children(0)[1];

    let new_tree = single_node_arena(MdastNodeType::ThematicBreak);
    let rebuilt = rebuild(
        &orig,
        &[Patch::InsertBefore {
            node_id: para_id,
            new_tree,
        }],
    );

    let root_children = rebuilt.get_children(0);
    assert_eq!(root_children.len(), 3);
    assert_eq!(
        rebuilt.get_node(root_children[0]).node_type,
        MdastNodeType::Heading as u8
    );
    assert_eq!(
        rebuilt.get_node(root_children[1]).node_type,
        MdastNodeType::ThematicBreak as u8
    );
    assert_eq!(
        rebuilt.get_node(root_children[2]).node_type,
        MdastNodeType::Paragraph as u8
    );
}

#[test]
fn insert_after_node() {
    let orig = build_hello_world();
    let heading_id = orig.get_children(0)[0];

    let new_tree = single_node_arena(MdastNodeType::ThematicBreak);
    let rebuilt = rebuild(
        &orig,
        &[Patch::InsertAfter {
            node_id: heading_id,
            new_tree,
        }],
    );

    let root_children = rebuilt.get_children(0);
    assert_eq!(root_children.len(), 3);
    assert_eq!(
        rebuilt.get_node(root_children[0]).node_type,
        MdastNodeType::Heading as u8
    );
    assert_eq!(
        rebuilt.get_node(root_children[1]).node_type,
        MdastNodeType::ThematicBreak as u8
    );
    assert_eq!(
        rebuilt.get_node(root_children[2]).node_type,
        MdastNodeType::Paragraph as u8
    );
}

#[test]
fn append_child() {
    let orig = build_hello_world();
    let heading_id = orig.get_children(0)[0];

    let child_tree = single_node_arena(MdastNodeType::Break);
    let rebuilt = rebuild(
        &orig,
        &[Patch::AppendChild {
            node_id: heading_id,
            child_tree,
        }],
    );

    let new_h = rebuilt.get_children(0)[0];
    let h_children = rebuilt.get_children(new_h);
    assert_eq!(h_children.len(), 2);
    assert_eq!(
        rebuilt.get_node(h_children[0]).node_type,
        MdastNodeType::Text as u8
    );
    assert_eq!(
        rebuilt.get_node(h_children[1]).node_type,
        MdastNodeType::Break as u8
    );
}

#[test]
fn prepend_child() {
    let orig = build_hello_world();
    let heading_id = orig.get_children(0)[0];

    let child_tree = single_node_arena(MdastNodeType::Break);
    let rebuilt = rebuild(
        &orig,
        &[Patch::PrependChild {
            node_id: heading_id,
            child_tree,
        }],
    );

    let new_h = rebuilt.get_children(0)[0];
    let h_children = rebuilt.get_children(new_h);
    assert_eq!(h_children.len(), 2);
    assert_eq!(
        rebuilt.get_node(h_children[0]).node_type,
        MdastNodeType::Break as u8
    );
    assert_eq!(
        rebuilt.get_node(h_children[1]).node_type,
        MdastNodeType::Text as u8
    );
}

#[test]
fn multiple_patches_applied_together() {
    let orig = build_hello_world();
    let heading_id = orig.get_children(0)[0];
    let para_id = orig.get_children(0)[1];

    let new_tree = single_node_arena(MdastNodeType::ThematicBreak);

    let patches = vec![
        Patch::Remove {
            node_id: heading_id,
        },
        Patch::InsertAfter {
            node_id: para_id,
            new_tree,
        },
    ];
    let rebuilt = rebuild(&orig, &patches);

    let root_children = rebuilt.get_children(0);
    assert_eq!(root_children.len(), 2);
    assert_eq!(
        rebuilt.get_node(root_children[0]).node_type,
        MdastNodeType::Paragraph as u8
    );
    assert_eq!(
        rebuilt.get_node(root_children[1]).node_type,
        MdastNodeType::ThematicBreak as u8
    );

    // Total: Root + Paragraph + Text(World) + ThematicBreak = 4 nodes
    assert_eq!(rebuilt.len(), 4);
}

/// Replacement subtree containing a directive child must have the directive's
/// `name` (and any attribute keys/values) remapped onto the merged source.
/// Without remapping, the directive's StringRef stays at the sub-arena's local
/// offset, which collides with the original-source bytes — and if those bytes
/// are inside a multi-byte codepoint, any later read panics with "byte index N
/// is not a char boundary". Reproduces the asides + directives-restoration
/// crash on Hindi MDX (see satteri-arena-panic.md).
#[test]
fn replacement_with_directive_child_remaps_string_refs() {
    use satteri_arena::StringRef;
    use satteri_ast::mdast::codec::{encode_directive_data, encode_string_ref_data};

    // Multi-byte UTF-8 prefix so that an un-remapped sub-arena offset that
    // lands inside this region would split a codepoint when sliced.
    let pad = "अवयव अवयव अवयव अवयव"; // 36 bytes of Devanagari + 4 ASCII spaces
    let source = format!("{pad}\n");
    let pad_len = pad.len() as u32;

    let mut b = ArenaBuilder::<Mdast>::new(source);
    b.open_node(MdastNodeType::Root as u8);
    b.open_node(MdastNodeType::Paragraph as u8);
    b.open_node(MdastNodeType::Text as u8);
    b.set_data_current(&encode_string_ref_data(StringRef::new(0, pad_len)));
    b.close_node();
    b.close_node();
    b.close_node();
    let orig = b.finish();

    let para_id = orig.get_children(0)[0];

    // Replacement: a paragraph whose only child is a textDirective named
    // "inline" with an attribute pair. The sub-arena's source starts empty,
    // so each `alloc_string` produces offsets 0, 6, … which — if not remapped
    // — would alias the multi-byte prefix of the merged source.
    let mut sub = ArenaBuilder::<Mdast>::new(String::new());
    sub.open_node(MdastNodeType::Paragraph as u8);
    sub.open_node(MdastNodeType::TextDirective as u8);
    let name_ref = sub.alloc_string("inline");
    let key_ref = sub.alloc_string("class");
    let val_ref = sub.alloc_string("note");
    sub.set_data_current(&encode_directive_data(name_ref, &[(key_ref, val_ref)]));
    sub.close_node();
    sub.close_node();
    let replacement = sub.finish();

    let rebuilt = rebuild(
        &orig,
        &[Patch::Replace {
            node_id: para_id,
            new_tree: replacement,
            keep_children: false,
        }],
    );

    let dir_id = (0..rebuilt.len() as u32)
        .find(|&id| rebuilt.get_node(id).node_type == MdastNodeType::TextDirective as u8)
        .expect("textDirective should be present");

    let dir_data = rebuilt.get_type_data(dir_id);
    let name_sr = StringRef::from_bytes(&dir_data[..8]);
    let key_sr = StringRef::from_bytes(&dir_data[16..24]);
    let val_sr = StringRef::from_bytes(&dir_data[24..32]);

    assert!(
        name_sr.offset >= pad_len && key_sr.offset >= pad_len && val_sr.offset >= pad_len,
        "directive StringRef offsets must be remapped past the original source",
    );
    assert_eq!(rebuilt.get_str(name_sr), "inline");
    assert_eq!(rebuilt.get_str(key_sr), "class");
    assert_eq!(rebuilt.get_str(val_sr), "note");
}

/// Regression: when a replacement subtree is emitted at a non-zero source_base
/// (because the sub-arena had its own source appended to the merged buffer),
/// MDAST `List` type_data must NOT be remapped as if its first 8 bytes were a
/// StringRef. The bytes at offset 0..4 are `start: u32`; treating them as a
/// StringRef offset corrupts ordered lists (start=1 → start=1+base), which
/// then surfaces as a spurious `start="N"` attribute on the rendered `<ol>`.
///
/// Numeric collision: MDAST `List` = 5 = HAST `Raw`.
#[test]
fn list_start_survives_source_base_remap() {
    use satteri_ast::mdast::codec::encode_list_data;

    // Build a tiny tree with a paragraph we'll replace with a subtree that
    // wraps the original list. The replacement carries its own source so the
    // builder appends it, producing a non-zero source_base for everything
    // emitted from the sub-arena.
    let mut b = ArenaBuilder::<Mdast>::new("placeholder".to_string());
    b.open_node(MdastNodeType::Root as u8);
    b.open_node(MdastNodeType::Paragraph as u8);
    b.close_node();
    b.close_node();
    let orig = b.finish();
    let para_id = orig.get_children(0)[0];

    // Replacement: a Paragraph wrapping a List(start=1, ordered) wrapping a
    // ListItem. Sub-arena has its own non-empty source, so `emit_subtree`
    // appends it and assigns a non-zero source_base for the descendants.
    let mut sub = ArenaBuilder::<Mdast>::new("xxxxxxxxxxxxxxxx".to_string());
    sub.open_node(MdastNodeType::Paragraph as u8);
    sub.open_node(MdastNodeType::List as u8);
    sub.set_data_current(&encode_list_data(true, 1, false));
    sub.open_node(MdastNodeType::ListItem as u8);
    sub.close_node();
    sub.close_node();
    sub.close_node();
    let replacement = sub.finish();

    let rebuilt = rebuild(
        &orig,
        &[Patch::Replace {
            node_id: para_id,
            new_tree: replacement,
            keep_children: false,
        }],
    );

    let list_id = (0..rebuilt.len() as u32)
        .find(|&id| rebuilt.get_node(id).node_type == MdastNodeType::List as u8)
        .expect("List node must be present");
    let data = rebuilt.get_type_data(list_id);
    let start = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let ordered = data[4] != 0;

    assert_eq!(
        start, 1,
        "ordered list start must round-trip as 1, not be polluted by source_base ({} bytes appended)",
        rebuilt.string_pool().len()
    );
    assert!(ordered, "ordered flag must survive remap");
}

#[test]
fn parent_references_consistent_after_rebuild() {
    let orig = build_hello_world();
    let para_id = orig.get_children(0)[1];

    let new_tree = single_node_arena(MdastNodeType::ThematicBreak);
    let rebuilt = rebuild(
        &orig,
        &[Patch::InsertAfter {
            node_id: para_id,
            new_tree,
        }],
    );

    let root = 0u32;
    for child_id in rebuilt.get_children(root) {
        let child = rebuilt.get_node(*child_id);
        assert_eq!(
            child.parent, root,
            "child of root should have root as parent"
        );

        for grandchild_id in rebuilt.get_children(*child_id) {
            let gc = rebuilt.get_node(*grandchild_id);
            assert_eq!(gc.parent, *child_id, "grandchild parent mismatch");
        }
    }
}

// ----- StringRef remap coverage --------------------------------------------
//
// These tests Replace a node with a sub-arena that carries its own non-empty
// source, which forces the rebuild path to allocate a non-zero `source_base`
// for the merged buffer. Any encoded StringRef in the replacement's
// `type_data` must be remapped from sub-arena offsets to merged-arena offsets
// or the round-tripped strings come back as garbage.

/// Build `root > paragraph` MDAST and Replace the paragraph with a single
/// node whose type_data is produced by `build_replacement`. The sub-arena's
/// own `"xxxxxxxxxxxxxxxx"` source guarantees a non-zero `source_base`.
fn replace_para_with<F>(node_type: MdastNodeType, build_replacement: F) -> Arena<Mdast>
where
    F: FnOnce(&mut ArenaBuilder<Mdast>),
{
    let mut b = ArenaBuilder::<Mdast>::new("placeholder".to_string());
    b.open_node(MdastNodeType::Root as u8);
    b.open_node(MdastNodeType::Paragraph as u8);
    b.close_node();
    b.close_node();
    let orig = b.finish();
    let para_id = orig.get_children(0)[0];

    let mut sub = ArenaBuilder::<Mdast>::new("xxxxxxxxxxxxxxxx".to_string());
    sub.open_node(node_type as u8);
    build_replacement(&mut sub);
    sub.close_node();
    let replacement = sub.finish();

    rebuild(
        &orig,
        &[Patch::Replace {
            node_id: para_id,
            new_tree: replacement,
            keep_children: false,
        }],
    )
}

fn first_node_of(arena: &Arena<Mdast>, node_type: MdastNodeType) -> u32 {
    (0..arena.len() as u32)
        .find(|&id| arena.get_node(id).node_type == node_type as u8)
        .unwrap_or_else(|| panic!("no {:?} node in rebuilt arena", node_type))
}

fn first_hast_node_of(arena: &Arena<Hast>, node_type: HastNodeType) -> u32 {
    (0..arena.len() as u32)
        .find(|&id| arena.get_node(id).node_type == node_type as u8)
        .unwrap_or_else(|| panic!("no {:?} node in rebuilt arena", node_type))
}

#[test]
fn code_string_refs_round_trip() {
    use satteri_ast::mdast::codec::{decode_code_data, encode_code_data};

    let rebuilt = replace_para_with(MdastNodeType::Code, |sub| {
        let lang = sub.alloc_string("rust");
        let meta = sub.alloc_string("title=\"x\"");
        let value = sub.alloc_string("fn main() {}");
        sub.set_data_current(&encode_code_data(lang, meta, value, b'`'));
    });

    let id = first_node_of(&rebuilt, MdastNodeType::Code);
    let cd = decode_code_data(rebuilt.get_type_data(id));
    assert_eq!(rebuilt.get_str(cd.lang), "rust");
    assert_eq!(rebuilt.get_str(cd.meta), "title=\"x\"");
    assert_eq!(rebuilt.get_str(cd.value), "fn main() {}");
}

#[test]
fn link_string_refs_round_trip() {
    use satteri_ast::mdast::codec::{decode_link_data, encode_link_data};

    let rebuilt = replace_para_with(MdastNodeType::Link, |sub| {
        let url = sub.alloc_string("https://example.com");
        let title = sub.alloc_string("Example");
        sub.set_data_current(&encode_link_data(url, title));
    });

    let id = first_node_of(&rebuilt, MdastNodeType::Link);
    let ld = decode_link_data(rebuilt.get_type_data(id));
    assert_eq!(rebuilt.get_str(ld.url), "https://example.com");
    assert_eq!(rebuilt.get_str(ld.title), "Example");
}

#[test]
fn image_string_refs_round_trip() {
    use satteri_ast::mdast::codec::{decode_image_data, encode_image_data};

    let rebuilt = replace_para_with(MdastNodeType::Image, |sub| {
        let url = sub.alloc_string("/img.png");
        let alt = sub.alloc_string("alt text");
        let title = sub.alloc_string("hover");
        sub.set_data_current(&encode_image_data(url, alt, title));
    });

    let id = first_node_of(&rebuilt, MdastNodeType::Image);
    let im = decode_image_data(rebuilt.get_type_data(id));
    assert_eq!(rebuilt.get_str(im.url), "/img.png");
    assert_eq!(rebuilt.get_str(im.alt), "alt text");
    assert_eq!(rebuilt.get_str(im.title), "hover");
}

#[test]
fn definition_string_refs_round_trip() {
    use satteri_ast::mdast::codec::{decode_definition_data, encode_definition_data};

    let rebuilt = replace_para_with(MdastNodeType::Definition, |sub| {
        let url = sub.alloc_string("https://example.com");
        let title = sub.alloc_string("title");
        let identifier = sub.alloc_string("id");
        let label = sub.alloc_string("Label");
        sub.set_data_current(&encode_definition_data(url, title, identifier, label));
    });

    let id = first_node_of(&rebuilt, MdastNodeType::Definition);
    let d = decode_definition_data(rebuilt.get_type_data(id));
    assert_eq!(rebuilt.get_str(d.url), "https://example.com");
    assert_eq!(rebuilt.get_str(d.title), "title");
    assert_eq!(rebuilt.get_str(d.identifier), "id");
    assert_eq!(rebuilt.get_str(d.label), "Label");
}

#[test]
fn footnote_definition_string_refs_round_trip() {
    use satteri_ast::mdast::codec::{
        decode_footnote_definition_data, encode_footnote_definition_data,
    };

    let rebuilt = replace_para_with(MdastNodeType::FootnoteDefinition, |sub| {
        let identifier = sub.alloc_string("note-1");
        let label = sub.alloc_string("Note 1");
        sub.set_data_current(&encode_footnote_definition_data(identifier, label));
    });

    let id = first_node_of(&rebuilt, MdastNodeType::FootnoteDefinition);
    let d = decode_footnote_definition_data(rebuilt.get_type_data(id));
    assert_eq!(rebuilt.get_str(d.identifier), "note-1");
    assert_eq!(rebuilt.get_str(d.label), "Note 1");
}

#[test]
fn link_reference_string_refs_round_trip() {
    use satteri_ast::mdast::codec::{decode_reference_data, encode_reference_data};

    let rebuilt = replace_para_with(MdastNodeType::LinkReference, |sub| {
        let identifier = sub.alloc_string("ref-id");
        let label = sub.alloc_string("Ref Label");
        sub.set_data_current(&encode_reference_data(identifier, label, 0));
    });

    let id = first_node_of(&rebuilt, MdastNodeType::LinkReference);
    let r = decode_reference_data(rebuilt.get_type_data(id));
    assert_eq!(rebuilt.get_str(r.identifier), "ref-id");
    assert_eq!(rebuilt.get_str(r.label), "Ref Label");
}

#[test]
fn image_reference_string_refs_round_trip() {
    use satteri_ast::mdast::codec::{
        decode_image_reference_alt, decode_reference_data, encode_image_reference_data,
    };

    let rebuilt = replace_para_with(MdastNodeType::ImageReference, |sub| {
        let identifier = sub.alloc_string("img-id");
        let label = sub.alloc_string("Img Label");
        let alt = sub.alloc_string("alt text");
        sub.set_data_current(&encode_image_reference_data(identifier, label, 0, alt));
    });

    let id = first_node_of(&rebuilt, MdastNodeType::ImageReference);
    let data = rebuilt.get_type_data(id);
    let r = decode_reference_data(data);
    let alt = decode_image_reference_alt(data);
    assert_eq!(rebuilt.get_str(r.identifier), "img-id");
    assert_eq!(rebuilt.get_str(r.label), "Img Label");
    assert_eq!(rebuilt.get_str(alt), "alt text");
}

#[test]
fn math_string_refs_round_trip() {
    use satteri_ast::mdast::codec::{decode_math_data, encode_math_data};

    let rebuilt = replace_para_with(MdastNodeType::Math, |sub| {
        let meta = sub.alloc_string("display");
        let value = sub.alloc_string("a^2 + b^2");
        sub.set_data_current(&encode_math_data(meta, value));
    });

    let id = first_node_of(&rebuilt, MdastNodeType::Math);
    let m = decode_math_data(rebuilt.get_type_data(id));
    assert_eq!(rebuilt.get_str(m.meta), "display");
    assert_eq!(rebuilt.get_str(m.value), "a^2 + b^2");
}

#[test]
fn expression_string_refs_round_trip() {
    use satteri_ast::mdast::codec::{decode_expression_data, encode_expression_data};

    let rebuilt = replace_para_with(MdastNodeType::MdxFlowExpression, |sub| {
        let value = sub.alloc_string("count + 1");
        sub.set_data_current(&encode_expression_data(value));
    });

    let id = first_node_of(&rebuilt, MdastNodeType::MdxFlowExpression);
    let e = decode_expression_data(rebuilt.get_type_data(id));
    assert_eq!(rebuilt.get_str(e.value), "count + 1");
}

#[test]
fn mdx_jsx_element_string_refs_round_trip() {
    use satteri_ast::mdast::codec::{
        decode_mdx_jsx_attr, decode_mdx_jsx_attr_count, decode_mdx_jsx_element_name,
        encode_mdx_jsx_element_data,
    };

    let rebuilt = replace_para_with(MdastNodeType::MdxJsxFlowElement, |sub| {
        let name = sub.alloc_string("Button");
        let a1_name = sub.alloc_string("variant");
        let a1_value = sub.alloc_string("primary");
        let a2_name = sub.alloc_string("disabled");
        let a2_value = sub.alloc_string("");
        sub.set_data_current(&encode_mdx_jsx_element_data(
            name,
            &[(1, a1_name, a1_value), (0, a2_name, a2_value)],
            true,
        ));
    });

    let id = first_node_of(&rebuilt, MdastNodeType::MdxJsxFlowElement);
    let data = rebuilt.get_type_data(id);
    assert_eq!(rebuilt.get_str(decode_mdx_jsx_element_name(data)), "Button");
    assert_eq!(decode_mdx_jsx_attr_count(data), 2);
    let (kind1, n1, v1) = decode_mdx_jsx_attr(data, 0);
    assert_eq!(kind1, 1);
    assert_eq!(rebuilt.get_str(n1), "variant");
    assert_eq!(rebuilt.get_str(v1), "primary");
    let (_, n2, _) = decode_mdx_jsx_attr(data, 1);
    assert_eq!(rebuilt.get_str(n2), "disabled");
}

/// `Replace { keep_children: true }` must remap StringRefs in the wrapper's
/// own type_data while leaving the kept children's data intact. Regression
/// guard for any future bug where the wrapper's source_base offset gets
/// applied (or not applied) only to the wrapper or only to the children.
#[test]
fn replace_keep_children_remaps_wrapper_string_refs() {
    use satteri_arena::StringRef;
    use satteri_ast::mdast::codec::{
        decode_link_data, decode_string_ref_data, encode_link_data, encode_string_ref_data,
    };

    // Original: root > paragraph > text("hello") at offset 0.
    let mut b = ArenaBuilder::<Mdast>::new("hello world".to_string());
    b.open_node(MdastNodeType::Root as u8);
    b.open_node(MdastNodeType::Paragraph as u8);
    b.open_node(MdastNodeType::Text as u8);
    b.set_data_current(&encode_string_ref_data(StringRef::new(0, 5)));
    b.close_node();
    b.close_node();
    b.close_node();
    let orig = b.finish();
    let para_id = orig.get_children(0)[0];

    // Replacement: a Link wrapper carrying its own (sub-arena-local) URL
    // and title. With keep_children: true, the original Text("hello") child
    // must be retained, and the Link's StringRefs must be remapped past the
    // sub-arena's source_base.
    let mut sub = ArenaBuilder::<Mdast>::new("padding-padding-".to_string());
    sub.open_node(MdastNodeType::Link as u8);
    let url = sub.alloc_string("https://example.com");
    let title = sub.alloc_string("Example");
    sub.set_data_current(&encode_link_data(url, title));
    sub.close_node();
    let replacement = sub.finish();

    let rebuilt = rebuild(
        &orig,
        &[Patch::Replace {
            node_id: para_id,
            new_tree: replacement,
            keep_children: true,
        }],
    );

    // Wrapper Link with original Text child preserved.
    let link_id = first_node_of(&rebuilt, MdastNodeType::Link);
    let link_children = rebuilt.get_children(link_id);
    assert_eq!(link_children.len(), 1, "kept-children should be retained");
    let text_id = link_children[0];
    assert_eq!(
        rebuilt.get_node(text_id).node_type,
        MdastNodeType::Text as u8
    );
    let text_sr = decode_string_ref_data(rebuilt.get_type_data(text_id));
    assert_eq!(rebuilt.get_str(text_sr), "hello");

    let ld = decode_link_data(rebuilt.get_type_data(link_id));
    assert_eq!(rebuilt.get_str(ld.url), "https://example.com");
    assert_eq!(rebuilt.get_str(ld.title), "Example");
}

#[test]
fn hast_element_with_properties_round_trip() {
    use satteri_arena::StringRef;
    use satteri_ast::hast::codec::{
        decode_element_prop, decode_element_prop_count, decode_element_tag, encode_element_data,
        encode_text_data,
    };
    use satteri_ast::shared::PROP_STRING;

    // Original HAST: root > text("seed"). We replace the text with an
    // element so the new element's tag + props all need remap.
    let mut b = ArenaBuilder::<Hast>::new("seed".to_string());
    b.open_node_raw(HastNodeType::Root as u8);
    b.open_node_raw(HastNodeType::Text as u8);
    b.set_data_current(&encode_text_data(StringRef::new(0, 4)));
    b.close_node();
    b.close_node();
    let orig = b.finish();
    let text_id = orig.get_children(0)[0];

    let mut sub = ArenaBuilder::<Hast>::new("padding-padding-".to_string());
    sub.open_node_raw(HastNodeType::Element as u8);
    let tag = sub.alloc_string("a");
    let href_name = sub.alloc_string("href");
    let href_value = sub.alloc_string("https://example.com");
    let class_name = sub.alloc_string("class");
    let class_value = sub.alloc_string("link primary");
    sub.set_data_current(&encode_element_data(
        tag,
        &[
            (href_name, PROP_STRING, href_value),
            (class_name, PROP_STRING, class_value),
        ],
    ));
    sub.close_node();
    let replacement = sub.finish();

    let rebuilt = rebuild_hast(
        &orig,
        &[Patch::Replace {
            node_id: text_id,
            new_tree: replacement,
            keep_children: false,
        }],
    );

    let elem_id = first_hast_node_of(&rebuilt, HastNodeType::Element);
    let data = rebuilt.get_type_data(elem_id);
    assert_eq!(rebuilt.get_str(decode_element_tag(data)), "a");
    assert_eq!(decode_element_prop_count(data), 2);
    let (n0, t0, v0) = decode_element_prop(data, 0);
    assert_eq!(rebuilt.get_str(n0), "href");
    assert_eq!(t0, PROP_STRING);
    assert_eq!(rebuilt.get_str(v0), "https://example.com");
    let (n1, _, v1) = decode_element_prop(data, 1);
    assert_eq!(rebuilt.get_str(n1), "class");
    assert_eq!(rebuilt.get_str(v1), "link primary");
}

#[test]
fn position_preserved_after_remove_sibling() {
    let orig = build_hello_world();
    let heading_id = orig.get_children(0)[0];
    let para_id = orig.get_children(0)[1];
    let text_in_para = orig.get_children(para_id)[0];

    let orig_para = *orig.get_node(para_id);
    let orig_text = *orig.get_node(text_in_para);

    let rebuilt = rebuild(
        &orig,
        &[Patch::Remove {
            node_id: heading_id,
        }],
    );

    let new_para = rebuilt.get_children(0)[0];
    let new_text = rebuilt.get_children(new_para)[0];
    let np = rebuilt.get_node(new_para);
    let nt = rebuilt.get_node(new_text);

    assert_eq!(np.start_offset, orig_para.start_offset);
    assert_eq!(np.end_offset, orig_para.end_offset);
    assert_eq!(np.start_line, orig_para.start_line);
    assert_eq!(np.start_column, orig_para.start_column);
    assert_eq!(np.end_line, orig_para.end_line);
    assert_eq!(np.end_column, orig_para.end_column);

    assert_eq!(nt.start_offset, orig_text.start_offset);
    assert_eq!(nt.end_offset, orig_text.end_offset);
    assert_eq!(nt.start_line, orig_text.start_line);
    assert_eq!(nt.start_column, orig_text.start_column);
    assert_eq!(nt.end_line, orig_text.end_line);
    assert_eq!(nt.end_column, orig_text.end_column);
}

#[test]
fn append_child_with_string_ref_type_data_round_trip() {
    use satteri_ast::mdast::codec::{decode_link_data, encode_link_data};

    let mut b = ArenaBuilder::<Mdast>::new("placeholder".to_string());
    b.open_node(MdastNodeType::Root as u8);
    b.open_node(MdastNodeType::Paragraph as u8);
    b.close_node();
    b.close_node();
    let orig = b.finish();
    let para_id = orig.get_children(0)[0];

    let mut sub = ArenaBuilder::<Mdast>::new("padding-padding-".to_string());
    sub.open_node(MdastNodeType::Link as u8);
    let url = sub.alloc_string("https://example.com");
    let title = sub.alloc_string("Example");
    sub.set_data_current(&encode_link_data(url, title));
    sub.close_node();
    let child_tree = sub.finish();

    let rebuilt = rebuild(
        &orig,
        &[Patch::AppendChild {
            node_id: para_id,
            child_tree,
        }],
    );

    let id = first_node_of(&rebuilt, MdastNodeType::Link);
    let ld = decode_link_data(rebuilt.get_type_data(id));
    assert_eq!(rebuilt.get_str(ld.url), "https://example.com");
    assert_eq!(rebuilt.get_str(ld.title), "Example");
}

#[test]
fn insert_after_with_string_ref_type_data_round_trip() {
    use satteri_ast::mdast::codec::{decode_image_data, encode_image_data};

    let mut b = ArenaBuilder::<Mdast>::new("placeholder".to_string());
    b.open_node(MdastNodeType::Root as u8);
    b.open_node(MdastNodeType::Paragraph as u8);
    b.close_node();
    b.close_node();
    let orig = b.finish();
    let para_id = orig.get_children(0)[0];

    let mut sub = ArenaBuilder::<Mdast>::new("padding-padding-".to_string());
    sub.open_node(MdastNodeType::Image as u8);
    let url = sub.alloc_string("/img.png");
    let alt = sub.alloc_string("alt text");
    let title = sub.alloc_string("hover");
    sub.set_data_current(&encode_image_data(url, alt, title));
    sub.close_node();
    let new_tree = sub.finish();

    let rebuilt = rebuild(
        &orig,
        &[Patch::InsertAfter {
            node_id: para_id,
            new_tree,
        }],
    );

    let id = first_node_of(&rebuilt, MdastNodeType::Image);
    let im = decode_image_data(rebuilt.get_type_data(id));
    assert_eq!(rebuilt.get_str(im.url), "/img.png");
    assert_eq!(rebuilt.get_str(im.alt), "alt text");
    assert_eq!(rebuilt.get_str(im.title), "hover");
}

#[test]
fn node_data_preserved_through_keep_children_replace() {
    use satteri_arena::StringRef;
    use satteri_ast::mdast::codec::encode_string_ref_data;

    let mut b = ArenaBuilder::<Mdast>::new("hello".to_string());
    b.open_node(MdastNodeType::Root as u8);
    b.open_node(MdastNodeType::Paragraph as u8);
    b.open_node(MdastNodeType::Text as u8);
    b.set_data_current(&encode_string_ref_data(StringRef::new(0, 5)));
    b.close_node();
    b.close_node();
    b.close_node();
    let mut orig = b.finish();

    let para_id = orig.get_children(0)[0];
    let text_id = orig.get_children(para_id)[0];
    orig.set_node_data(text_id, br#"{"hName":"em"}"#.to_vec());
    orig.set_node_data(para_id, br#"{"className":"intro"}"#.to_vec());

    // Replace paragraph with strong wrapper, keep children. Both the
    // wrapper's own node_data and the kept text's node_data must survive.
    let mut sub = ArenaBuilder::<Mdast>::new(String::new());
    sub.open_node(MdastNodeType::Strong as u8);
    sub.close_node();
    let mut replacement = sub.finish();
    replacement.set_node_data(0, br#"{"hName":"b"}"#.to_vec());

    let rebuilt = rebuild(
        &orig,
        &[Patch::Replace {
            node_id: para_id,
            new_tree: replacement,
            keep_children: true,
        }],
    );

    let new_strong = rebuilt.get_children(0)[0];
    let new_text = rebuilt.get_children(new_strong)[0];

    assert_eq!(
        rebuilt.get_node_data(new_strong),
        Some(br#"{"hName":"b"}"#.as_slice()),
        "wrapper node_data must come from the replacement subtree",
    );
    assert_eq!(
        rebuilt.get_node_data(new_text),
        Some(br#"{"hName":"em"}"#.as_slice()),
        "kept child's node_data must survive Replace {{ keep_children: true }}",
    );
}

#[test]
fn hast_text_round_trip_with_source_base() {
    use satteri_ast::hast::codec::{decode_text_data, encode_text_data};

    // Original HAST: root > element. We replace the element with a Text
    // node whose StringRef must be remapped from the sub-arena's source.
    let mut b = ArenaBuilder::<Hast>::new("seed".to_string());
    b.open_node_raw(HastNodeType::Root as u8);
    b.open_node_raw(HastNodeType::Element as u8);
    b.close_node();
    b.close_node();
    let orig = b.finish();
    let elem_id = orig.get_children(0)[0];

    let mut sub = ArenaBuilder::<Hast>::new("padding-padding-".to_string());
    sub.open_node_raw(HastNodeType::Text as u8);
    let value = sub.alloc_string("Hello, world!");
    sub.set_data_current(&encode_text_data(value));
    sub.close_node();
    let replacement = sub.finish();

    let rebuilt = rebuild_hast(
        &orig,
        &[Patch::Replace {
            node_id: elem_id,
            new_tree: replacement,
            keep_children: false,
        }],
    );

    let text_id = first_hast_node_of(&rebuilt, HastNodeType::Text);
    let sr = decode_text_data(rebuilt.get_type_data(text_id));
    assert_eq!(rebuilt.get_str(sr), "Hello, world!");
}

#[test]
fn hast_empty_text_child_ref_with_nonzero_offset_is_remapped() {
    use satteri_arena::StringRef;
    use satteri_ast::hast::codec::{
        decode_element_tag, decode_text_data, encode_element_data, encode_text_data,
    };

    // Root > [Text "é", Element <pre>]
    let mut b = ArenaBuilder::<Hast>::new("é".to_string());
    b.open_node_raw(HastNodeType::Root as u8);

    // Add `Text "é"` which starts at byte 0 and is 2 bytes long in UTF-8.
    b.open_node_raw(HastNodeType::Text as u8);
    b.set_data_current(&encode_text_data(StringRef::new(0, 2)));
    b.close_node();

    // Add `Element <pre>`.
    b.open_node_raw(HastNodeType::Element as u8);
    let pre = b.alloc_string("pre");
    b.set_data_current(&encode_element_data(pre, &[]));
    b.close_node();

    b.close_node();
    let orig = b.finish();
    let elem_id = orig.get_children(0)[1];

    // Replace `<pre></pre>` by `<a></a>` with an empty text child.
    let mut sub = ArenaBuilder::<Hast>::new(String::new());
    sub.open_node_raw(HastNodeType::Element as u8);

    // Add `Element <a>`.
    let tag = sub.alloc_string("a");
    sub.set_data_current(&encode_element_data(tag, &[]));

    sub.open_node_raw(HastNodeType::Text as u8);

    // Add the empty text node. With the tag "a" taking up bytes 0..1 in the arena, the empty text
    // starts at byte 1 and has a length of 0.
    let empty = sub.alloc_string("");
    assert_eq!(empty.offset, 1);
    assert_eq!(empty.len, 0);
    sub.set_data_current(&encode_text_data(empty));
    sub.close_node();

    sub.close_node();
    let replacement = sub.finish();

    let rebuilt = rebuild_hast(
        &orig,
        &[Patch::Replace {
            node_id: elem_id,
            new_tree: replacement,
            keep_children: false,
        }],
    );

    // Root > [Text "é", Element <a> > Text ""]
    let root_children = rebuilt.get_children(0);
    assert_eq!(root_children.len(), 2);

    let original_text_id = root_children[0];
    let original_text = decode_text_data(rebuilt.get_type_data(original_text_id));
    assert_eq!(rebuilt.get_str(original_text), "é");

    let a_id = root_children[1];
    let a_data = rebuilt.get_type_data(a_id);
    assert_eq!(rebuilt.get_str(decode_element_tag(a_data)), "a");

    let a_children = rebuilt.get_children(a_id);
    assert_eq!(a_children.len(), 1);

    let text_id = a_children[0];
    let sr = decode_text_data(rebuilt.get_type_data(text_id));

    assert_eq!(sr.len, 0);
    // The empty text offset must be remapped from 1 (from the replacement arena) to a valid offset
    // in the rebuilt arena, otherwise, it would point to the middle of the "é" character.
    // In this test, this should be after `é` (2 bytes) + `pre` (3 bytes) from the source arena and
    // `a` (1 byte) from the replacement arena, so 6 in total.
    assert_eq!(sr.offset, 6);
    assert_eq!(rebuilt.get_str(sr), "");
}

#[test]
fn mdast_empty_text_child_ref_with_nonzero_offset_is_remapped() {
    use satteri_arena::StringRef;
    use satteri_ast::mdast::codec::{
        decode_link_data, decode_string_ref_data, encode_link_data, encode_string_ref_data,
    };

    // Root > [Text "é", Link (url "pre")]
    let mut b = ArenaBuilder::<Mdast>::new("é".to_string());
    b.open_node(MdastNodeType::Root as u8);

    // Add `Text "é"` which starts at byte 0 and is 2 bytes long in UTF-8.
    b.open_node(MdastNodeType::Text as u8);
    b.set_data_current(&encode_string_ref_data(StringRef::new(0, 2)));
    b.close_node();

    // Add `Link` with url "pre".
    b.open_node(MdastNodeType::Link as u8);
    let pre = b.alloc_string("pre");
    b.set_data_current(&encode_link_data(pre, StringRef::empty()));
    b.close_node();

    b.close_node();
    let orig = b.finish();
    let link_id = orig.get_children(0)[1];

    // Replace the link by a `Link` (url "a") with an empty text child.
    let mut sub = ArenaBuilder::<Mdast>::new(String::new());
    sub.open_node(MdastNodeType::Link as u8);

    // Add `Link` with url "a".
    let url = sub.alloc_string("a");
    sub.set_data_current(&encode_link_data(url, StringRef::empty()));

    sub.open_node(MdastNodeType::Text as u8);

    // Add the empty text node. With the url "a" taking up bytes 0..1 in the arena, the empty text
    // starts at byte 1 and has a length of 0.
    let empty = sub.alloc_string("");
    assert_eq!(empty.offset, 1);
    assert_eq!(empty.len, 0);
    sub.set_data_current(&encode_string_ref_data(empty));
    sub.close_node();

    sub.close_node();
    let replacement = sub.finish();

    let rebuilt = rebuild(
        &orig,
        &[Patch::Replace {
            node_id: link_id,
            new_tree: replacement,
            keep_children: false,
        }],
    );

    // Root > [Text "é", Link (url "a") > Text ""]
    let root_children = rebuilt.get_children(0);
    assert_eq!(root_children.len(), 2);

    let original_text_id = root_children[0];
    let original_text = decode_string_ref_data(rebuilt.get_type_data(original_text_id));
    assert_eq!(rebuilt.get_str(original_text), "é");

    let link_id = root_children[1];
    let link_data = rebuilt.get_type_data(link_id);
    assert_eq!(rebuilt.get_str(decode_link_data(link_data).url), "a");

    let link_children = rebuilt.get_children(link_id);
    assert_eq!(link_children.len(), 1);

    let text_id = link_children[0];
    let sr = decode_string_ref_data(rebuilt.get_type_data(text_id));

    assert_eq!(sr.len, 0);
    // The empty text offset must be remapped from 1 (from the replacement arena) to a valid offset
    // in the rebuilt arena, otherwise, it would point to the middle of the "é" character.
    // In this test, this should be after `é` (2 bytes) + `pre` (3 bytes) from the source arena and
    // `a` (1 byte) from the replacement arena, so 6 in total.
    assert_eq!(sr.offset, 6);
    assert_eq!(rebuilt.get_str(sr), "");
}

/// Replacing a block (the paragraph) with a `Root`-wrapped tree — as a raw
/// markdown return does — must splice the Root's child in, not the Root.
/// Before the fix this produced `Root > Root > Paragraph`.
#[test]
fn replace_with_root_wrapped_tree_strips_root() {
    let orig = build_hello_world();
    let para_id = orig.get_children(0)[1];

    let replacement = root_wrapped_arena(&[MdastNodeType::Paragraph]);
    let rebuilt = rebuild(
        &orig,
        &[Patch::Replace {
            node_id: para_id,
            new_tree: replacement,
            keep_children: false,
        }],
    );

    let root_children = rebuilt.get_children(0);
    assert_eq!(root_children.len(), 2, "heading + spliced paragraph");
    for &child in root_children {
        assert_ne!(
            rebuilt.get_node(child).node_type,
            MdastNodeType::Root as u8,
            "no nested Root may be spliced into the tree"
        );
    }
    assert_eq!(
        rebuilt.get_node(root_children[1]).node_type,
        MdastNodeType::Paragraph as u8
    );
}

/// Replacing an inline node (the text inside the heading) with a `Root`-wrapped
/// tree. Option B strips only the Root, so the parser's wrapping Paragraph
/// remains — a block in an inline slot — but no nested Root survives.
#[test]
fn replace_inline_with_root_wrapped_tree_strips_only_root() {
    let orig = build_hello_world();
    let heading_id = orig.get_children(0)[0];
    let text_id = orig.get_children(heading_id)[0];

    let replacement = root_wrapped_arena(&[MdastNodeType::Paragraph]);
    let rebuilt = rebuild(
        &orig,
        &[Patch::Replace {
            node_id: text_id,
            new_tree: replacement,
            keep_children: false,
        }],
    );

    let new_heading = rebuilt.get_children(0)[0];
    let heading_children = rebuilt.get_children(new_heading);
    assert_eq!(heading_children.len(), 1);
    assert_eq!(
        rebuilt.get_node(heading_children[0]).node_type,
        MdastNodeType::Paragraph as u8,
        "Root stripped, parser's Paragraph remains (block-level raw, by design)"
    );
}

/// A raw return parsing to several top-level blocks splices them all as
/// siblings into the slot.
#[test]
fn replace_with_multi_block_root_splices_all_siblings() {
    let orig = build_hello_world();
    let para_id = orig.get_children(0)[1];

    let replacement = root_wrapped_arena(&[MdastNodeType::Heading, MdastNodeType::ThematicBreak]);
    let rebuilt = rebuild(
        &orig,
        &[Patch::Replace {
            node_id: para_id,
            new_tree: replacement,
            keep_children: false,
        }],
    );

    let root_children = rebuilt.get_children(0);
    assert_eq!(
        root_children.len(),
        3,
        "original heading + 2 spliced blocks"
    );
    assert_eq!(
        rebuilt.get_node(root_children[1]).node_type,
        MdastNodeType::Heading as u8
    );
    assert_eq!(
        rebuilt.get_node(root_children[2]).node_type,
        MdastNodeType::ThematicBreak as u8
    );
}

/// An empty raw return (`Root` with no children) removes the slot cleanly
/// rather than leaving an empty Root behind.
#[test]
fn replace_with_empty_root_removes_slot() {
    let orig = build_hello_world();
    let para_id = orig.get_children(0)[1];

    let replacement = root_wrapped_arena(&[]);
    let rebuilt = rebuild(
        &orig,
        &[Patch::Replace {
            node_id: para_id,
            new_tree: replacement,
            keep_children: false,
        }],
    );

    let root_children = rebuilt.get_children(0);
    assert_eq!(root_children.len(), 1, "only the heading remains");
    assert_eq!(
        rebuilt.get_node(root_children[0]).node_type,
        MdastNodeType::Heading as u8
    );
}

/// Regression: a `Replace` whose subtree references the replaced node itself
/// must splice the original once, not recurse forever. This is the "wrap a
/// heading in a div containing the heading" shape (Starlight autolink) that
/// previously overflowed the stack.
#[test]
fn replace_with_ref_to_self_splices_once() {
    use satteri_arena::StringRef;
    use satteri_ast::hast::codec::encode_element_data;
    use satteri_ast::rebuild::REF_NODE_TYPE;

    // Root > Element(h2) > Text "Heading"
    let mut b = ArenaBuilder::<Hast>::new("Heading".to_string());
    b.open_node(HastNodeType::Root as u8);
    b.open_node(HastNodeType::Element as u8);
    let tag = b.alloc_string("h2");
    b.set_data_current(&encode_element_data(tag, &[]));
    b.open_node(HastNodeType::Text as u8);
    b.set_data_current(&satteri_ast::hast::codec::encode_text_data(StringRef::new(
        0, 7,
    )));
    b.close_node(); // text
    b.close_node(); // element
    b.close_node(); // root
    let orig = b.finish();

    let heading_id = orig.get_children(0)[0];

    // Replacement subtree: <div>{ REF -> heading_id }</div>
    let mut sub = ArenaBuilder::<Hast>::new(String::new());
    sub.open_node(HastNodeType::Element as u8);
    let div_tag = sub.alloc_string("div");
    sub.set_data_current(&encode_element_data(div_tag, &[]));
    sub.open_node_raw(REF_NODE_TYPE);
    sub.set_data_current(&heading_id.to_le_bytes());
    sub.close_node(); // ref
    sub.close_node(); // div
    let new_tree = sub.finish();

    let rebuilt = rebuild_hast(
        &orig,
        &[Patch::Replace {
            node_id: heading_id,
            new_tree,
            keep_children: false,
        }],
    );

    // Expect: Root > div > h2 > text, no runaway duplication.
    let root_children = rebuilt.get_children(0);
    assert_eq!(root_children.len(), 1, "root holds the single wrapper div");
    let div = root_children[0];
    assert_eq!(rebuilt.get_node(div).node_type, HastNodeType::Element as u8);

    let div_children = rebuilt.get_children(div);
    assert_eq!(
        div_children.len(),
        1,
        "div wraps exactly the original heading"
    );
    let inner = div_children[0];
    assert_eq!(
        rebuilt.get_node(inner).node_type,
        HastNodeType::Element as u8,
        "the re-parented node is the original heading element"
    );
    assert_eq!(
        rebuilt.get_children(inner).len(),
        1,
        "heading keeps its original text child"
    );
    assert_eq!(
        rebuilt.get_node(rebuilt.get_children(inner)[0]).node_type,
        HastNodeType::Text as u8
    );
}

/// `Wrap` keeps the wrapper's own declared children, with the wrapped node
/// emitted first: `div > [anchor]` wrapping a heading yields
/// `div > [heading, anchor]` rather than dropping the anchor.
#[test]
fn wrap_keeps_wrapper_children_after_wrapped_node() {
    use satteri_arena::StringRef;
    use satteri_ast::hast::codec::{encode_element_data, encode_text_data};

    // Root > Element(h2) > Text "Hello"
    let mut b = ArenaBuilder::<Hast>::new("Hello".to_string());
    b.open_node(HastNodeType::Root as u8);
    b.open_node(HastNodeType::Element as u8);
    let h2 = b.alloc_string("h2");
    b.set_data_current(&encode_element_data(h2, &[]));
    b.open_node(HastNodeType::Text as u8);
    b.set_data_current(&encode_text_data(StringRef::new(0, 5)));
    b.close_node(); // text
    b.close_node(); // h2
    b.close_node(); // root
    let orig = b.finish();
    let heading_id = orig.get_children(0)[0];

    // Wrapper: div > a (the anchor sibling), built as a sub-arena
    let mut w = ArenaBuilder::<Hast>::new(String::new());
    w.open_node(HastNodeType::Element as u8);
    let div = w.alloc_string("div");
    w.set_data_current(&encode_element_data(div, &[]));
    w.open_node(HastNodeType::Element as u8);
    let a = w.alloc_string("a");
    w.set_data_current(&encode_element_data(a, &[]));
    w.close_node(); // a (no children)
    w.close_node(); // div
    let parent_tree = w.finish();

    let rebuilt = rebuild_hast(
        &orig,
        &[Patch::Wrap {
            node_id: heading_id,
            parent_tree,
        }],
    );

    // Root > div > [h2(>text), a]
    let div_id = rebuilt.get_children(0)[0];
    assert_eq!(
        rebuilt.get_node(div_id).node_type,
        HastNodeType::Element as u8
    );
    let kids = rebuilt.get_children(div_id);
    assert_eq!(kids.len(), 2, "wrapped node plus the wrapper's own child");

    // First child is the wrapped heading (keeps its text child).
    assert_eq!(
        rebuilt.get_node(kids[0]).node_type,
        HastNodeType::Element as u8
    );
    assert_eq!(
        rebuilt.get_children(kids[0]).len(),
        1,
        "wrapped node is first and keeps its original children"
    );
    assert_eq!(
        rebuilt.get_node(rebuilt.get_children(kids[0])[0]).node_type,
        HastNodeType::Text as u8
    );

    // Second child is the wrapper's declared anchor (childless).
    assert_eq!(
        rebuilt.get_node(kids[1]).node_type,
        HastNodeType::Element as u8
    );
    assert_eq!(
        rebuilt.get_children(kids[1]).len(),
        0,
        "wrapper's declared child follows the wrapped node"
    );
}

/// A wrapped node keeps `PrependChild`/`AppendChild` patches queued on it in
/// the same pass: wrapping must not discard child patches on the node it wraps.
/// Prepend lands before the node's original children, append after.
#[test]
fn wrap_applies_prepend_and_append_child_on_wrapped_node() {
    use satteri_ast::hast::codec::encode_element_data;

    fn single(node_type: HastNodeType) -> Arena<Hast> {
        let mut b = ArenaBuilder::<Hast>::new(String::new());
        b.open_node(node_type as u8);
        b.close_node();
        b.finish()
    }

    // Root > Element(h2) > Element(em) — original child is an element.
    let mut b = ArenaBuilder::<Hast>::new(String::new());
    b.open_node(HastNodeType::Root as u8);
    b.open_node(HastNodeType::Element as u8);
    let h2 = b.alloc_string("h2");
    b.set_data_current(&encode_element_data(h2, &[]));
    b.open_node(HastNodeType::Element as u8);
    let em = b.alloc_string("em");
    b.set_data_current(&encode_element_data(em, &[]));
    b.close_node(); // em
    b.close_node(); // h2
    b.close_node(); // root
    let orig = b.finish();
    let heading_id = orig.get_children(0)[0];

    let rebuilt = rebuild_hast(
        &orig,
        &[
            Patch::Wrap {
                node_id: heading_id,
                parent_tree: single(HastNodeType::Element), // <div>-shaped wrapper
            },
            Patch::PrependChild {
                node_id: heading_id,
                child_tree: single(HastNodeType::Text),
            },
            Patch::AppendChild {
                node_id: heading_id,
                child_tree: single(HastNodeType::Comment),
            },
        ],
    );

    // Root > wrapper > h2 > [Text(prepend), Element(em), Comment(append)]
    let wrapper = rebuilt.get_children(0)[0];
    let h2_id = rebuilt.get_children(wrapper)[0];
    let kids = rebuilt.get_children(h2_id);
    assert_eq!(kids.len(), 3, "prepend + original + append, none dropped");
    assert_eq!(
        rebuilt.get_node(kids[0]).node_type,
        HastNodeType::Text as u8
    );
    assert_eq!(
        rebuilt.get_node(kids[1]).node_type,
        HastNodeType::Element as u8
    );
    assert_eq!(
        rebuilt.get_node(kids[2]).node_type,
        HastNodeType::Comment as u8
    );
}
