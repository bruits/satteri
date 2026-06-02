//! Arena rebuild: apply structural patches to produce a new arena.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::commands::CommandError;
use satteri_arena::{Arena, ArenaBuilder, ArenaKind, Hast, Mdast};

/// Sentinel `node_type` for a *reference* node inside a replacement sub-tree:
/// "splice the existing original node whose id is stored in this node's
/// type_data (u32 LE) here." Higher than any real MDAST (≤104) or HAST (≤14)
/// type. Resolving it copies the original subtree *and applies any pending
/// patch on it*, so a nested transform queued on a passed-through child still
/// lands — no stranding, no re-visit.
pub const REF_NODE_TYPE: u8 = 0xFF;

#[derive(Debug, Clone)]
pub enum Patch<K: ArenaKind> {
    Replace {
        node_id: u32,
        new_tree: Arena<K>,
        keep_children: bool,
    },
    /// Removes the entire subtree rooted at this node
    Remove {
        node_id: u32,
    },
    /// Inserted as a preceding sibling
    InsertBefore {
        node_id: u32,
        new_tree: Arena<K>,
    },
    /// Inserted as a following sibling
    InsertAfter {
        node_id: u32,
        new_tree: Arena<K>,
    },
    /// The original node becomes a child of the new parent
    Wrap {
        node_id: u32,
        parent_tree: Arena<K>,
    },
    PrependChild {
        node_id: u32,
        child_tree: Arena<K>,
    },
    AppendChild {
        node_id: u32,
        child_tree: Arena<K>,
    },
}

/// Node IDs in the new arena are assigned fresh (monotonically increasing)
/// but the structure is preserved. Sub-arena type_data bytes are copied
/// verbatim; full StringRef remapping is deferred to Phase 8.
///
/// Multiple patches anchored on the same `node_id` are preserved in buffer
/// order — e.g. several `InsertBefore` calls on the same anchor each emit
/// their sub-tree, in the order they were issued. `Remove` (or `Replace`)
/// composes with sibling inserts on the same anchor: pre-inserts emit, the
/// node is replaced or skipped, then post-inserts emit.
///
/// Returns an error if a patch combination would silently drop work:
///   - `Wrap` / `PrependChild` / `AppendChild` on an anchor that is also
///     removed or replaced — there's no inside left for the child, and no
///     original to wrap.
///   - Any patch on an anchor whose subtree was discarded by an ancestor's
///     `Remove` (or by a `Replace { keep_children: false }`).
pub fn rebuild<K: ArenaKind>(
    arena: &Arena<K>,
    patches: &[Patch<K>],
) -> Result<Arena<K>, CommandError> {
    let result = rebuild_lenient(arena, patches)?;
    if let Some(&anchor) = result.dropped.first() {
        return Err(CommandError::PatchOnRemovedSubtree(anchor));
    }
    Ok(result.arena)
}

/// Outcome of [`rebuild_lenient`].
pub struct RebuildResult<K: ArenaKind> {
    pub arena: Arena<K>,
    /// Anchors whose patch landed inside a subtree that an ancestor's
    /// `Remove`/`Replace` genuinely discarded, so the patch could not be
    /// applied — and is moot, since the plugin chose to drop that subtree. A
    /// *passed-through* child is not dropped this way: it is spliced back by a
    /// `REF_NODE_TYPE` node (see [`REF_NODE_TYPE`]), keeping its id so a patch
    /// queued on it still applies.
    pub dropped: Vec<u32>,
}

/// Like [`rebuild`], but instead of erroring when a patch targets a node inside
/// a removed/replaced subtree, drops that patch and reports its anchor in
/// [`RebuildResult::dropped`]. Genuine misuse that can't be re-derived
/// (`Wrap`/`PrependChild`/`AppendChild` on a removed anchor) still errors.
pub fn rebuild_lenient<K: ArenaKind>(
    arena: &Arena<K>,
    patches: &[Patch<K>],
) -> Result<RebuildResult<K>, CommandError> {
    let mut patch_map: FxHashMap<u32, Vec<&Patch<K>>> = FxHashMap::default();
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
        patch_map.entry(node_id).or_default().push(patch);
    }

    // Replaced or removed nodes are skipped during normal traversal
    let mut deleted: FxHashSet<u32> = FxHashSet::default();
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

    // Pre-flight: Wrap / PrependChild / AppendChild against a deleted anchor
    // can't be honored — the node won't exist to wrap, and the deleted node
    // has no inside to receive children. Sibling inserts (Before/After) are
    // fine: they emit around the absence.
    for patch in patches {
        match patch {
            Patch::Wrap { node_id, .. } if deleted.contains(node_id) => {
                return Err(CommandError::WrapOnRemovedNode(*node_id));
            }
            Patch::PrependChild { node_id, .. } | Patch::AppendChild { node_id, .. }
                if deleted.contains(node_id) =>
            {
                return Err(CommandError::ChildPatchOnRemovedNode(*node_id));
            }
            _ => {}
        }
    }

    let new_source = arena.source().to_string();
    let mut builder: ArenaBuilder<K> = ArenaBuilder::new(new_source);

    let mut visited: FxHashSet<u32> = FxHashSet::default();
    copy_node(0, arena, &mut builder, &patch_map, &deleted, &mut visited);

    // Any anchor in patch_map that wasn't reached during the walk lives
    // inside a removed subtree (or a Replace { keep_children: false }
    // subtree) that no reference spliced back, so its patch was not applied.
    // Report it; the lenient caller drops it (the subtree was discarded).
    let mut dropped: Vec<u32> = patch_map
        .keys()
        .copied()
        .filter(|anchor| !visited.contains(anchor))
        .collect();
    dropped.sort_unstable();

    Ok(RebuildResult {
        arena: builder.finish(),
        dropped,
    })
}

