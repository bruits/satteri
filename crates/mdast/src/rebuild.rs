//! MdastArena rebuild: apply structural patches to produce a new arena.

use std::collections::{HashMap, HashSet};

use crate::{MdastArena, MdastBuilder, MdastNodeType};

#[derive(Debug, Clone)]
pub enum Patch {
    Replace {
        node_id: u32,
        new_tree: MdastArena,
    },
    /// Removes the entire subtree rooted at this node
    Remove {
        node_id: u32,
    },
    /// Inserted as a preceding sibling
    InsertBefore {
        node_id: u32,
        new_tree: MdastArena,
    },
    /// Inserted as a following sibling
    InsertAfter {
        node_id: u32,
        new_tree: MdastArena,
    },
    /// The original node becomes a child of the new parent
    Wrap {
        node_id: u32,
        parent_tree: MdastArena,
    },
    PrependChild {
        node_id: u32,
        child_tree: MdastArena,
    },
    AppendChild {
        node_id: u32,
        child_tree: MdastArena,
    },
}

/// Node IDs in the new arena are assigned fresh (monotonically increasing)
/// but the structure is preserved. Sub-arena type_data bytes are copied
/// verbatim — full StringRef remapping is deferred to Phase 8.
pub fn rebuild(arena: &MdastArena, patches: &[Patch]) -> MdastArena {
    let mut patch_map: HashMap<u32, &Patch> = HashMap::new();
    for patch in patches {
        let node_id = match patch {
            Patch::Replace { node_id, .. } => *node_id,
            Patch::Remove { node_id } => *node_id,
            Patch::InsertBefore { node_id, .. } => *node_id,
            Patch::InsertAfter { node_id, .. } => *node_id,
            Patch::Wrap { node_id, .. } => *node_id,
            Patch::PrependChild { node_id, .. } => *node_id,
            Patch::AppendChild { node_id, .. } => *node_id,
        };
        patch_map.insert(node_id, patch);
    }

    // Replaced or removed nodes are skipped during normal traversal
    let mut deleted: HashSet<u32> = HashSet::new();
    for patch in patches {
        match patch {
            Patch::Remove { node_id } => {
                deleted.insert(*node_id);
            }
            Patch::Replace { node_id, .. } => {
                deleted.insert(*node_id);
            }
            _ => {}
        }
    }

    let new_source = arena.source().to_string();
    let mut builder = MdastBuilder::new(new_source);

    copy_node(0, arena, &mut builder, &patch_map, &deleted);

    builder.finish()
}

/// Returns `true` if the node was emitted (or a replacement was emitted),
/// `false` if skipped (Remove).
fn copy_node(
    node_id: u32,
    orig: &MdastArena,
    builder: &mut MdastBuilder,
    patch_map: &HashMap<u32, &Patch>,
    deleted: &HashSet<u32>,
) -> bool {
    // For Replace patches, the replacement is emitted here (not by the parent)
    // when this is the root node or when copy_children delegates to copy_node.
    if deleted.contains(&node_id) {
        if let Some(Patch::Replace { new_tree, .. }) = patch_map.get(&node_id) {
            emit_subtree(new_tree, builder);
            return true;
        }
        return false;
    }

    if let Some(Patch::InsertBefore { new_tree, .. }) = patch_map.get(&node_id) {
        emit_subtree(new_tree, builder);
    }

    // Wrap: parent_tree's root becomes the wrapper; the original node becomes
    // its only child. Any existing children in parent_tree are ignored (Phase 6
    // simplification).
    if let Some(Patch::Wrap { parent_tree, .. }) = patch_map.get(&node_id) {
        emit_wrap_node(parent_tree, node_id, orig, builder, patch_map, deleted);

        // InsertAfter (after the wrapped group)
        if let Some(Patch::InsertAfter { new_tree, .. }) = patch_map.get(&node_id) {
            emit_subtree(new_tree, builder);
        }
        return true;
    }

    let node = orig.get_node(node_id);
    let node_type =
        MdastNodeType::from_u8(node.node_type).expect("unknown node type in arena — corrupt data");

    builder.open_node(node_type);

    builder.set_position_current(
        node.start_offset,
        node.end_offset,
        node.start_line,
        node.start_column,
        node.end_line,
        node.end_column,
    );

    let type_data = orig.get_type_data(node_id);
    if !type_data.is_empty() {
        builder.set_data_current(type_data);
    }

    if let Some(Patch::PrependChild { child_tree, .. }) = patch_map.get(&node_id) {
        emit_subtree(child_tree, builder);
    }

    let child_ids: Vec<u32> = orig.get_children(node_id).to_vec();
    for child_id in child_ids {
        if deleted.contains(&child_id) {
            if let Some(Patch::Replace { new_tree, .. }) = patch_map.get(&child_id) {
                emit_subtree(new_tree, builder);
            }
        } else {
            copy_node(child_id, orig, builder, patch_map, deleted);
        }
    }

    if let Some(Patch::AppendChild { child_tree, .. }) = patch_map.get(&node_id) {
        emit_subtree(child_tree, builder);
    }

    builder.close_node();

    if let Some(Patch::InsertAfter { new_tree, .. }) = patch_map.get(&node_id) {
        emit_subtree(new_tree, builder);
    }

    true
}

