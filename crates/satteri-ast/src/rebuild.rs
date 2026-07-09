//! Arena patching: apply structural patches to the arena in place.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::commands::CommandError;
use satteri_arena::{Arena, ArenaKind, Hast, Mdast};

/// Sentinel `node_type` for a *reference* node inside a replacement sub-tree:
/// "splice the existing original node whose id is stored in this node's
/// type_data (u32 LE) here." Higher than any real MDAST (≤104) or HAST (≤14)
/// type. Resolving it copies the original subtree *and applies any pending
/// patch on it*, so a nested transform queued on a passed-through child still
/// lands — no stranding, no re-visit.
pub const REF_NODE_TYPE: u8 = 0xFF;

#[derive(Debug, Clone)]
/// Payload of a structural patch: a free-standing tree copied in at apply
/// time, or subtrees already replayed into the target arena as orphans
/// (opstream payloads; strings live in the main pool, so no remap).
pub enum PatchContent<K: ArenaKind> {
    Tree(Arena<K>),
    Grafted(Vec<u32>),
}

impl<K: ArenaKind> From<Arena<K>> for PatchContent<K> {
    fn from(tree: Arena<K>) -> Self {
        PatchContent::Tree(tree)
    }
}

pub enum Patch<K: ArenaKind> {
    Replace {
        node_id: u32,
        new_tree: PatchContent<K>,
        keep_children: bool,
    },
    /// Removes the entire subtree rooted at this node
    Remove { node_id: u32 },
    /// Inserted as a preceding sibling
    InsertBefore {
        node_id: u32,
        new_tree: PatchContent<K>,
    },
    /// Inserted as a following sibling
    InsertAfter {
        node_id: u32,
        new_tree: PatchContent<K>,
    },
    /// The original node becomes a child of the new parent
    Wrap {
        node_id: u32,
        parent_tree: PatchContent<K>,
    },
    PrependChild {
        node_id: u32,
        child_tree: PatchContent<K>,
    },
    AppendChild {
        node_id: u32,
        child_tree: PatchContent<K>,
    },
    /// Replaces the node's child list with `new_children` (a Root-rooted
    /// sub-arena, spliced in), keeping the node itself — unlike `Replace`.
    SetChildren {
        node_id: u32,
        new_children: PatchContent<K>,
    },
}

/// Outcome of [`apply_patches_in_place`].
pub struct ApplyResult<K: ArenaKind> {
    pub arena: Arena<K>,
    /// Anchors whose patch landed inside a subtree that an ancestor's
    /// `Remove`/`Replace` genuinely discarded, so the patch could not be
    /// applied — and is moot, since the plugin chose to drop that subtree. A
    /// *passed-through* child is not dropped this way: it is spliced back by a
    /// `REF_NODE_TYPE` node (see [`REF_NODE_TYPE`]), keeping its id so a patch
    /// queued on it still applies.
    pub dropped: Vec<u32>,
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
/// Test-only alias so integration tests can build grafted payloads.
#[doc(hidden)]
pub fn remap_mdast_refs_for_test(data: &mut [u8], node_type: u8, base: u32) {
    remap_mdast_string_refs(data, node_type, base);
}

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
        // Html(7), Text(10), InlineCode(13), Yaml(25), Toml(26): single StringRef at 0
        7 | 10 | 13 | 25 | 26 => &[0],
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
        // Math(27), InlineMath(28): meta(0), value(8)
        27 | 28 => &[0, 8],
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

fn patch_anchor<K: ArenaKind>(patch: &Patch<K>) -> u32 {
    match patch {
        Patch::Replace { node_id, .. }
        | Patch::Remove { node_id }
        | Patch::InsertBefore { node_id, .. }
        | Patch::InsertAfter { node_id, .. }
        | Patch::Wrap { node_id, .. }
        | Patch::PrependChild { node_id, .. }
        | Patch::AppendChild { node_id, .. }
        | Patch::SetChildren { node_id, .. } => *node_id,
    }
}

fn patch_payload<K: ArenaKind>(patch: &Patch<K>) -> Option<&PatchContent<K>> {
    match patch {
        Patch::Replace { new_tree, .. }
        | Patch::InsertBefore { new_tree, .. }
        | Patch::InsertAfter { new_tree, .. } => Some(new_tree),
        Patch::Wrap { parent_tree, .. } => Some(parent_tree),
        Patch::PrependChild { child_tree, .. } | Patch::AppendChild { child_tree, .. } => {
            Some(child_tree)
        }
        Patch::SetChildren { new_children, .. } => Some(new_children),
        Patch::Remove { .. } => None,
    }
}

fn unsupported(reason: &'static str) -> CommandError {
    CommandError::UnsupportedPatchShape(reason)
}

/// Subtree copy by id; the append-only pool keeps type_data StringRefs valid verbatim.
fn copy_subtree<K: ArenaKind>(arena: &mut Arena<K>, id: u32) -> u32 {
    let node = *arena.get_node(id);
    let new_id = arena.alloc_node(node.node_type);
    if let Some(data) = arena.get_node_data(id).map(<[u8]>::to_vec) {
        arena.set_node_data(new_id, data);
    }
    arena.set_position(
        new_id,
        node.start_offset,
        node.end_offset,
        node.start_line,
        node.start_column,
        node.end_line,
        node.end_column,
    );
    let type_data = arena.get_type_data(id).to_vec();
    if !type_data.is_empty() {
        arena.set_type_data(new_id, &type_data);
    }
    let children = arena.get_children(id).to_vec();
    if !children.is_empty() {
        let ids: Vec<u32> = children.iter().map(|&c| copy_subtree(arena, c)).collect();
        arena.set_children(new_id, &ids);
    }
    new_id
}

/// `REF_NODE_TYPE` payload nodes expand to the pre-resolved id list for their position.
fn graft_node<K: ArenaKind>(
    arena: &mut Arena<K>,
    sub: &Arena<K>,
    sub_id: u32,
    source_base: u32,
    resolved_refs: &FxHashMap<u32, Vec<u32>>,
    out: &mut Vec<u32>,
) {
    let node = sub.get_node(sub_id);
    if node.node_type == REF_NODE_TYPE {
        out.extend_from_slice(&resolved_refs[&sub_id]);
        return;
    }
    let new_id = arena.alloc_node(node.node_type);
    if let Some(data) = sub.get_node_data(sub_id) {
        arena.set_node_data(new_id, data.to_vec());
    }
    arena.set_position(
        new_id,
        node.start_offset + source_base,
        node.end_offset + source_base,
        node.start_line,
        node.start_column,
        node.end_line,
        node.end_column,
    );
    let type_data = sub.get_type_data(sub_id);
    if !type_data.is_empty() {
        if source_base != 0 {
            let mut remapped = type_data.to_vec();
            remap_string_refs::<K>(&mut remapped, node.node_type, source_base);
            arena.set_type_data(new_id, &remapped);
        } else {
            arena.set_type_data(new_id, type_data);
        }
    }
    let sub_children = sub.get_children(sub_id).to_vec();
    if !sub_children.is_empty() {
        let mut ids: Vec<u32> = Vec::with_capacity(sub_children.len());
        for c in sub_children {
            graft_node(arena, sub, c, source_base, resolved_refs, &mut ids);
        }
        arena.set_children(new_id, &ids);
    }
    out.push(new_id);
}

/// Graft a payload sub-arena, returning the ids to splice into the slot.
/// Root-wrapped payloads contribute their root's children, mirroring
/// `emit_subtree`.
fn graft_subtree<K: ArenaKind>(
    arena: &mut Arena<K>,
    sub: &Arena<K>,
    resolved_refs: &FxHashMap<u32, Vec<u32>>,
) -> Vec<u32> {
    if sub.is_empty() {
        return Vec::new();
    }
    let sub_pool = sub.string_pool();
    let source_base = if sub_pool.is_empty() {
        0u32
    } else {
        arena.alloc_string(sub_pool).offset
    };
    let mut out = Vec::new();
    if sub.get_node(0).node_type == K::ROOT_TAG {
        for c in sub.get_children(0).to_vec() {
            graft_node(arena, sub, c, source_base, resolved_refs, &mut out);
        }
    } else {
        graft_node(arena, sub, 0, source_base, resolved_refs, &mut out);
    }
    out
}

/// Per-anchor plan derived from its patch group on the pristine tree.
struct AnchorPlan<'p, K: ArenaKind> {
    /// Remove/Replace present: the anchor node itself goes away.
    deleted: bool,
    /// Last-wins Replace, if any.
    winning_replace: Option<&'p Patch<K>>,
    /// Last-wins Wrap, if any (ignored when deleted, mirroring the rebuild).
    winning_wrap: Option<&'p Patch<K>>,
    /// Last-wins SetChildren, if any.
    winning_set_children: Option<&'p Patch<K>>,
    /// The anchor's patch group was inside a discarded subtree; nothing applies.
    dropped: bool,
}