/// Returns `true` if the node was emitted (or a replacement was emitted),
/// `false` if skipped entirely (Remove with no sibling inserts).
///
/// `visited` accumulates every reached anchor (deleted or not) so that the
/// caller can detect anchors stranded inside discarded subtrees.
fn copy_node<K: ArenaKind>(
    node_id: u32,
    orig: &Arena<K>,
    builder: &mut ArenaBuilder<K>,
    patch_map: &FxHashMap<u32, Vec<&Patch<K>>>,
    deleted: &FxHashSet<u32>,
    visited: &mut FxHashSet<u32>,
) -> bool {
    let node_patches: &[&Patch<K>] = patch_map.get(&node_id).map(Vec::as_slice).unwrap_or(&[]);
    if !node_patches.is_empty() {
        visited.insert(node_id);
    }

    // Pre-siblings emit before either the original node, its replacement, or
    // its absence — whichever applies.
    for patch in node_patches {
        if let Patch::InsertBefore { new_tree, .. } = patch {
            emit_subtree(new_tree, builder, orig, patch_map, deleted, visited);
        }
    }

    if deleted.contains(&node_id) {
        // Multiple `Replace` patches on the same anchor are last-wins: each
        // one expresses "the new shape of this node," so the latest supersedes
        // earlier ones. The HAST `setProperty` flow for MDX JSX elements
        // relies on this — each prop set produces a fresh `replaceNode` call
        // carrying the accumulated attributes, and we want the final one.
        let replacement = node_patches.iter().rev().find_map(|p| match p {
            Patch::Replace {
                new_tree,
                keep_children,
                ..
            } => Some((new_tree, *keep_children)),
            _ => None,
        });
        if let Some((new_tree, keep_children)) = replacement {
            if keep_children {
                emit_subtree_with_original_children(
                    new_tree, node_id, orig, builder, patch_map, deleted, visited,
                );
            } else {
                emit_subtree(new_tree, builder, orig, patch_map, deleted, visited);
            }
        }
        // Post-siblings still apply for Remove and Replace.
        for patch in node_patches {
            if let Patch::InsertAfter { new_tree, .. } = patch {
                emit_subtree(new_tree, builder, orig, patch_map, deleted, visited);
            }
        }
        return true;
    }

    // Wrap: parent_tree's root becomes the wrapper; the original node becomes
    // its only child. Any existing children in parent_tree are ignored (Phase 6
    // simplification). Multiple wraps on the same anchor are last-wins for
    // the same reason as Replace.
    let wrap_tree = node_patches.iter().rev().find_map(|p| match p {
        Patch::Wrap { parent_tree, .. } => Some(parent_tree),
        _ => None,
    });
    if let Some(parent_tree) = wrap_tree {
        emit_wrap_node(
            parent_tree,
            node_id,
            orig,
            builder,
            patch_map,
            deleted,
            visited,
        );
        for patch in node_patches {
            if let Patch::InsertAfter { new_tree, .. } = patch {
                emit_subtree(new_tree, builder, orig, patch_map, deleted, visited);
            }
        }
        return true;
    }

    let node = orig.get_node(node_id);

    let new_id = builder.open_node_raw(node.node_type);

    // Copy node_data if present
    if let Some(data) = orig.get_node_data(node_id) {
        builder.arena_mut().set_node_data(new_id, data.to_vec());
    }

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

    for patch in node_patches {
        if let Patch::PrependChild { child_tree, .. } = patch {
            emit_subtree(child_tree, builder, orig, patch_map, deleted, visited);
        }
    }

    let child_ids: Vec<u32> = orig.get_children(node_id).to_vec();
    for child_id in child_ids {
        copy_node(child_id, orig, builder, patch_map, deleted, visited);
    }

    for patch in node_patches {
        if let Patch::AppendChild { child_tree, .. } = patch {
            emit_subtree(child_tree, builder, orig, patch_map, deleted, visited);
        }
    }

    builder.close_node();

    for patch in node_patches {
        if let Patch::InsertAfter { new_tree, .. } = patch {
            emit_subtree(new_tree, builder, orig, patch_map, deleted, visited);
        }
    }

    true
}

/// Sub-arena source is appended to the builder's source, and StringRef
/// offsets in type_data are remapped into the merged buffer.
fn emit_subtree<K: ArenaKind>(
    sub_arena: &Arena<K>,
    builder: &mut ArenaBuilder<K>,
    orig: &Arena<K>,
    patch_map: &FxHashMap<u32, Vec<&Patch<K>>>,
    deleted: &FxHashSet<u32>,
    visited: &mut FxHashSet<u32>,
) {
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

    // Raw markdown / HTML returns are parsed into a full document with a Root
    // at node 0. Splice the Root's children into the slot rather than the
    // wrapper itself; 0, 1, or N children all behave (none → the slot is
    // removed). Structured-node returns have a real node at 0, not a Root, so
    // they skip this and emit unchanged.
    if sub_arena.get_node(0).node_type == K::ROOT_TAG {
        for child in sub_arena.get_children(0).to_vec() {
            emit_subtree_node(
                child,
                sub_arena,
                builder,
                source_base,
                orig,
                patch_map,
                deleted,
                visited,
            );
        }
    } else {
        emit_subtree_node(
            0,
            sub_arena,
            builder,
            source_base,
            orig,
            patch_map,
            deleted,
            visited,
        );
    }
}