/// Sub-arena source is appended to the builder's source, and StringRef
/// offsets in type_data are remapped into the merged buffer.
fn emit_subtree(sub_arena: &MdastArena, builder: &mut MdastBuilder) {
    if sub_arena.is_empty() {
        return;
    }
    let sub_source = sub_arena.source();
    let source_base = if sub_source.is_empty() {
        0u32
    } else {
        let sref = builder.alloc_string(sub_source);
        sref.offset
    };
    emit_subtree_node(0, sub_arena, builder, source_base);
}

/// `source_base` is the offset added to StringRef offsets to remap them
/// into the merged source buffer.
fn emit_subtree_node(
    node_id: u32,
    sub_arena: &MdastArena,
    builder: &mut MdastBuilder,
    source_base: u32,
) {
    let node = sub_arena.get_node(node_id);
    let node_type = MdastNodeType::from_u8(node.node_type)
        .expect("unknown node type in sub-arena — corrupt data");

    builder.open_node(node_type);

    builder.set_position_current(
        node.start_offset + source_base,
        node.end_offset + source_base,
        node.start_line,
        node.start_column,
        node.end_line,
        node.end_column,
    );

    let type_data = sub_arena.get_type_data(node_id);
    if !type_data.is_empty() {
        if source_base != 0 {
            let mut remapped = type_data.to_vec();
            remap_string_refs(&mut remapped, node.node_type, source_base);
            builder.set_data_current(&remapped);
        } else {
            builder.set_data_current(type_data);
        }
    }

    let child_ids: Vec<u32> = sub_arena.get_children(node_id).to_vec();
    for child_id in child_ids {
        emit_subtree_node(child_id, sub_arena, builder, source_base);
    }

    builder.close_node();
}