/// Resolve one ref occurrence given the self-ref parity rules: nothing for
/// a removed anchor, a raw copy for any other self-ref, else the shared
/// adoption/copy logic.
#[allow(clippy::too_many_arguments)]
fn resolve_target<K: ArenaKind>(
    arena: &mut Arena<K>,
    target: u32,
    anchor: u32,
    self_removed: bool,
    slots: &FxHashMap<u32, Vec<u32>>,
    truly_dead: &FxHashSet<u32>,
    adopted_by_id: &mut FxHashSet<u32>,
) -> Vec<u32> {
    if target == anchor {
        if self_removed {
            return Vec::new();
        }
        return vec![copy_subtree(arena, anchor)];
    }
    if let Some(slot) = slots.get(&target) {
        if truly_dead.contains(&target) && adopted_by_id.insert(target) {
            return slot.clone();
        }
        let ids: Vec<u32> = slot.clone();
        return ids.iter().map(|&id| copy_subtree(arena, id)).collect();
    }
    if truly_dead.contains(&target) && adopted_by_id.insert(target) {
        return vec![target];
    }
    vec![copy_subtree(arena, target)]
}

/// Resolve a grafted payload's placeholder refs in place and return the final
/// slot ids (root-unwrapped unless `is_wrap`, whose node 0 stays verbatim).
#[allow(clippy::too_many_arguments)]
fn resolve_grafted<K: ArenaKind>(
    arena: &mut Arena<K>,
    roots: &[u32],
    placeholders: &[(u32, u32)],
    anchor: u32,
    self_removed: bool,
    slots: &FxHashMap<u32, Vec<u32>>,
    truly_dead: &FxHashSet<u32>,
    adopted_by_id: &mut FxHashSet<u32>,
    is_wrap: bool,
) -> Vec<u32> {
    for &(ph, target) in placeholders {
        if roots.contains(&ph) {
            continue;
        }
        let ids = resolve_target(
            arena,
            target,
            anchor,
            self_removed,
            slots,
            truly_dead,
            adopted_by_id,
        );
        let parent = arena.get_node(ph).parent;
        let current = arena.get_children(parent).to_vec();
        let mut new_list: Vec<u32> = Vec::with_capacity(current.len() + ids.len());
        for &c in &current {
            if c == ph {
                new_list.extend_from_slice(&ids);
            } else {
                new_list.push(c);
            }
        }
        arena.set_children(parent, &new_list);
    }
    let mut out: Vec<u32> = Vec::with_capacity(roots.len());
    for (i, &r) in roots.iter().enumerate() {
        if !(is_wrap && i == 0) && arena.get_node(r).node_type == REF_NODE_TYPE {
            let td = arena.get_type_data(r).to_vec();
            let target = u32::from_le_bytes([td[0], td[1], td[2], td[3]]);
            out.extend(resolve_target(
                arena,
                target,
                anchor,
                self_removed,
                slots,
                truly_dead,
                adopted_by_id,
            ));
        } else {
            out.push(r);
        }
    }
    if !is_wrap && out.len() == 1 && arena.get_node(out[0]).node_type == K::ROOT_TAG {
        return arena.get_children(out[0]).to_vec();
    }
    out
}