/// `source_base` is the offset added to StringRef offsets to remap them
/// into the merged source buffer.
// Threads the same rebuild state (orig/patch_map/deleted/visited) the other
// emit helpers carry, so a ref node can resolve against it via `copy_node`.
#[allow(clippy::too_many_arguments)]
fn emit_subtree_node<K: ArenaKind>(
    node_id: u32,
    sub_arena: &Arena<K>,
    builder: &mut ArenaBuilder<K>,
    source_base: u32,
    orig: &Arena<K>,
    patch_map: &FxHashMap<u32, Vec<&Patch<K>>>,
    deleted: &FxHashSet<u32>,
    visited: &mut FxHashSet<u32>,
) {
    let node = sub_arena.get_node(node_id);

    // A reference node: splice the original subtree it names, applying any
    // pending patch on it (so a nested transform on a passed-through child
    // runs in the same pass instead of stranding).
    if node.node_type == REF_NODE_TYPE {
        let td = sub_arena.get_type_data(node_id);
        let target = u32::from_le_bytes([td[0], td[1], td[2], td[3]]);
        copy_node(target, orig, builder, patch_map, deleted, visited);
        return;
    }

    let new_id = builder.open_node_raw(node.node_type);

    if let Some(data) = sub_arena.get_node_data(node_id) {
        builder.arena_mut().set_node_data(new_id, data.to_vec());
    }

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
            remap_string_refs::<K>(&mut remapped, node.node_type, source_base);
            builder.set_data_current(&remapped);
        } else {
            builder.set_data_current(type_data);
        }
    }

    let child_ids: Vec<u32> = sub_arena.get_children(node_id).to_vec();
    for child_id in child_ids {
        emit_subtree_node(
            child_id,
            sub_arena,
            builder,
            source_base,
            orig,
            patch_map,
            deleted,
            visited,
        );
    }

    builder.close_node();
}

/// Emit the replacement node's root (type + data) but use the original node's children.
fn emit_subtree_with_original_children<K: ArenaKind>(
    sub_arena: &Arena<K>,
    orig_node_id: u32,
    orig: &Arena<K>,
    builder: &mut ArenaBuilder<K>,
    patch_map: &FxHashMap<u32, Vec<&Patch<K>>>,
    deleted: &FxHashSet<u32>,
    visited: &mut FxHashSet<u32>,
) {
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

    // Emit the replacement root node's type and data
    let node = sub_arena.get_node(0);
    let new_id = builder.open_node_raw(node.node_type);

    if let Some(data) = sub_arena.get_node_data(0) {
        builder.arena_mut().set_node_data(new_id, data.to_vec());
    }

    let type_data = sub_arena.get_type_data(0);
    if !type_data.is_empty() {
        if source_base != 0 {
            let mut remapped = type_data.to_vec();
            remap_string_refs::<K>(&mut remapped, node.node_type, source_base);
            builder.set_data_current(&remapped);
        } else {
            builder.set_data_current(type_data);
        }
    }

    // Copy children from the original node
    let child_ids: Vec<u32> = orig.get_children(orig_node_id).to_vec();
    for child_id in child_ids {
        copy_node(child_id, orig, builder, patch_map, deleted, visited);
    }

    builder.close_node();
}

/// Add `base` to all StringRef offset fields in type_data.
/// StringRefs are `(offset: u32 LE, len: u32 LE)` pairs at known positions
/// depending on the node type.
///
/// MDAST and HAST share many numeric `node_type` values (e.g. MDAST `List` and
/// HAST `Raw` both = 5). Dispatch on `K::KIND_TAG` first so each schema's
/// layout is interpreted independently — applying HAST's "StringRef at 0"
/// rule to an MDAST `List` would corrupt the `start: u32` field stored there.
fn remap_string_refs<K: ArenaKind>(data: &mut [u8], node_type: u8, base: u32) {
    if K::KIND_TAG == Mdast::KIND_TAG {
        remap_mdast_string_refs(data, node_type, base);
    } else if K::KIND_TAG == Hast::KIND_TAG {
        remap_hast_string_refs(data, node_type, base);
    }
}

/// MDAST type_data layouts. Node-type IDs match `MdastNodeType`.
fn remap_mdast_string_refs(data: &mut [u8], node_type: u8, base: u32) {
    // Variable-length layouts: handle and return before the fixed-offset table.
    match node_type {
        // MdxJsxFlowElement(100), MdxJsxTextElement(101): name(0..8), attr_count(8..12),
        // then each attr at 16+i*20: kind(0..4), name(4..12), value(12..20).
        100 | 101 if data.len() >= 16 => {
            remap_one_ref(data, 0, base);
            let attr_count = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
            for i in 0..attr_count {
                let attr_base = 16 + i * 20;
                remap_one_ref(data, attr_base + 4, base); // name
                remap_one_ref(data, attr_base + 12, base); // value
            }
            return;
        }
        // ContainerDirective(30), LeafDirective(31), TextDirective(32):
        // name(0..8), attr_count(8..12), then each attr at 16+i*16: key(0..8), value(8..16).
        30..=32 if data.len() >= 16 => {
            remap_one_ref(data, 0, base);
            let attr_count = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
            for i in 0..attr_count {
                let attr_base = 16 + i * 16;
                remap_one_ref(data, attr_base, base); // key
                remap_one_ref(data, attr_base + 8, base); // value
            }
            return;
        }
        _ => {}
    }

    let ref_offsets: &[usize] = match node_type {
        // Html(7), Text(10), InlineCode(13), Yaml(25), Toml(26), InlineMath(28): single StringRef at 0
        7 | 10 | 13 | 25 | 26 | 28 => &[0],
        // Code(8): lang(0), meta(8), value(16)
        8 => &[0, 8, 16],
        // Definition(9): url(0), title(8), identifier(16), label(24)
        9 => &[0, 8, 16, 24],
        // Link(15): url(0), title(8)
        15 => &[0, 8],
        // Image(16): url(0), alt(8), title(16)
        16 => &[0, 8, 16],
        // LinkReference(17), FootnoteReference(20): identifier(0), label(8)
        17 | 20 => &[0, 8],
        // ImageReference(18): identifier(0), label(8), then 4-byte
        // (kind + _pad) header at 16..20, then alt(20..28).
        18 => &[0, 8, 20],
        // FootnoteDefinition(19): identifier(0), label(8)
        19 => &[0, 8],
        // Math(27): meta(0), value(8)
        27 => &[0, 8],
        // MdxFlowExpression(102), MdxTextExpression(103), MdxjsEsm(104): value(0)
        102..=104 => &[0],
        // List(5) carries `start: u32` at offset 0 — NOT a StringRef. Heading(2)
        // carries `depth: u8` only. ListItem(6), Table(21) and the rest have no
        // StringRef fields. Don't remap.
        _ => &[],
    };

    for &off in ref_offsets {
        remap_one_ref(data, off, base);
    }
}