/// Add `base` to all StringRef offset fields in type_data.
/// StringRefs are `(offset: u32 LE, len: u32 LE)` pairs at known positions
/// depending on the node type.
fn remap_string_refs(data: &mut [u8], node_type: u8, base: u32) {
    // StringRef positions depend on node type; each is (offset: u32 LE, len: u32 LE)
    let ref_offsets: &[usize] = match node_type {
        // Text, InlineCode, Html, Yaml, Toml, InlineMath: single StringRef at 0
        10 | 13 | 7 | 25 | 26 | 28 => &[0],
        // Code: lang(0), meta(8), value(16)
        8 => &[0, 8, 16],
        // Math: meta(0), value(8)
        27 => &[0, 8],
        // Link: url(0), title(8)
        15 => &[0, 8],
        // Image: url(0), alt(8), title(16)
        16 => &[0, 8, 16],
        // Definition: url(0), title(8), identifier(16), label(24)
        9 => &[0, 8, 16, 24],
        // LinkReference, ImageReference, FootnoteReference: identifier(0), label(8)
        17 | 18 | 20 => &[0, 8],
        // FootnoteDefinition: identifier(0), label(8)
        19 => &[0, 8],
        // MdxJsxFlowElement, MdxJsxTextElement: variable-length (handled below)
        100 | 101 => &[],
        // MdxFlowExpression, MdxTextExpression, MdxjsEsm: value(0)
        102..=104 => &[0],
        // HAST_TEXT(2), HAST_COMMENT(3), HAST_RAW(5),
        // HAST_MDX_EXPRESSION(12): single StringRef at 0
        // (HAST_MDX_ESM=13 is already covered by InlineCode=13 above)
        2 | 3 | 5 | 12 => &[0],
        // Heading(depth u8), List, ListItem, Table, HAST_ROOT(0), HAST_DOCTYPE(4), etc.
        _ => &[],
    };

    // HAST element types (1, 10, 11) have variable-length property/attribute data.
    // Handle them specially: tag/name StringRef at 0, then props/attrs at fixed stride.
    match node_type {
        // HAST_ELEMENT: tag(0), then each prop: name at 16+i*20, value at 16+i*20+12
        1 => {
            remap_one_ref(data, 0, base);
            if data.len() >= 12 {
                let prop_count =
                    u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
                for i in 0..prop_count {
                    let prop_base = 16 + i * 20;
                    remap_one_ref(data, prop_base, base); // name
                    remap_one_ref(data, prop_base + 12, base); // value
                }
            }
            return;
        }
        // MDX JSX elements: MDAST(100,101) and HAST(10,11): name(0), then attrs
        10 | 11 | 100 | 101 if data.len() >= 16 => {
            remap_one_ref(data, 0, base);
            let attr_count = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
            for i in 0..attr_count {
                let attr_base = 16 + i * 20;
                remap_one_ref(data, attr_base + 4, base); // name
                remap_one_ref(data, attr_base + 12, base); // value
            }
            return;
        }
        _ => {}
    }

    for &off in ref_offsets {
        remap_one_ref(data, off, base);
    }
}

fn remap_one_ref(data: &mut [u8], off: usize, base: u32) {
    if off + 8 <= data.len() {
        let current = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
        let len = u32::from_le_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]);
        if len > 0 {
            let new_offset = current + base;
            data[off..off + 4].copy_from_slice(&new_offset.to_le_bytes());
        }
    }
}