/// Apply structural patches by editing the arena in place; detached nodes
/// stay behind as unreachable garbage (every consumer traverses from root).
/// Anchors are processed in ref-dependency order so subtree copies always
/// observe post-patch state. Pathological re-entrant shapes (refs targeting
/// an anchor's own ancestors, ref-dependency cycles, discarding self-refs
/// over patched descendants) and Replace/Remove/Wrap/sibling-inserts on the
/// root error with [`CommandError::UnsupportedPatchShape`].
pub fn apply_patches_in_place<K: ArenaKind>(
    mut arena: Arena<K>,
    patches: &[Patch<K>],
) -> Result<ApplyResult<K>, CommandError> {
    let arena_len = arena.len() as u32;
    let mut patch_map: FxHashMap<u32, Vec<&Patch<K>>> = FxHashMap::default();
    let mut anchor_order: Vec<u32> = Vec::new();
    for patch in patches {
        let anchor = patch_anchor(patch);
        if anchor >= arena_len {
            return Err(unsupported("anchor out of bounds"));
        }
        if anchor == 0
            && matches!(
                patch,
                Patch::Replace { .. }
                    | Patch::Remove { .. }
                    | Patch::Wrap { .. }
                    | Patch::InsertBefore { .. }
                    | Patch::InsertAfter { .. }
            )
        {
            return Err(unsupported("non-child patch on root"));
        }
        if let std::collections::hash_map::Entry::Vacant(e) = patch_map.entry(anchor) {
            e.insert(Vec::new());
            anchor_order.push(anchor);
        }
        patch_map.get_mut(&anchor).unwrap().push(patch);
    }

    // Per-anchor plans; `discards` maps a patched ancestor to "its original
    // children are discarded" for the location walks below.
    let mut plans: FxHashMap<u32, AnchorPlan<'_, K>> = FxHashMap::default();
    let mut discards: FxHashMap<u32, bool> = FxHashMap::default();
    for (&anchor, group) in &patch_map {
        let mut removed = false;
        let mut winning_replace = None;
        let mut winning_wrap = None;
        let mut winning_set_children = None;
        for &p in group {
            match p {
                Patch::Remove { .. } => removed = true,
                Patch::Replace { .. } => winning_replace = Some(p),
                Patch::Wrap { .. } => winning_wrap = Some(p),
                Patch::SetChildren { .. } => winning_set_children = Some(p),
                _ => {}
            }
        }
        let deleted = removed || winning_replace.is_some();
        if deleted {
            let discard_children = match winning_replace {
                Some(Patch::Replace { keep_children, .. }) => !keep_children,
                _ => true,
            };
            discards.insert(anchor, discard_children);
        } else if winning_set_children.is_some() {
            discards.insert(anchor, true);
        }
        plans.insert(
            anchor,
            AnchorPlan {
                deleted,
                winning_replace,
                winning_wrap,
                winning_set_children,
                dropped: false,
            },
        );
    }

    // Same pre-flight errors as `rebuild`.
    for patch in patches {
        match patch {
            Patch::Wrap { node_id, .. } if plans[node_id].deleted => {
                return Err(CommandError::WrapOnRemovedNode(*node_id));
            }
            Patch::PrependChild { node_id, .. } | Patch::AppendChild { node_id, .. }
                if plans[node_id].deleted =>
            {
                return Err(CommandError::ChildPatchOnRemovedNode(*node_id));
            }
            _ => {}
        }
    }

    // Refs count only in payloads the rebuild would actually emit.
    let mut ref_uses: Vec<(u32, u32)> = Vec::new(); // (referring anchor, target)
    let mut ref_positions: FxHashMap<*const Patch<K>, Vec<(u32, u32)>> = FxHashMap::default(); // patch -> [(sub_id, target)]
    let mut ref_placeholders: FxHashMap<*const Patch<K>, Vec<(u32, u32)>> = FxHashMap::default(); // patch -> [(node_id, target)]
    let mut ref_targets: FxHashSet<u32> = FxHashSet::default();
    for (&anchor, plan) in &plans {
        for &p in &patch_map[&anchor] {
            let used = match p {
                Patch::Replace { .. } => plan.winning_replace.is_some_and(|w| std::ptr::eq(w, p)),
                Patch::Wrap { .. } => {
                    plan.winning_wrap.is_some_and(|w| std::ptr::eq(w, p)) && !plan.deleted
                }
                Patch::SetChildren { .. } => plan
                    .winning_set_children
                    .is_some_and(|w| std::ptr::eq(w, p)),
                _ => true,
            };
            if !used {
                continue;
            }
            let Some(content) = patch_payload(p) else {
                continue;
            };
            if let Patch::Replace {
                keep_children: true,
                ..
            } = p
            {
                match content {
                    PatchContent::Tree(sub) => {
                        if !sub.is_empty()
                            && (sub.get_node(0).node_type == REF_NODE_TYPE
                                || sub.get_node(0).node_type == K::ROOT_TAG)
                        {
                            return Err(unsupported("keep_children payload with ROOT/REF root"));
                        }
                    }
                    PatchContent::Grafted(_) => {
                        return Err(unsupported("grafted keep_children payload"));
                    }
                }
                // Payload children are never emitted for keep_children.
                continue;
            }
            // A REF at wrap position 0 is the wrapper itself, copied verbatim, never resolved.
            let is_wrap = matches!(p, Patch::Wrap { .. });
            match content {
                PatchContent::Tree(sub) => {
                    let scan_start = if is_wrap { 1 } else { 0 };
                    for sub_id in scan_start..sub.len() as u32 {
                        if sub.get_node(sub_id).node_type != REF_NODE_TYPE {
                            continue;
                        }
                        let td = sub.get_type_data(sub_id);
                        if td.len() < 4 {
                            return Err(unsupported("ref node with short type_data"));
                        }
                        let target = u32::from_le_bytes([td[0], td[1], td[2], td[3]]);
                        if target >= arena_len {
                            return Err(unsupported("ref target out of bounds"));
                        }
                        ref_uses.push((anchor, target));
                        ref_positions
                            .entry(p as *const _)
                            .or_default()
                            .push((sub_id, target));
                        ref_targets.insert(target);
                    }
                }
                PatchContent::Grafted(roots) => {
                    let mut stack: Vec<u32> = Vec::new();
                    for (i, &r) in roots.iter().enumerate() {
                        if is_wrap && i == 0 {
                            stack.extend_from_slice(arena.get_children(r));
                        } else {
                            stack.push(r);
                        }
                    }
                    while let Some(id) = stack.pop() {
                        if arena.get_node(id).node_type == REF_NODE_TYPE {
                            let td = arena.get_type_data(id);
                            if td.len() < 4 {
                                return Err(unsupported("ref node with short type_data"));
                            }
                            let target = u32::from_le_bytes([td[0], td[1], td[2], td[3]]);
                            if target >= arena_len {
                                return Err(unsupported("ref target out of bounds"));
                            }
                            ref_uses.push((anchor, target));
                            ref_placeholders
                                .entry(p as *const _)
                                .or_default()
                                .push((id, target));
                            ref_targets.insert(target);
                        } else {
                            stack.extend_from_slice(arena.get_children(id));
                        }
                    }
                }
            }
        }
    }

    // A node's fate follows its decider chain, nearest ancestor first: a ref
    // target rescues the region only if some LIVE anchor splices it, so
    // rescues cascade and must settle by fixpoint. With no refs there is
    // nothing to rescue or splice: the first discarding ancestor decides.
    let mut target_inner_patched: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
    let mut deciders: FxHashMap<u32, (Vec<u32>, bool)> = FxHashMap::default();
    let mut truly_dead: FxHashSet<u32> = FxHashSet::default();
    let mut dropped_set: FxHashSet<u32> = FxHashSet::default();
    if ref_uses.is_empty() {
        if !discards.is_empty() {
            for &anchor in &anchor_order {
                let mut cur = anchor;
                while cur != 0 {
                    let parent = arena.get_node(cur).parent;
                    if let Some(&true) = discards.get(&parent) {
                        dropped_set.insert(anchor);
                        break;
                    }
                    cur = parent;
                }
            }
        }
    } else {
        let ids_to_walk: Vec<u32> = anchor_order
            .iter()
            .copied()
            .chain(ref_targets.iter().copied())
            .collect();
        for id in ids_to_walk {
            if deciders.contains_key(&id) {
                continue;
            }
            let mut chain: Vec<u32> = Vec::new();
            if ref_targets.contains(&id) {
                chain.push(id);
            }
            let mut ends_in_discard = false;
            let mut cur = id;
            while cur != 0 {
                let parent = arena.get_node(cur).parent;
                if !ends_in_discard {
                    // A discarding ancestor cannot rescue its own children:
                    // its splice emits nothing below it.
                    if let Some(&true) = discards.get(&parent) {
                        ends_in_discard = true;
                    } else if ref_targets.contains(&parent) {
                        chain.push(parent);
                    }
                }
                if plans.contains_key(&id) && ref_targets.contains(&parent) {
                    target_inner_patched.entry(parent).or_default().push(id);
                }
                cur = parent;
            }
            deciders.insert(id, (chain, ends_in_discard));
        }
        let mut referrers: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
        for &(anchor, target) in &ref_uses {
            // An ancestor ref re-enters content mid-emission; only the rebuild's
            // active-set recursion guard reproduces that suppression.
            let mut cur = anchor;
            while cur != 0 {
                cur = arena.get_node(cur).parent;
                if cur == target {
                    return Err(unsupported("payload ref targets an ancestor of its anchor"));
                }
            }
            // A discarding self-ref raw-copies its subtree with descendant
            // patches applied mid-copy; that re-entry needs the rebuild.
            if target == anchor
                && discards.get(&anchor) == Some(&true)
                && target_inner_patched
                    .get(&anchor)
                    .is_some_and(|v| !v.is_empty())
            {
                return Err(unsupported("discarding self-ref over patched descendants"));
            }
            referrers.entry(target).or_default().push(anchor);
        }
        // Least fixpoint of liveness: unlinked anchors start dropped and are
        // revived only by an already-live referrer, so self- and mutual rescues
        // stay dropped, matching the rebuild's root walk.
        dropped_set = anchor_order
            .iter()
            .copied()
            .filter(|a| deciders[a].1)
            .collect();
        loop {
            let mut changed = false;
            let revived: Vec<u32> = dropped_set
                .iter()
                .copied()
                .filter(|a| {
                    deciders[a].0.iter().any(|t| {
                        referrers
                            .get(t)
                            .is_some_and(|rs| rs.iter().any(|r| !dropped_set.contains(r)))
                    })
                })
                .collect();
            for a in revived {
                dropped_set.remove(&a);
                changed = true;
            }
            if !changed {
                break;
            }
        }
        // A target may be adopted by id only when its old location is truly dead;
        // a live-spliced enclosing region would otherwise copy it a second time.
        for &target in &ref_targets {
            let (chain, ends_in_discard) = &deciders[&target];
            let live_spliced = |t: u32| {
                referrers
                    .get(&t)
                    .is_some_and(|rs| rs.iter().any(|r| !dropped_set.contains(r)))
            };
            if *ends_in_discard && !chain.iter().skip(1).any(|&t| live_spliced(t)) {
                truly_dead.insert(target);
            }
        }
    }
    for (&anchor, plan) in plans.iter_mut() {
        if dropped_set.contains(&anchor) {
            plan.dropped = true;
        }
    }

    // Referring anchors wait for the target's patches and patches inside it.
    let order: Vec<u32> = if ref_uses.is_empty() {
        anchor_order.clone()
    } else {
        let mut deps: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
        for &(anchor, target) in &ref_uses {
            let mut d: Vec<u32> = Vec::new();
            // A self-ref copies the anchor's pre-splice subtree; no edge.
            if plans.contains_key(&target) && target != anchor {
                d.push(target);
            }
            if let Some(inner) = target_inner_patched.get(&target) {
                d.extend(inner.iter().copied().filter(|&i| i != anchor));
            }
            deps.entry(anchor).or_default().extend(d);
        }
        let mut state: FxHashMap<u32, u8> = FxHashMap::default(); // 1=visiting, 2=done
        let mut out: Vec<u32> = Vec::with_capacity(anchor_order.len());
        let mut stack: Vec<(u32, usize)> = Vec::new();
        let mut cyclic = false;
        for &root in &anchor_order {
            if state.get(&root).copied().unwrap_or(0) == 2 {
                continue;
            }
            stack.push((root, 0));
            state.insert(root, 1);
            while let Some(&mut (node, ref mut idx)) = stack.last_mut() {
                let node_deps = deps.get(&node).map(Vec::as_slice).unwrap_or(&[]);
                if *idx < node_deps.len() {
                    let dep = node_deps[*idx];
                    *idx += 1;
                    match state.get(&dep).copied().unwrap_or(0) {
                        0 => {
                            state.insert(dep, 1);
                            stack.push((dep, 0));
                        }
                        1 => {
                            cyclic = true;
                            break;
                        }
                        _ => {}
                    }
                } else {
                    state.insert(node, 2);
                    out.push(node);
                    stack.pop();
                }
            }
            if cyclic {
                break;
            }
        }
        if cyclic {
            return Err(unsupported("ref-dependency cycle"));
        }
        out
    };

    // Each anchor splices immediately so later copies observe post-patch state.
    let mut slots: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
    let mut grafted: FxHashMap<*const Patch<K>, Vec<u32>> = FxHashMap::default();
    let mut wrap_resolved: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
    let mut adopted_by_id: FxHashSet<u32> = FxHashSet::default();
    let mut redirect: FxHashMap<u32, u32> = FxHashMap::default();

    for anchor in order {
        let plan = &plans[&anchor];
        if plan.dropped {
            continue;
        }
        let group: &[&Patch<K>] = &patch_map[&anchor];
        let winning_replace = plan.winning_replace.map(|p| p as *const Patch<K>);
        let winning_wrap = plan.winning_wrap.map(|p| p as *const Patch<K>);
        let winning_set_children = plan.winning_set_children.map(|p| p as *const Patch<K>);
        let deleted = plan.deleted;

        // Refs resolve before own-child edits: a self-ref must copy the raw subtree.
        grafted.clear();
        wrap_resolved.clear();
        for &p in group {
            let key = p as *const Patch<K>;
            let used = match p {
                Patch::Replace { .. } => winning_replace == Some(key),
                // Wrap payloads graft in the wiring below, at their final pool base.
                Patch::Wrap { .. } => false,
                Patch::SetChildren { .. } => winning_set_children == Some(key),
                Patch::Remove { .. } => false,
                _ => true,
            };
            if winning_wrap == Some(key) && !deleted {
                if let Some(positions) = ref_positions.get(&key) {
                    for &(sub_id, target) in positions {
                        let ids = resolve_target(
                            &mut arena,
                            target,
                            anchor,
                            false,
                            &slots,
                            &truly_dead,
                            &mut adopted_by_id,
                        );
                        wrap_resolved.insert(sub_id, ids);
                    }
                }
                if let (PatchContent::Grafted(roots), Some(placeholders)) =
                    (patch_payload(p).unwrap(), ref_placeholders.get(&key))
                {
                    resolve_grafted(
                        &mut arena,
                        roots,
                        placeholders,
                        anchor,
                        false,
                        &slots,
                        &truly_dead,
                        &mut adopted_by_id,
                        true,
                    );
                }
                continue;
            }
            if !used {
                continue;
            }
            if let Patch::Replace {
                new_tree: PatchContent::Tree(new_tree),
                keep_children: true,
                ..
            } = p
            {
                // Parity with the rebuild: only the payload root's type/data
                // land, position stays zeroed, payload children are ignored.
                let original_children = arena.get_children(anchor).to_vec();
                let sub_pool_base = if new_tree.string_pool().is_empty() {
                    0
                } else {
                    arena.alloc_string(new_tree.string_pool()).offset
                };
                let node = new_tree.get_node(0);
                let new_id = arena.alloc_node(node.node_type);
                if let Some(data) = new_tree.get_node_data(0) {
                    arena.set_node_data(new_id, data.to_vec());
                }
                let type_data = new_tree.get_type_data(0);
                if !type_data.is_empty() {
                    if sub_pool_base != 0 {
                        let mut remapped = type_data.to_vec();
                        remap_string_refs::<K>(&mut remapped, node.node_type, sub_pool_base);
                        arena.set_type_data(new_id, &remapped);
                    } else {
                        arena.set_type_data(new_id, type_data);
                    }
                }
                arena.set_children(new_id, &original_children);
                redirect.insert(anchor, new_id);
                grafted.insert(key, vec![new_id]);
                continue;
            }
            let Some(content) = patch_payload(p) else {
                continue;
            };
            let self_removed = deleted && winning_replace.is_none();
            match content {
                PatchContent::Tree(sub) => {
                    let mut resolved: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
                    if let Some(positions) = ref_positions.get(&key) {
                        for &(sub_id, target) in positions {
                            // Self-refs mirror the rebuild's active re-entry:
                            // nothing for a removed anchor, otherwise a raw
                            // copy; adoption would splice the anchor into its
                            // own graft.
                            let ids = resolve_target(
                                &mut arena,
                                target,
                                anchor,
                                self_removed,
                                &slots,
                                &truly_dead,
                                &mut adopted_by_id,
                            );
                            resolved.insert(sub_id, ids);
                        }
                    }
                    grafted.insert(key, graft_subtree(&mut arena, sub, &resolved));
                }
                PatchContent::Grafted(roots) => {
                    static EMPTY: &[(u32, u32)] = &[];
                    let placeholders = ref_placeholders
                        .get(&key)
                        .map(Vec::as_slice)
                        .unwrap_or(EMPTY);
                    let ids = resolve_grafted(
                        &mut arena,
                        roots,
                        placeholders,
                        anchor,
                        self_removed,
                        &slots,
                        &truly_dead,
                        &mut adopted_by_id,
                        false,
                    );
                    grafted.insert(key, ids);
                }
            }
        }

        // The rebuild's degenerate empty wrapper suppresses the anchor's own
        // child-list patches (`copy_node_raw(.., false)` on re-entry).
        let empty_wrap = matches!(
            plan.winning_wrap,
            Some(Patch::Wrap { parent_tree, .. }) if match parent_tree {
                PatchContent::Tree(t) => t.is_empty(),
                PatchContent::Grafted(roots) => roots.is_empty(),
            }
        ) && !deleted;

        // Own child-list edits (ignored on replaced anchors, like the rebuild).
        if !deleted && !empty_wrap {
            let has_own_edit = group.iter().any(|p| {
                matches!(
                    p,
                    Patch::PrependChild { .. }
                        | Patch::AppendChild { .. }
                        | Patch::SetChildren { .. }
                )
            });
            if has_own_edit {
                let mut new_list: Vec<u32> = Vec::new();
                for &p in group {
                    if let Patch::PrependChild { .. } = p {
                        new_list.extend_from_slice(&grafted[&(p as *const Patch<K>)]);
                    }
                }
                if let Some(key) = winning_set_children {
                    new_list.extend_from_slice(&grafted[&key]);
                } else {
                    new_list.extend_from_slice(arena.get_children(anchor));
                }
                for &p in group {
                    if let Patch::AppendChild { .. } = p {
                        new_list.extend_from_slice(&grafted[&(p as *const Patch<K>)]);
                    }
                }
                let target = redirect.get(&anchor).copied().unwrap_or(anchor);
                arena.set_children(target, &new_list);
            }
        }

        // Read before wrap wiring re-parents the anchor to its wrapper.
        let splice_parent = arena.get_node(anchor).parent;

        // Parity with emit_wrap_node: wrapper positions are NOT pool-rebased.
        let mut core: Vec<u32> = Vec::new();
        if deleted {
            if let Some(key) = winning_replace {
                core.extend_from_slice(&grafted[&key]);
            }
        } else if let (Some(_), false) = (winning_wrap, empty_wrap) {
            let Some(Patch::Wrap { parent_tree, .. }) = plan.winning_wrap else {
                unreachable!()
            };
            if let PatchContent::Grafted(roots) = parent_tree {
                // Grafted wrapper is already in the arena; adopt the anchor
                // as its first child (placeholders resolved in the graft loop).
                let wrapper_id = roots[0];
                let mut wrapper_children: Vec<u32> = vec![anchor];
                wrapper_children.extend_from_slice(arena.get_children(wrapper_id));
                arena.set_children(wrapper_id, &wrapper_children);
                core.push(wrapper_id);
            } else if let PatchContent::Tree(parent_tree) = parent_tree {
                let sub_pool = parent_tree.string_pool();
                let source_base = if sub_pool.is_empty() {
                    0u32
                } else {
                    arena.alloc_string(sub_pool).offset
                };
                let wrapper = parent_tree.get_node(0);
                let wrapper_id = arena.alloc_node(wrapper.node_type);
                if let Some(data) = parent_tree.get_node_data(0) {
                    arena.set_node_data(wrapper_id, data.to_vec());
                }
                arena.set_position(
                    wrapper_id,
                    wrapper.start_offset,
                    wrapper.end_offset,
                    wrapper.start_line,
                    wrapper.start_column,
                    wrapper.end_line,
                    wrapper.end_column,
                );
                let wrapper_data = parent_tree.get_type_data(0).to_vec();
                if !wrapper_data.is_empty() {
                    if source_base != 0 {
                        let mut remapped = wrapper_data;
                        remap_string_refs::<K>(&mut remapped, wrapper.node_type, source_base);
                        arena.set_type_data(wrapper_id, &remapped);
                    } else {
                        arena.set_type_data(wrapper_id, &wrapper_data);
                    }
                }
                let mut wrapper_children: Vec<u32> = vec![anchor];
                for c in parent_tree.get_children(0).to_vec() {
                    graft_node(
                        &mut arena,
                        parent_tree,
                        c,
                        source_base,
                        &wrap_resolved,
                        &mut wrapper_children,
                    );
                }
                arena.set_children(wrapper_id, &wrapper_children);
                core.push(wrapper_id);
            }
        } else {
            core.push(redirect.get(&anchor).copied().unwrap_or(anchor));
        }

        let mut slot: Vec<u32> = Vec::new();
        for &p in group {
            if let Patch::InsertBefore { .. } = p {
                slot.extend_from_slice(&grafted[&(p as *const Patch<K>)]);
            }
        }
        slot.extend_from_slice(&core);
        for &p in group {
            if let Patch::InsertAfter { .. } = p {
                slot.extend_from_slice(&grafted[&(p as *const Patch<K>)]);
            }
        }

        // Splice into the current parent list unless the slot is exactly the
        // anchor where it already stands.
        if anchor != 0 && slot.as_slice() != [anchor] {
            let parent = redirect
                .get(&splice_parent)
                .copied()
                .unwrap_or(splice_parent);
            let current = arena.get_children(parent).to_vec();
            let mut new_list: Vec<u32> = Vec::with_capacity(current.len() + slot.len());
            for &child in &current {
                if child == anchor {
                    new_list.extend_from_slice(&slot);
                } else {
                    new_list.push(child);
                }
            }
            arena.set_children(parent, &new_list);
        }
        if !ref_uses.is_empty() {
            slots.insert(anchor, slot);
        }
    }

    // `rebuild` never carries cp_offsets into the new arena; match it.
    arena.cp_offsets = Vec::new();

    let mut dropped: Vec<u32> = plans
        .iter()
        .filter(|(_, plan)| plan.dropped)
        .map(|(&a, _)| a)
        .collect();
    dropped.sort_unstable();

    Ok(ApplyResult { arena, dropped })
}

