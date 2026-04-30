//! Integration tests for arena rebuild.
//!
//! Tests apply patches to the "# Hello\n\nWorld" arena and verify the resulting structure.

use satteri_arena::{Arena, ArenaBuilder};
use satteri_ast::mdast::MdastNodeType;
use satteri_ast::rebuild::{rebuild as rebuild_raw, Patch};

fn rebuild(arena: &Arena, patches: &[Patch]) -> Arena {
    rebuild_raw(arena, patches).expect("rebuild failed")
}

/// Tree structure:
///   Root (0)
///     Heading depth=1 (1)
///       Text "Hello" (2)
///     Paragraph (3)
///       Text "World" (4)
fn build_hello_world() -> Arena {
    use satteri_arena::StringRef;
    use satteri_ast::mdast::codec::{encode_heading_data, encode_string_ref_data};

    let source = "# Hello\n\nWorld".to_string();
    let mut b = ArenaBuilder::new(source);

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

fn single_node_arena(node_type: MdastNodeType) -> Arena {
    let mut b = ArenaBuilder::new(String::new());
    b.open_node(node_type as u8);
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

    let mut b = ArenaBuilder::new(source);
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
    let mut sub = ArenaBuilder::new(String::new());
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