/// Assumes parent_tree's root is the single wrapper node. Any children
/// already present in parent_tree are ignored — the original node becomes
/// the sole child (Phase 6 simplification).
fn emit_wrap_node(
    parent_tree: &MdastArena,
    original_node_id: u32,
    orig: &MdastArena,
    builder: &mut MdastBuilder,
    patch_map: &HashMap<u32, &Patch>,
    deleted: &HashSet<u32>,
) {
    if parent_tree.is_empty() {
        // Degenerate: no wrapper, just emit original
        copy_node(original_node_id, orig, builder, patch_map, deleted);
        return;
    }

    let wrapper = parent_tree.get_node(0);
    let node_type =
        MdastNodeType::from_u8(wrapper.node_type).expect("unknown node type in wrapper arena");

    builder.open_node(node_type);
    builder.set_position_current(
        wrapper.start_offset,
        wrapper.end_offset,
        wrapper.start_line,
        wrapper.start_column,
        wrapper.end_line,
        wrapper.end_column,
    );
    let wrapper_data = parent_tree.get_type_data(0);
    if !wrapper_data.is_empty() {
        builder.set_data_current(wrapper_data);
    }

    // Emit the original node as the child
    copy_node(original_node_id, orig, builder, patch_map, deleted);

    builder.close_node();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MdastBuilder, MdastNodeType};

    /// Build the "# Hello\n\nWorld" arena for testing.
    fn build_hello_world() -> MdastArena {
        use crate::codec::{encode_heading_data, encode_string_ref_data};
        use crate::node::StringRef;

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
        b.close_node(); // text

        b.close_node(); // heading

        b.open_node(MdastNodeType::Paragraph);
        b.set_position_current(9, 14, 2, 1, 2, 6);

        b.open_node(MdastNodeType::Text);
        b.set_position_current(9, 14, 2, 1, 2, 6);
        b.set_data_current(&encode_string_ref_data(StringRef::new(9, 5)));
        b.close_node(); // text

        b.close_node(); // paragraph
        b.close_node(); // root

        b.finish()
    }

    #[test]
    fn empty_patches_preserves_structure() {
        let orig = build_hello_world();
        let rebuilt = rebuild(&orig, &[]);
        assert_eq!(rebuilt.len(), orig.len(), "node count must be the same");
        // Root still has 2 children
        assert_eq!(rebuilt.get_children(0).len(), 2);
    }

    #[test]
    fn remove_leaf_node() {
        // Remove the Text node inside Heading (node 2 in the original tree).
        // Original: Root(0) -> Heading(1) -> Text(2), Paragraph(3) -> Text(4)
        let orig = build_hello_world();
        // Find the Text child of Heading
        let heading_id = orig.get_children(0)[0]; // id=1
        let text_in_heading = orig.get_children(heading_id)[0]; // id=2

        let patches = vec![Patch::Remove {
            node_id: text_in_heading,
        }];
        let rebuilt = rebuild(&orig, &patches);

        // We should have 4 nodes: Root, Heading (now empty), Paragraph, Text(World)
        assert_eq!(rebuilt.len(), 4, "text under heading should be removed");

        // Heading child in rebuilt arena — find heading
        let new_root_children = rebuilt.get_children(0);
        assert_eq!(new_root_children.len(), 2);
        let new_heading_id = new_root_children[0];
        assert_eq!(
            rebuilt.get_node(new_heading_id).node_type,
            MdastNodeType::Heading as u8
        );
        assert_eq!(
            rebuilt.get_children(new_heading_id).len(),
            0,
            "heading should have no children"
        );
    }

    #[test]
    fn remove_non_leaf_removes_subtree() {
        let orig = build_hello_world();
        // Remove the Heading (and its Text child)
        let heading_id = orig.get_children(0)[0]; // id=1
        let patches = vec![Patch::Remove {
            node_id: heading_id,
        }];
        let rebuilt = rebuild(&orig, &patches);

        // Root + Paragraph + Text(World) = 3 nodes
        assert_eq!(rebuilt.len(), 3);
        let new_root_children = rebuilt.get_children(0);
        assert_eq!(new_root_children.len(), 1);
        assert_eq!(
            rebuilt.get_node(new_root_children[0]).node_type,
            MdastNodeType::Paragraph as u8
        );
    }

    #[test]
    fn replace_leaf_node() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];
        let text_id = orig.get_children(heading_id)[0];

        // Build a replacement: a ThematicBreak (no children, no data)
        let mut replacement_builder = MdastBuilder::new(orig.source().to_string());
        replacement_builder.open_node(MdastNodeType::ThematicBreak);
        replacement_builder.close_node();
        let replacement = replacement_builder.finish();

        let patches = vec![Patch::Replace {
            node_id: text_id,
            new_tree: replacement,
        }];
        let rebuilt = rebuild(&orig, &patches);

        // Same node count (Text replaced by ThematicBreak, 1-for-1)
        assert_eq!(rebuilt.len(), orig.len());
        // Find ThematicBreak under Heading
        let new_heading_id = rebuilt.get_children(0)[0];
        let child_of_heading = rebuilt.get_children(new_heading_id)[0];
        assert_eq!(
            rebuilt.get_node(child_of_heading).node_type,
            MdastNodeType::ThematicBreak as u8
        );
    }

    #[test]
    fn replace_root_child_with_different_type() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];

        // Replace Heading with a Paragraph
        let mut replacement_builder = MdastBuilder::new(orig.source().to_string());
        replacement_builder.open_node(MdastNodeType::Paragraph);
        replacement_builder.close_node();
        let replacement = replacement_builder.finish();

        let patches = vec![Patch::Replace {
            node_id: heading_id,
            new_tree: replacement,
        }];
        let rebuilt = rebuild(&orig, &patches);

        // Root should still have 2 children; first one is now Paragraph
        let root_children = rebuilt.get_children(0);
        assert_eq!(root_children.len(), 2);
        assert_eq!(
            rebuilt.get_node(root_children[0]).node_type,
            MdastNodeType::Paragraph as u8
        );
        // Second child should still be the original Paragraph
        assert_eq!(
            rebuilt.get_node(root_children[1]).node_type,
            MdastNodeType::Paragraph as u8
        );
    }

    #[test]
    fn insert_before_node() {
        let orig = build_hello_world();
        let para_id = orig.get_children(0)[1]; // Paragraph node

        // Insert a ThematicBreak before the Paragraph
        let mut new_tree_builder = MdastBuilder::new(orig.source().to_string());
        new_tree_builder.open_node(MdastNodeType::ThematicBreak);
        new_tree_builder.close_node();
        let new_tree = new_tree_builder.finish();

        let patches = vec![Patch::InsertBefore {
            node_id: para_id,
            new_tree,
        }];
        let rebuilt = rebuild(&orig, &patches);

        // Root should now have 3 children: Heading, ThematicBreak, Paragraph
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
        let heading_id = orig.get_children(0)[0]; // Heading node

        let mut new_tree_builder = MdastBuilder::new(orig.source().to_string());
        new_tree_builder.open_node(MdastNodeType::ThematicBreak);
        new_tree_builder.close_node();
        let new_tree = new_tree_builder.finish();

        let patches = vec![Patch::InsertAfter {
            node_id: heading_id,
            new_tree,
        }];
        let rebuilt = rebuild(&orig, &patches);

        // Root should now have 3 children: Heading, ThematicBreak, Paragraph
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

        let mut child_builder = MdastBuilder::new(orig.source().to_string());
        child_builder.open_node(MdastNodeType::Break);
        child_builder.close_node();
        let child_tree = child_builder.finish();

        let patches = vec![Patch::AppendChild {
            node_id: heading_id,
            child_tree,
        }];
        let rebuilt = rebuild(&orig, &patches);

        // Heading should now have 2 children: original Text + new Break
        let new_heading_id = rebuilt.get_children(0)[0];
        let heading_children = rebuilt.get_children(new_heading_id);
        assert_eq!(heading_children.len(), 2);
        assert_eq!(
            rebuilt.get_node(heading_children[0]).node_type,
            MdastNodeType::Text as u8
        );
        assert_eq!(
            rebuilt.get_node(heading_children[1]).node_type,
            MdastNodeType::Break as u8
        );
    }

    #[test]
    fn prepend_child() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];

        let mut child_builder = MdastBuilder::new(orig.source().to_string());
        child_builder.open_node(MdastNodeType::Break);
        child_builder.close_node();
        let child_tree = child_builder.finish();

        let patches = vec![Patch::PrependChild {
            node_id: heading_id,
            child_tree,
        }];
        let rebuilt = rebuild(&orig, &patches);

        // Heading should now have 2 children: new Break + original Text
        let new_heading_id = rebuilt.get_children(0)[0];
        let heading_children = rebuilt.get_children(new_heading_id);
        assert_eq!(heading_children.len(), 2);
        assert_eq!(
            rebuilt.get_node(heading_children[0]).node_type,
            MdastNodeType::Break as u8
        );
        assert_eq!(
            rebuilt.get_node(heading_children[1]).node_type,
            MdastNodeType::Text as u8
        );
    }

    #[test]
    fn multiple_patches_applied_together() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];
        let para_id = orig.get_children(0)[1];

        // Remove the heading AND insert a ThematicBreak after paragraph
        let mut new_tree_builder = MdastBuilder::new(orig.source().to_string());
        new_tree_builder.open_node(MdastNodeType::ThematicBreak);
        new_tree_builder.close_node();
        let new_tree = new_tree_builder.finish();

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

        // Root should have 2 children: original Paragraph + new ThematicBreak
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
    }
}