fn remap_one_ref(data: &mut [u8], off: usize, base: u32) {
    if off + 8 <= data.len() {
        let current = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
        let len = u32::from_le_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]);
        if len > 0 || current > 0 {
            let new_offset = current + base;
            data[off..off + 4].copy_from_slice(&new_offset.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mdast::MdastNodeType;
    use satteri_arena::{ArenaBuilder, Hast, Mdast};

    /// Strict shim matching the old `rebuild` contract: dropped -> error.
    fn rebuild<K: ArenaKind>(
        arena: &Arena<K>,
        patches: &[Patch<K>],
    ) -> Result<Arena<K>, CommandError> {
        let result = apply_patches_in_place(arena.clone(), patches)?;
        if let Some(&anchor) = result.dropped.first() {
            return Err(CommandError::PatchOnRemovedSubtree(anchor));
        }
        Ok(result.arena)
    }

    fn rebuild_lenient<K: ArenaKind>(
        arena: &Arena<K>,
        patches: &[Patch<K>],
    ) -> Result<ApplyResult<K>, CommandError> {
        apply_patches_in_place(arena.clone(), patches)
    }

    /// In-place apply leaves detached garbage; only root-reachable nodes count.
    fn reachable_count<K: ArenaKind>(arena: &Arena<K>) -> usize {
        fn walk<K: ArenaKind>(arena: &Arena<K>, id: u32) -> usize {
            1 + arena
                .get_children(id)
                .iter()
                .map(|&c| walk(arena, c))
                .sum::<usize>()
        }
        if arena.is_empty() {
            0
        } else {
            walk(arena, 0)
        }
    }

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
        assert_eq!(
            reachable_count(&rebuilt),
            orig.len(),
            "node count must be the same"
        );
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
        assert_eq!(
            reachable_count(&rebuilt),
            4,
            "text under heading should be removed"
        );

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
        assert_eq!(reachable_count(&rebuilt), 3);
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
        let mut replacement_builder = ArenaBuilder::<Mdast>::new(orig.string_pool().to_string());
        replacement_builder.open_node(MdastNodeType::ThematicBreak as u8);
        replacement_builder.close_node();
        let replacement = replacement_builder.finish();

        let patches = vec![Patch::Replace {
            node_id: text_id,
            new_tree: PatchContent::Tree(replacement),
            keep_children: false,
        }];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

        // Same node count (Text replaced by ThematicBreak, 1-for-1)
        assert_eq!(reachable_count(&rebuilt), orig.len());
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
        let mut replacement_builder = ArenaBuilder::<Mdast>::new(orig.string_pool().to_string());
        replacement_builder.open_node(MdastNodeType::Paragraph as u8);
        replacement_builder.close_node();
        let replacement = replacement_builder.finish();

        let patches = vec![Patch::Replace {
            node_id: heading_id,
            new_tree: PatchContent::Tree(replacement),
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
        let mut new_tree_builder = ArenaBuilder::<Mdast>::new(orig.string_pool().to_string());
        new_tree_builder.open_node(MdastNodeType::ThematicBreak as u8);
        new_tree_builder.close_node();
        let new_tree = new_tree_builder.finish();

        let patches = vec![Patch::InsertBefore {
            node_id: para_id,
            new_tree: PatchContent::Tree(new_tree),
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

        let mut new_tree_builder = ArenaBuilder::<Mdast>::new(orig.string_pool().to_string());
        new_tree_builder.open_node(MdastNodeType::ThematicBreak as u8);
        new_tree_builder.close_node();
        let new_tree = new_tree_builder.finish();

        let patches = vec![Patch::InsertAfter {
            node_id: heading_id,
            new_tree: PatchContent::Tree(new_tree),
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

        let mut child_builder = ArenaBuilder::<Mdast>::new(orig.string_pool().to_string());
        child_builder.open_node(MdastNodeType::Break as u8);
        child_builder.close_node();
        let child_tree = child_builder.finish();

        let patches = vec![Patch::AppendChild {
            node_id: heading_id,
            child_tree: PatchContent::Tree(child_tree),
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

        let mut child_builder = ArenaBuilder::<Mdast>::new(orig.string_pool().to_string());
        child_builder.open_node(MdastNodeType::Break as u8);
        child_builder.close_node();
        let child_tree = child_builder.finish();

        let patches = vec![Patch::PrependChild {
            node_id: heading_id,
            child_tree: PatchContent::Tree(child_tree),
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
    fn set_children_swaps_child_list_and_keeps_the_node() {
        use crate::mdast::codec::decode_heading_data;

        let mut orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];

        let data_offset = orig.get_node(heading_id).data_offset as usize;
        orig.type_data[data_offset] = 3;

        let mut children_builder = ArenaBuilder::<Mdast>::new(String::new());
        children_builder.open_node(MdastNodeType::Root as u8);
        children_builder.open_node(MdastNodeType::Break as u8);
        children_builder.close_node();
        children_builder.close_node();
        let new_children = children_builder.finish();

        let patches = vec![Patch::SetChildren {
            node_id: heading_id,
            new_children: PatchContent::Tree(new_children),
        }];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

        let new_heading_id = rebuilt.get_children(0)[0];
        assert_eq!(
            rebuilt.get_node(new_heading_id).node_type,
            MdastNodeType::Heading as u8
        );
        assert_eq!(
            decode_heading_data(rebuilt.get_type_data(new_heading_id)).depth,
            3
        );

        let heading_children = rebuilt.get_children(new_heading_id);
        assert_eq!(heading_children.len(), 1);
        assert_eq!(
            rebuilt.get_node(heading_children[0]).node_type,
            MdastNodeType::Break as u8
        );
    }

    #[test]
    fn multiple_patches_applied_together() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];
        let para_id = orig.get_children(0)[1];

        // Remove the heading AND insert a ThematicBreak after paragraph
        let mut new_tree_builder = ArenaBuilder::<Mdast>::new(orig.string_pool().to_string());
        new_tree_builder.open_node(MdastNodeType::ThematicBreak as u8);
        new_tree_builder.close_node();
        let new_tree = new_tree_builder.finish();

        let patches = vec![
            Patch::Remove {
                node_id: heading_id,
            },
            Patch::InsertAfter {
                node_id: para_id,
                new_tree: PatchContent::Tree(new_tree),
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
            parent_tree: PatchContent::Tree(wrapper),
        }];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

        // Should be: root -> div -> h1 -> text
        assert_eq!(reachable_count(&rebuilt), 4);
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
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::ThematicBreak)),
            },
            Patch::InsertBefore {
                node_id: para_id,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Break)),
            },
            Patch::InsertBefore {
                node_id: para_id,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Blockquote)),
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
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::ThematicBreak)),
            },
            Patch::InsertAfter {
                node_id: heading_id,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Break)),
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
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::ThematicBreak)),
            },
            Patch::InsertBefore {
                node_id: para_id,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Break)),
            },
            Patch::InsertAfter {
                node_id: para_id,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Blockquote)),
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

        let mut replacement = ArenaBuilder::<Mdast>::new(orig.string_pool().to_string());
        replacement.open_node(MdastNodeType::Paragraph as u8);
        replacement.close_node();
        let replacement = replacement.finish();

        let patches = vec![
            Patch::InsertBefore {
                node_id: heading_id,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::ThematicBreak)),
            },
            Patch::Replace {
                node_id: heading_id,
                new_tree: PatchContent::Tree(replacement),
                keep_children: false,
            },
            Patch::InsertAfter {
                node_id: heading_id,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Break)),
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

        let mut first = ArenaBuilder::<Mdast>::new(orig.string_pool().to_string());
        first.open_node(MdastNodeType::ThematicBreak as u8);
        first.close_node();
        let first = first.finish();

        let mut second = ArenaBuilder::<Mdast>::new(orig.string_pool().to_string());
        second.open_node(MdastNodeType::Break as u8);
        second.close_node();
        let second = second.finish();

        let patches = vec![
            Patch::Replace {
                node_id: heading_id,
                new_tree: PatchContent::Tree(first),
                keep_children: false,
            },
            Patch::Replace {
                node_id: heading_id,
                new_tree: PatchContent::Tree(second),
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
                child_tree: PatchContent::Tree(single_node_arena(MdastNodeType::ThematicBreak)),
            },
            Patch::PrependChild {
                node_id: heading_id,
                child_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Break)),
            },
            Patch::AppendChild {
                node_id: heading_id,
                child_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Blockquote)),
            },
            Patch::AppendChild {
                node_id: heading_id,
                child_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Paragraph)),
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
                parent_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Blockquote)),
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
                child_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Break)),
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
                child_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Break)),
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
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Break)),
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
        let mut replacement = ArenaBuilder::<Mdast>::new(orig.string_pool().to_string());
        replacement.open_node(MdastNodeType::Paragraph as u8);
        replacement.close_node();
        let replacement = replacement.finish();

        let patches = vec![
            Patch::Replace {
                node_id: heading_id,
                new_tree: PatchContent::Tree(replacement),
                keep_children: false,
            },
            Patch::Replace {
                node_id: text_in_heading,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Break)),
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
                new_tree: PatchContent::Tree(replacement),
                keep_children: false,
            },
            Patch::Replace {
                node_id: text_in_heading,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Break)),
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
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Break)),
                keep_children: false,
            },
            Patch::InsertBefore {
                node_id: text_in_para,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Break)),
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
                parent_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Blockquote)),
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

        let mut replacement = ArenaBuilder::<Mdast>::new(orig.string_pool().to_string());
        replacement.open_node(MdastNodeType::Paragraph as u8);
        replacement.close_node();
        let replacement = replacement.finish();

        let patches = vec![
            Patch::Replace {
                node_id: heading_id,
                new_tree: PatchContent::Tree(replacement),
                keep_children: true,
            },
            Patch::InsertBefore {
                node_id: text_in_heading,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Break)),
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

    /// Build a sub-arena holding a single `REF_NODE_TYPE` node targeting
    /// `target`, optionally wrapped in a parent node.
    fn ref_arena(target: u32, wrapper: Option<MdastNodeType>) -> Arena<Mdast> {
        let mut b = ArenaBuilder::<Mdast>::new(String::new());
        if let Some(w) = wrapper {
            b.open_node(w as u8);
        }
        b.open_node_raw(REF_NODE_TYPE);
        b.set_data_current(&target.to_le_bytes());
        b.close_node();
        if wrapper.is_some() {
            b.close_node();
        }
        b.finish()
    }

    /// A payload ref naming an ancestor of its own anchor re-enters content
    /// mid-emission; in-place application rejects it with a clear error.
    #[test]
    fn replace_with_ref_to_ancestor_errors() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];

        let patches = vec![Patch::Replace {
            node_id: heading_id,
            new_tree: PatchContent::Tree(ref_arena(0, Some(MdastNodeType::Paragraph))),
            keep_children: false,
        }];
        match rebuild(&orig, &patches) {
            Err(CommandError::UnsupportedPatchShape(_)) => {}
            other => panic!("expected UnsupportedPatchShape, got {other:?}"),
        }
    }

    /// Same rejection for sibling-insert payloads that ref an ancestor.
    #[test]
    fn insert_before_with_ref_to_ancestor_errors() {
        let orig = build_hello_world();
        let para_id = orig.get_children(0)[1];

        let patches = vec![Patch::InsertBefore {
            node_id: para_id,
            new_tree: PatchContent::Tree(ref_arena(0, None)),
        }];
        match rebuild(&orig, &patches) {
            Err(CommandError::UnsupportedPatchShape(_)) => {}
            other => panic!("expected UnsupportedPatchShape, got {other:?}"),
        }
    }
}