/// HAST type_data layouts. Node-type IDs match `HastNodeType`.
fn remap_hast_string_refs(data: &mut [u8], node_type: u8, base: u32) {
    // Variable-length layouts: handle and return before the fixed-offset table.
    match node_type {
        // Element(1): tag(0..8), prop_count(8..12), then each prop at 16+i*20:
        // name(0..8), kind(8..12), value(12..20).
        1 if data.len() >= 12 => {
            remap_one_ref(data, 0, base);
            let prop_count = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
            for i in 0..prop_count {
                let prop_base = 16 + i * 20;
                remap_one_ref(data, prop_base, base); // name
                remap_one_ref(data, prop_base + 12, base); // value
            }
            return;
        }
        // MdxJsxElement(10), MdxJsxTextElement(11): same shape as MDAST MDX JSX.
        10 | 11 if data.len() >= 16 => {
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

    let ref_offsets: &[usize] = match node_type {
        // Text(2), Comment(3), Raw(5), MdxFlowExpression(12), MdxEsm(13),
        // MdxTextExpression(14): single StringRef at 0.
        2 | 3 | 5 | 12 | 13 | 14 => &[0],
        // Root(0), Doctype(4) and the rest have no StringRef fields.
        _ => &[],
    };

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
/// already present in parent_tree are ignored, the original node becomes
/// the sole child (Phase 6 simplification).
fn emit_wrap_node<K: ArenaKind>(
    parent_tree: &Arena<K>,
    original_node_id: u32,
    orig: &Arena<K>,
    builder: &mut ArenaBuilder<K>,
    patch_map: &FxHashMap<u32, Vec<&Patch<K>>>,
    deleted: &FxHashSet<u32>,
    visited: &mut FxHashSet<u32>,
) {
    if parent_tree.is_empty() {
        // Degenerate: no wrapper, just emit original
        copy_node(original_node_id, orig, builder, patch_map, deleted, visited);
        return;
    }

    // Remap string refs from the wrapper arena into the builder's merged source
    let sub_source = parent_tree.source();
    let source_base = if sub_source.is_empty() {
        0u32
    } else {
        let sref = builder.alloc_string(sub_source);
        sref.offset
    };

    let wrapper = parent_tree.get_node(0);

    let new_id = builder.open_node_raw(wrapper.node_type);

    if let Some(data) = parent_tree.get_node_data(0) {
        builder.arena_mut().set_node_data(new_id, data.to_vec());
    }

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
        if source_base != 0 {
            let mut remapped = wrapper_data.to_vec();
            remap_string_refs::<K>(&mut remapped, wrapper.node_type, source_base);
            builder.set_data_current(&remapped);
        } else {
            builder.set_data_current(wrapper_data);
        }
    }

    // Emit the original node as the child, copy it directly without
    // consulting the patch map (to avoid infinite recursion back into Wrap).
    copy_node_raw(original_node_id, orig, builder, patch_map, deleted, visited);

    builder.close_node();
}

/// Copy a single node and its children without checking the patch map
/// for the node itself (only children are patched). Used by wrap to
/// avoid re-entering the Wrap branch.
fn copy_node_raw<K: ArenaKind>(
    node_id: u32,
    orig: &Arena<K>,
    builder: &mut ArenaBuilder<K>,
    patch_map: &FxHashMap<u32, Vec<&Patch<K>>>,
    deleted: &FxHashSet<u32>,
    visited: &mut FxHashSet<u32>,
) {
    let node = orig.get_node(node_id);
    let new_id = builder.open_node_raw(node.node_type);

    if let Some(data) = orig.get_node_data(node_id) {
        builder.arena_mut().set_node_data(new_id, data.to_vec());
    }

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

    // Children are copied normally (patches on children still apply)
    let child_ids: Vec<u32> = orig.get_children(node_id).to_vec();
    for child_id in child_ids {
        copy_node(child_id, orig, builder, patch_map, deleted, visited);
    }

    builder.close_node();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mdast::MdastNodeType;
    use satteri_arena::{ArenaBuilder, Hast, Mdast};

    /// Build the "# Hello\n\nWorld" arena for testing.
    fn build_hello_world() -> Arena<Mdast> {
        use crate::mdast::codec::{encode_heading_data, encode_string_ref_data};
        use satteri_arena::StringRef;

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
        b.close_node(); // text

        b.close_node(); // heading

        b.open_node(MdastNodeType::Paragraph as u8);
        b.set_position_current(9, 14, 2, 1, 2, 6);

        b.open_node(MdastNodeType::Text as u8);
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
        let rebuilt = rebuild(&orig, &[]).expect("rebuild failed");
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
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

        // We should have 4 nodes: Root, Heading (now empty), Paragraph, Text(World)
        assert_eq!(rebuilt.len(), 4, "text under heading should be removed");

        // Heading child in rebuilt arena, find heading
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
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

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
        let mut replacement_builder = ArenaBuilder::<Mdast>::new(orig.source().to_string());
        replacement_builder.open_node(MdastNodeType::ThematicBreak as u8);
        replacement_builder.close_node();
        let replacement = replacement_builder.finish();

        let patches = vec![Patch::Replace {
            node_id: text_id,
            new_tree: replacement,
            keep_children: false,
        }];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

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
        let mut replacement_builder = ArenaBuilder::<Mdast>::new(orig.source().to_string());
        replacement_builder.open_node(MdastNodeType::Paragraph as u8);
        replacement_builder.close_node();
        let replacement = replacement_builder.finish();

        let patches = vec![Patch::Replace {
            node_id: heading_id,
            new_tree: replacement,
            keep_children: false,
        }];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

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
        let mut new_tree_builder = ArenaBuilder::<Mdast>::new(orig.source().to_string());
        new_tree_builder.open_node(MdastNodeType::ThematicBreak as u8);
        new_tree_builder.close_node();
        let new_tree = new_tree_builder.finish();

        let patches = vec![Patch::InsertBefore {
            node_id: para_id,
            new_tree,
        }];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

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

        let mut new_tree_builder = ArenaBuilder::<Mdast>::new(orig.source().to_string());
        new_tree_builder.open_node(MdastNodeType::ThematicBreak as u8);
        new_tree_builder.close_node();
        let new_tree = new_tree_builder.finish();

        let patches = vec![Patch::InsertAfter {
            node_id: heading_id,
            new_tree,
        }];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

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

        let mut child_builder = ArenaBuilder::<Mdast>::new(orig.source().to_string());
        child_builder.open_node(MdastNodeType::Break as u8);
        child_builder.close_node();
        let child_tree = child_builder.finish();

        let patches = vec![Patch::AppendChild {
            node_id: heading_id,
            child_tree,
        }];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

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

        let mut child_builder = ArenaBuilder::<Mdast>::new(orig.source().to_string());
        child_builder.open_node(MdastNodeType::Break as u8);
        child_builder.close_node();
        let child_tree = child_builder.finish();

        let patches = vec![Patch::PrependChild {
            node_id: heading_id,
            child_tree,
        }];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

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
        let mut new_tree_builder = ArenaBuilder::<Mdast>::new(orig.source().to_string());
        new_tree_builder.open_node(MdastNodeType::ThematicBreak as u8);
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
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

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

    #[test]
    fn wrap_hast_element() {
        // Build a minimal HAST arena: root(0) -> h1(1) -> text(2)
        use crate::hast::HastNodeType;
        use crate::mdast::codec::encode_string_ref_data;

        let mut b = ArenaBuilder::<Hast>::new(String::new());
        b.open_node_raw(HastNodeType::Root as u8);

        b.open_node_raw(HastNodeType::Element as u8);
        // Element type_data: tag_ref(0..8), prop_count(8..12), pad(12..16)
        let tag = b.alloc_string("h1");
        let mut td = vec![0u8; 16];
        td[0..4].copy_from_slice(&tag.offset.to_le_bytes());
        td[4..8].copy_from_slice(&tag.len.to_le_bytes());
        b.set_data_current(&td);

        b.open_node_raw(HastNodeType::Text as u8);
        let text = b.alloc_string("Hello");
        b.set_data_current(&encode_string_ref_data(text));
        b.close_node(); // text

        b.close_node(); // h1
        b.close_node(); // root
        let orig = b.finish();

        // Build wrapper: div element
        let mut wb = ArenaBuilder::<Hast>::new(String::new());
        wb.open_node_raw(HastNodeType::Element as u8);
        let div_tag = wb.alloc_string("div");
        let mut div_td = vec![0u8; 16];
        div_td[0..4].copy_from_slice(&div_tag.offset.to_le_bytes());
        div_td[4..8].copy_from_slice(&div_tag.len.to_le_bytes());
        wb.set_data_current(&div_td);
        wb.close_node();
        let wrapper = wb.finish();

        // Wrap node 1 (h1) with the div
        let patches = vec![Patch::Wrap {
            node_id: 1,
            parent_tree: wrapper,
        }];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

        // Should be: root -> div -> h1 -> text
        assert_eq!(rebuilt.len(), 4);
        let root_children = rebuilt.get_children(0);
        assert_eq!(root_children.len(), 1);
        let div_id = root_children[0];
        assert_eq!(
            rebuilt.get_node(div_id).node_type,
            HastNodeType::Element as u8
        );
        let div_children = rebuilt.get_children(div_id);
        assert_eq!(div_children.len(), 1);
        let h1_id = div_children[0];
        assert_eq!(
            rebuilt.get_node(h1_id).node_type,
            HastNodeType::Element as u8
        );
    }

    /// Build a single-node arena rooted at `node_type`, with no data and no
    /// children. Used to construct distinct sibling sub-trees for multi-patch
    /// tests.
    fn single_node_arena(node_type: MdastNodeType) -> Arena<Mdast> {
        let mut b = ArenaBuilder::<Mdast>::new(String::new());
        b.open_node(node_type as u8);
        b.close_node();
        b.finish()
    }

    /// Multiple `InsertBefore` patches against the same anchor must all be
    /// emitted, in the order they were issued (issuance order = buffer order).
    /// Regression: previously the patch map was keyed by node_id with a single
    /// `&Patch` value, so all but the last collided and were silently lost.
    #[test]
    fn multiple_insert_before_same_anchor_preserves_order() {
        let orig = build_hello_world();
        let para_id = orig.get_children(0)[1];

        let patches = vec![
            Patch::InsertBefore {
                node_id: para_id,
                new_tree: single_node_arena(MdastNodeType::ThematicBreak),
            },
            Patch::InsertBefore {
                node_id: para_id,
                new_tree: single_node_arena(MdastNodeType::Break),
            },
            Patch::InsertBefore {
                node_id: para_id,
                new_tree: single_node_arena(MdastNodeType::Blockquote),
            },
        ];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

        // Root: Heading, ThematicBreak, Break, Blockquote, Paragraph
        let root_children = rebuilt.get_children(0);
        assert_eq!(root_children.len(), 5);
        let types: Vec<u8> = root_children
            .iter()
            .map(|&id| rebuilt.get_node(id).node_type)
            .collect();
        assert_eq!(
            types,
            vec![
                MdastNodeType::Heading as u8,
                MdastNodeType::ThematicBreak as u8,
                MdastNodeType::Break as u8,
                MdastNodeType::Blockquote as u8,
                MdastNodeType::Paragraph as u8,
            ]
        );
    }

    /// Multiple `InsertAfter` patches against the same anchor: same contract,
    /// preserve buffer order.
    #[test]
    fn multiple_insert_after_same_anchor_preserves_order() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];

        let patches = vec![
            Patch::InsertAfter {
                node_id: heading_id,
                new_tree: single_node_arena(MdastNodeType::ThematicBreak),
            },
            Patch::InsertAfter {
                node_id: heading_id,
                new_tree: single_node_arena(MdastNodeType::Break),
            },
        ];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

        let root_children = rebuilt.get_children(0);
        assert_eq!(root_children.len(), 4);
        let types: Vec<u8> = root_children
            .iter()
            .map(|&id| rebuilt.get_node(id).node_type)
            .collect();
        assert_eq!(
            types,
            vec![
                MdastNodeType::Heading as u8,
                MdastNodeType::ThematicBreak as u8,
                MdastNodeType::Break as u8,
                MdastNodeType::Paragraph as u8,
            ]
        );
    }

    /// The asides-plugin flow: `insertBefore(anchor, opening)` × N for body
    /// children, `insertAfter(anchor, closing)`, then `removeNode(anchor)`.
    /// All sibling inserts must survive the remove on the same anchor.
    #[test]
    fn insert_before_after_and_remove_same_anchor() {
        let orig = build_hello_world();
        let para_id = orig.get_children(0)[1];

        let patches = vec![
            Patch::InsertBefore {
                node_id: para_id,
                new_tree: single_node_arena(MdastNodeType::ThematicBreak),
            },
            Patch::InsertBefore {
                node_id: para_id,
                new_tree: single_node_arena(MdastNodeType::Break),
            },
            Patch::InsertAfter {
                node_id: para_id,
                new_tree: single_node_arena(MdastNodeType::Blockquote),
            },
            Patch::Remove { node_id: para_id },
        ];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

        // Root should be: Heading, ThematicBreak, Break, Blockquote
        // (Paragraph removed, but the inserts around it stay.)
        let root_children = rebuilt.get_children(0);
        assert_eq!(root_children.len(), 4);
        let types: Vec<u8> = root_children
            .iter()
            .map(|&id| rebuilt.get_node(id).node_type)
            .collect();
        assert_eq!(
            types,
            vec![
                MdastNodeType::Heading as u8,
                MdastNodeType::ThematicBreak as u8,
                MdastNodeType::Break as u8,
                MdastNodeType::Blockquote as u8,
            ]
        );
    }

    /// `Replace` composes with sibling inserts on the same anchor: pre-insert
    /// emits, then the replacement emits in place of the original, then
    /// post-insert emits.
    #[test]
    fn replace_with_insert_before_and_after_same_anchor() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];

        let mut replacement = ArenaBuilder::<Mdast>::new(orig.source().to_string());
        replacement.open_node(MdastNodeType::Paragraph as u8);
        replacement.close_node();
        let replacement = replacement.finish();

        let patches = vec![
            Patch::InsertBefore {
                node_id: heading_id,
                new_tree: single_node_arena(MdastNodeType::ThematicBreak),
            },
            Patch::Replace {
                node_id: heading_id,
                new_tree: replacement,
                keep_children: false,
            },
            Patch::InsertAfter {
                node_id: heading_id,
                new_tree: single_node_arena(MdastNodeType::Break),
            },
        ];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

        // Root: ThematicBreak, Paragraph (was Heading), Break, Paragraph (orig)
        let root_children = rebuilt.get_children(0);
        assert_eq!(root_children.len(), 4);
        let types: Vec<u8> = root_children
            .iter()
            .map(|&id| rebuilt.get_node(id).node_type)
            .collect();
        assert_eq!(
            types,
            vec![
                MdastNodeType::ThematicBreak as u8,
                MdastNodeType::Paragraph as u8,
                MdastNodeType::Break as u8,
                MdastNodeType::Paragraph as u8,
            ]
        );
    }

    /// Multiple `Replace` patches on the same anchor: last-wins. The HAST
    /// `setProperty` path for MDX JSX elements emits a fresh `replaceNode`
    /// for every prop set, each carrying the accumulated attribute list — so
    /// the final replacement is the one with the full state.
    #[test]
    fn multiple_replace_same_anchor_last_wins() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];

        let mut first = ArenaBuilder::<Mdast>::new(orig.source().to_string());
        first.open_node(MdastNodeType::ThematicBreak as u8);
        first.close_node();
        let first = first.finish();

        let mut second = ArenaBuilder::<Mdast>::new(orig.source().to_string());
        second.open_node(MdastNodeType::Break as u8);
        second.close_node();
        let second = second.finish();

        let patches = vec![
            Patch::Replace {
                node_id: heading_id,
                new_tree: first,
                keep_children: false,
            },
            Patch::Replace {
                node_id: heading_id,
                new_tree: second,
                keep_children: false,
            },
        ];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

        let root_children = rebuilt.get_children(0);
        assert_eq!(root_children.len(), 2);
        assert_eq!(
            rebuilt.get_node(root_children[0]).node_type,
            MdastNodeType::Break as u8,
            "the second Replace should win"
        );
    }

    /// Multiple `PrependChild` and `AppendChild` patches on the same anchor
    /// also accumulate in buffer order, not collide.
    #[test]
    fn multiple_prepend_and_append_child_same_anchor() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];

        let patches = vec![
            Patch::PrependChild {
                node_id: heading_id,
                child_tree: single_node_arena(MdastNodeType::ThematicBreak),
            },
            Patch::PrependChild {
                node_id: heading_id,
                child_tree: single_node_arena(MdastNodeType::Break),
            },
            Patch::AppendChild {
                node_id: heading_id,
                child_tree: single_node_arena(MdastNodeType::Blockquote),
            },
            Patch::AppendChild {
                node_id: heading_id,
                child_tree: single_node_arena(MdastNodeType::Paragraph),
            },
        ];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

        // Heading children: ThematicBreak, Break, original Text, Blockquote, Paragraph
        let new_heading_id = rebuilt.get_children(0)[0];
        let heading_children = rebuilt.get_children(new_heading_id);
        let types: Vec<u8> = heading_children
            .iter()
            .map(|&id| rebuilt.get_node(id).node_type)
            .collect();
        assert_eq!(
            types,
            vec![
                MdastNodeType::ThematicBreak as u8,
                MdastNodeType::Break as u8,
                MdastNodeType::Text as u8,
                MdastNodeType::Blockquote as u8,
                MdastNodeType::Paragraph as u8,
            ]
        );
    }

    /// `wrapNode(N) + removeNode(N)` has no defined meaning — the node won't
    /// exist to wrap. Surface as an error rather than silently dropping the
    /// wrap.
    #[test]
    fn wrap_on_removed_node_errors() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];

        let patches = vec![
            Patch::Wrap {
                node_id: heading_id,
                parent_tree: single_node_arena(MdastNodeType::Blockquote),
            },
            Patch::Remove {
                node_id: heading_id,
            },
        ];
        match rebuild(&orig, &patches) {
            Err(CommandError::WrapOnRemovedNode(id)) => assert_eq!(id, heading_id),
            other => panic!("expected WrapOnRemovedNode, got {other:?}"),
        }
    }

    /// `prependChild(N, …) + removeNode(N)` has no inside to receive the
    /// child. Same for `appendChild`.
    #[test]
    fn prepend_child_on_removed_node_errors() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];

        let patches = vec![
            Patch::PrependChild {
                node_id: heading_id,
                child_tree: single_node_arena(MdastNodeType::Break),
            },
            Patch::Remove {
                node_id: heading_id,
            },
        ];
        match rebuild(&orig, &patches) {
            Err(CommandError::ChildPatchOnRemovedNode(id)) => assert_eq!(id, heading_id),
            other => panic!("expected ChildPatchOnRemovedNode, got {other:?}"),
        }
    }

    #[test]
    fn append_child_on_removed_node_errors() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];

        let patches = vec![
            Patch::Remove {
                node_id: heading_id,
            },
            Patch::AppendChild {
                node_id: heading_id,
                child_tree: single_node_arena(MdastNodeType::Break),
            },
        ];
        match rebuild(&orig, &patches) {
            Err(CommandError::ChildPatchOnRemovedNode(id)) => assert_eq!(id, heading_id),
            other => panic!("expected ChildPatchOnRemovedNode, got {other:?}"),
        }
    }

    /// Patching a descendant of a removed subtree: the descendant's anchor
    /// is never reached during the walk because we don't recurse into
    /// removed nodes. Caught post-walk as `PatchOnRemovedSubtree`.
    #[test]
    fn patch_on_descendant_of_removed_node_errors() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0]; // heading
        let text_in_heading = orig.get_children(heading_id)[0]; // text inside heading

        let patches = vec![
            Patch::Remove {
                node_id: heading_id,
            },
            Patch::InsertBefore {
                node_id: text_in_heading,
                new_tree: single_node_arena(MdastNodeType::Break),
            },
        ];
        match rebuild(&orig, &patches) {
            Err(CommandError::PatchOnRemovedSubtree(id)) => assert_eq!(id, text_in_heading),
            other => panic!("expected PatchOnRemovedSubtree, got {other:?}"),
        }
    }

    /// `rebuild_lenient` drops a patch stranded inside a removed/replaced
    /// subtree instead of erroring, and reports its anchor. The rest of the
    /// rebuild still applies.
    #[test]
    fn rebuild_lenient_drops_and_reports_stranded_patch() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];
        let text_in_heading = orig.get_children(heading_id)[0];

        // Replace the heading (dropping its subtree), and also replace the text
        // inside it — the kind of pair a nested-directive transform produces.
        let mut replacement = ArenaBuilder::<Mdast>::new(orig.source().to_string());
        replacement.open_node(MdastNodeType::Paragraph as u8);
        replacement.close_node();
        let replacement = replacement.finish();

        let patches = vec![
            Patch::Replace {
                node_id: heading_id,
                new_tree: replacement,
                keep_children: false,
            },
            Patch::Replace {
                node_id: text_in_heading,
                new_tree: single_node_arena(MdastNodeType::Break),
                keep_children: false,
            },
        ];
        let result = rebuild_lenient(&orig, &patches).expect("lenient rebuild should not error");
        assert_eq!(result.dropped, vec![text_in_heading]);
        // The heading replacement still applied: root's first child is the new Paragraph.
        let root_children = result.arena.get_children(0);
        assert_eq!(
            result.arena.get_node(root_children[0]).node_type,
            MdastNodeType::Paragraph as u8
        );
    }

    /// Same shape as the stranding test, but the replacement *references* the
    /// original child via a `REF_NODE_TYPE` node instead of discarding it. The
    /// child's own patch then applies (text → Break) and nothing strands — this
    /// is how a passed-through child keeps its identity so a nested transform
    /// queued on it runs in the same pass.
    #[test]
    fn ref_node_splices_original_and_applies_its_patch() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];
        let text_in_heading = orig.get_children(heading_id)[0];

        // Replacement: a Blockquote whose only child is a reference to the
        // heading's original text node.
        let mut replacement = ArenaBuilder::<Mdast>::new(String::new());
        replacement.open_node(MdastNodeType::Blockquote as u8);
        replacement.open_node_raw(REF_NODE_TYPE);
        replacement.set_data_current(&text_in_heading.to_le_bytes());
        replacement.close_node();
        replacement.close_node();
        let replacement = replacement.finish();

        let patches = vec![
            Patch::Replace {
                node_id: heading_id,
                new_tree: replacement,
                keep_children: false,
            },
            Patch::Replace {
                node_id: text_in_heading,
                new_tree: single_node_arena(MdastNodeType::Break),
                keep_children: false,
            },
        ];
        let result = rebuild_lenient(&orig, &patches).expect("lenient rebuild should not error");
        assert!(
            result.dropped.is_empty(),
            "the referenced child should not strand: {:?}",
            result.dropped
        );
        // root > blockquote > break (the referenced text, transformed in place).
        let bq = result.arena.get_children(0)[0];
        assert_eq!(
            result.arena.get_node(bq).node_type,
            MdastNodeType::Blockquote as u8
        );
        let bq_children = result.arena.get_children(bq);
        assert_eq!(bq_children.len(), 1);
        assert_eq!(
            result.arena.get_node(bq_children[0]).node_type,
            MdastNodeType::Break as u8
        );
    }

    /// Every patch stranded under a removed subtree is reported, not just the
    /// first, so strict `rebuild` can surface the complete set.
    #[test]
    fn rebuild_lenient_reports_every_stranded_anchor() {
        // Root(0) -> Heading(1) -> Text(2), Paragraph(3) -> Text(4)
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];
        let text_in_heading = orig.get_children(heading_id)[0];
        let para_id = orig.get_children(0)[1];
        let text_in_para = orig.get_children(para_id)[0];

        let patches = vec![
            // Remove both top-level nodes, stranding the text inside each.
            Patch::Remove {
                node_id: heading_id,
            },
            Patch::Remove { node_id: para_id },
            Patch::Replace {
                node_id: text_in_heading,
                new_tree: single_node_arena(MdastNodeType::Break),
                keep_children: false,
            },
            Patch::InsertBefore {
                node_id: text_in_para,
                new_tree: single_node_arena(MdastNodeType::Break),
            },
        ];
        let result = rebuild_lenient(&orig, &patches).expect("lenient rebuild should not error");
        assert_eq!(result.dropped, vec![text_in_heading, text_in_para]);
        // Both removals applied: the root is now empty.
        assert_eq!(result.arena.get_children(0).len(), 0);
    }

    /// Leniency only covers the "stranded inside a removed subtree" case. A
    /// `Wrap` (or child-add) on a node that is itself removed is unrecoverable
    /// misuse and still errors in `rebuild_lenient`, not just in `rebuild`.
    #[test]
    fn rebuild_lenient_still_errors_on_wrap_on_removed() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];

        let patches = vec![
            Patch::Wrap {
                node_id: heading_id,
                parent_tree: single_node_arena(MdastNodeType::Blockquote),
            },
            Patch::Remove {
                node_id: heading_id,
            },
        ];
        match rebuild_lenient(&orig, &patches) {
            Err(CommandError::WrapOnRemovedNode(id)) => assert_eq!(id, heading_id),
            Err(other) => panic!("expected WrapOnRemovedNode, got {other:?}"),
            Ok(_) => panic!("expected WrapOnRemovedNode error, got Ok"),
        }
    }

    /// `Replace { keep_children: true }` keeps the original children, so
    /// patches on those children should still apply (no error).
    #[test]
    fn patch_on_descendant_survives_replace_keep_children() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];
        let text_in_heading = orig.get_children(heading_id)[0];

        let mut replacement = ArenaBuilder::<Mdast>::new(orig.source().to_string());
        replacement.open_node(MdastNodeType::Paragraph as u8);
        replacement.close_node();
        let replacement = replacement.finish();

        let patches = vec![
            Patch::Replace {
                node_id: heading_id,
                new_tree: replacement,
                keep_children: true,
            },
            Patch::InsertBefore {
                node_id: text_in_heading,
                new_tree: single_node_arena(MdastNodeType::Break),
            },
        ];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild should succeed");
        // The new wrapper has Break + Text inside.
        let new_wrapper = rebuilt.get_children(0)[0];
        let inside = rebuilt.get_children(new_wrapper);
        let types: Vec<u8> = inside
            .iter()
            .map(|&id| rebuilt.get_node(id).node_type)
            .collect();
        assert_eq!(
            types,
            vec![MdastNodeType::Break as u8, MdastNodeType::Text as u8]
        );
    }
}
