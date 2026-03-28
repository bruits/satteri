//! Integration tests for arena rebuild.
//!
//! Tests apply patches to the "# Hello\n\nWorld" arena and verify the resulting structure.

use tryckeri_mdast::{
    rebuild::{rebuild, Patch},
    MdastArena, MdastBuilder, MdastNodeType,
};

/// Tree structure:
///   Root (0)
///     Heading depth=1 (1)
///       Text "Hello" (2)
///     Paragraph (3)
///       Text "World" (4)
fn build_hello_world() -> MdastArena {
    use tryckeri_mdast::codec::{encode_heading_data, encode_string_ref_data};
    use tryckeri_mdast::StringRef;

    let source = "# Hello\n\nWorld".to_string();
    let mut b = MdastBuilder::new(source);

    b.open_node(MdastNodeType::Root);
    b.set_position_current(0, 14, 1, 1, 2, 6);

    b.open_node(MdastNodeType::Heading);
    b.set_position_current(0, 7, 1, 1, 1, 8);
    b.set_data_current(&encode_heading_data(1));

    b.open_node(MdastNodeType::Text);
    b.set_position_current(2, 7, 1, 3, 1, 8);
    b.set_data_current(&encode_string_ref_data(StringRef::new(2, 5)));
    b.close_node();

    b.close_node(); // heading

    b.open_node(MdastNodeType::Paragraph);
    b.set_position_current(9, 14, 2, 1, 2, 6);

    b.open_node(MdastNodeType::Text);
    b.set_position_current(9, 14, 2, 1, 2, 6);
    b.set_data_current(&encode_string_ref_data(StringRef::new(9, 5)));
    b.close_node();

    b.close_node(); // paragraph
    b.close_node(); // root

    b.finish()
}

fn single_node_arena(node_type: MdastNodeType) -> MdastArena {
    let mut b = MdastBuilder::new(String::new());
    b.open_node(node_type);
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
