//! Arena patching: apply structural patches to the arena in place.

use rustc_hash::{FxHashMap, FxHashSet};
use satteri_arena::{Arena, ArenaKind, Hast, Mdast, NodePosition};

use crate::commands::{CommandError, NodeLabel};
use crate::stack::with_headroom;

/// Sentinel `node_type` for a *reference* node inside a replacement sub-tree:
/// "splice the existing original node whose id is stored in this node's
/// type_data (u32 LE) here." Higher than any real MDAST (≤104) or HAST (≤14)
/// type. Resolving it moves the original node there, *with any pending patch
/// on it applied*, so a nested transform queued on a passed-through child still
/// lands (no stranding, no re-visit). A node can be named by one live ref only;
/// a second fails with [`CommandError::NodePlacedTwice`]. A self-ref, the anchor
/// named inside its own replacement, keeps the anchor itself there, so its own
/// edits and those inside it still apply.
pub const REF_NODE_TYPE: u8 = 0xFF;

/// Fifth byte of a ref node's type_data: the ref arranges its target in place, as
/// a plain ref in a `Replace` or set-children payload to a descendant of the
/// rebuilt node does, so a removal of the target empties it rather than winning
/// over a placement.
pub const REF_KIND_SLOT: u8 = 1;

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
    /// sub-arena, spliced in), keeping the node itself (unlike `Replace`).
    SetChildren {
        node_id: u32,
        new_children: PatchContent<K>,
    },
}

/// Add `base` to all StringRef offset fields in type_data.
/// StringRefs are `(offset: u32 LE, len: u32 LE)` pairs at known positions
/// depending on the node type.
///
/// MDAST and HAST share many numeric `node_type` values (e.g. MDAST `List` and
/// HAST `Raw` both = 5). Dispatch on `K::KIND_TAG` first so each schema's
/// layout is interpreted independently: applying HAST's "StringRef at 0"
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
        // Custom(38): name(0), value(8)
        38 => &[0, 8],
        // MdxFlowExpression(102), MdxTextExpression(103), MdxjsEsm(104): value(0)
        102..=104 => &[0],
        // List(5) carries `start: u32` at offset 0, NOT a StringRef. Heading(2)
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

/// `REF_NODE_TYPE` payload nodes expand to the pre-resolved id list for their position.
fn graft_node<K: ArenaKind>(
    arena: &mut Arena<K>,
    sub: &Arena<K>,
    sub_id: u32,
    source_base: u32,
    resolved_refs: &FxHashMap<u32, Vec<u32>>,
    out: &mut Vec<u32>,
    depth: u32,
) {
    with_headroom(depth, || {
        graft_node_inner(arena, sub, sub_id, source_base, resolved_refs, out, depth);
    });
}

fn graft_node_inner<K: ArenaKind>(
    arena: &mut Arena<K>,
    sub: &Arena<K>,
    sub_id: u32,
    source_base: u32,
    resolved_refs: &FxHashMap<u32, Vec<u32>>,
    out: &mut Vec<u32>,
    depth: u32,
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
    // Payload coordinates are fragment-relative, so the no-position sentinel stays.
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
            graft_node(
                arena,
                sub,
                c,
                source_base,
                resolved_refs,
                &mut ids,
                depth + 1,
            );
        }
        arena.set_children(new_id, &ids);
    }
    out.push(new_id);
}

/// Graft a payload sub-arena, returning the ids to splice into the slot.
/// Root-wrapped payloads contribute their root's children, mirroring
/// `emit_subtree`, unless `preserve_root` is set (root-anchored Replace),
/// which keeps the payload root as a node since it lands on node 0.
fn graft_subtree<K: ArenaKind>(
    arena: &mut Arena<K>,
    sub: &Arena<K>,
    resolved_refs: &FxHashMap<u32, Vec<u32>>,
    preserve_root: bool,
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
    if !preserve_root && sub.get_node(0).node_type == K::ROOT_TAG {
        for c in sub.get_children(0).to_vec() {
            graft_node(arena, sub, c, source_base, resolved_refs, &mut out, 0);
        }
    } else {
        graft_node(arena, sub, 0, source_base, resolved_refs, &mut out, 0);
    }
    out
}

/// Copy a free-standing tree with no ref nodes into `arena` as orphans, returning
/// the ids to splice; a root-wrapped tree contributes its root's children.
pub fn graft_tree<K: ArenaKind>(arena: &mut Arena<K>, tree: &Arena<K>) -> Vec<u32> {
    graft_subtree(arena, tree, &FxHashMap::default(), false)
}

/// Per-anchor plan derived from its patch group on the pristine tree.
/// Winning patches are stored as indices into the input `patches` slice.
struct AnchorPlan {
    /// Remove present: the anchor node goes away and no replacement takes its place.
    removed: bool,
    /// The anchor node itself leaves the tree: removed, or replaced by content that does not name it.
    gone: bool,
    /// Last-wins Replace, if any; never set on a removed anchor.
    winning_replace: Option<usize>,
    /// The replacement is one new node, which takes the anchor's own child edits.
    replaced_by_one: bool,
    /// Wrap present: each wraps whatever holds the anchor's place, in call order.
    wrapped: bool,
    /// Last-wins SetChildren, if any.
    winning_set_children: Option<usize>,
    /// The anchor's patch group was inside a discarded subtree; nothing applies.
    dropped: bool,
}

impl AnchorPlan {
    fn place_replaced(&self) -> bool {
        self.removed || self.winning_replace.is_some()
    }

    fn takes_child_edits(&self) -> bool {
        !self.gone || self.replaced_by_one
    }
}

fn names_node<K: ArenaKind>(arena: &Arena<K>, content: &PatchContent<K>, anchor: u32) -> bool {
    let is_ref_to_anchor = |node_type: u8, td: &[u8]| {
        node_type == REF_NODE_TYPE
            && td.len() >= 4
            && u32::from_le_bytes([td[0], td[1], td[2], td[3]]) == anchor
    };
    match content {
        PatchContent::Tree(sub) => (0..sub.len() as u32)
            .any(|id| is_ref_to_anchor(sub.get_node(id).node_type, sub.get_type_data(id))),
        PatchContent::Grafted(roots) => {
            let mut stack: Vec<u32> = roots.clone();
            while let Some(id) = stack.pop() {
                let node_type = arena.get_node(id).node_type;
                if is_ref_to_anchor(node_type, arena.get_type_data(id)) {
                    return true;
                }
                if node_type != REF_NODE_TYPE {
                    stack.extend_from_slice(arena.get_children(id));
                }
            }
            false
        }
    }
}

/// Where edits to a replaced node's own fields and children land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditTarget {
    /// The replacement names the node, which stays itself inside it.
    Itself,
    /// The one node the replacement resolves to: new, or an existing node moved there.
    Node(u32),
    /// Several nodes, raw content, or nothing.
    Nowhere,
}

/// What takes the edits on `anchor` once `content` replaces it.
pub fn replacement_edit_target<K: ArenaKind>(
    arena: &Arena<K>,
    anchor: u32,
    content: &PatchContent<K>,
) -> EditTarget {
    if anchor != 0 && names_node(arena, content, anchor) {
        return EditTarget::Itself;
    }
    let PatchContent::Grafted(roots) = content else {
        return EditTarget::Nowhere;
    };
    let only = match *roots.as_slice() {
        [root] if anchor != 0 && arena.get_node(root).node_type == K::ROOT_TAG => {
            match *arena.get_children(root) {
                [only] => only,
                _ => return EditTarget::Nowhere,
            }
        }
        [root] => root,
        _ => return EditTarget::Nowhere,
    };
    if arena.get_node(only).node_type != REF_NODE_TYPE {
        return EditTarget::Node(only);
    }
    match *arena.get_type_data(only) {
        [a, b, c, d, ..] => EditTarget::Node(u32::from_le_bytes([a, b, c, d])),
        _ => EditTarget::Nowhere,
    }
}

/// A root's replacement is one root-typed node landing on node 0, never a sequence.
fn is_one_new_node<K: ArenaKind>(arena: &Arena<K>, content: &PatchContent<K>, root: bool) -> bool {
    let is_new = |node_type: u8| node_type != REF_NODE_TYPE && (root || node_type != K::ROOT_TAG);
    match content {
        PatchContent::Tree(sub) => !sub.is_empty() && is_new(sub.get_node(0).node_type),
        PatchContent::Grafted(roots) => match *roots.as_slice() {
            [root_id] if !root && arena.get_node(root_id).node_type == K::ROOT_TAG => matches!(
                *arena.get_children(root_id),
                [only] if is_new(arena.get_node(only).node_type)
            ),
            [root_id] => is_new(arena.get_node(root_id).node_type),
            _ => false,
        },
    }
}

/// How the final id list of a grafted payload treats its top-level nodes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum GraftMode {
    /// Sibling-slot payload: a single ROOT-typed result contributes its
    /// children, mirroring `emit_subtree`.
    Splice,
    /// Wrap payload: node 0 is the wrapper itself, copied verbatim (a REF at
    /// position 0 is never resolved).
    Wrap,
    /// Root-anchored Replace: the payload root is kept as-is (even a ROOT)
    /// because it lands on node 0 instead of being spliced into a sibling
    /// list. REF roots still resolve.
    KeepRoot,
}

/// How a payload ref resolves.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RefKind {
    /// A descendant of the node being rebuilt, arranged in the rebuild unless placed elsewhere.
    Arranged,
    /// Any other existing node, moved here.
    Placed,
    /// Nothing: a removed node placed, or one placed elsewhere and so not arranged here.
    Gone,
}

/// The refs in the payloads that will be emitted, keyed by patch index.
#[derive(Default)]
struct RefScan {
    /// (referring anchor, target) for every live ref but a self-ref.
    uses: Vec<(u32, u32)>,
    /// Tree payloads: [(sub_id, target, kind)].
    positions: FxHashMap<usize, Vec<(u32, u32, RefKind)>>,
    /// Grafted payloads: [(node_id, target, kind)].
    placeholders: FxHashMap<usize, Vec<(u32, u32, RefKind)>>,
    targets: FxHashSet<u32>,
    gone: FxHashSet<u32>,
}

impl RefScan {
    fn record<K: ArenaKind>(
        &mut self,
        pi: usize,
        anchor: u32,
        content: &PatchContent<K>,
        refs: Vec<(u32, u32, RefKind)>,
    ) {
        if refs.is_empty() {
            return;
        }
        for &(_, target, kind) in &refs {
            if kind == RefKind::Gone {
                self.gone.insert(target);
            } else if target != anchor {
                self.uses.push((anchor, target));
                self.targets.insert(target);
            }
        }
        match content {
            PatchContent::Tree(_) => self.positions.insert(pi, refs),
            PatchContent::Grafted(_) => self.placeholders.insert(pi, refs),
        };
    }
}

/// Every ref in a payload with how it resolves; a wrap payload's own node is the wrapper, never a ref.
fn scan_refs<K: ArenaKind>(
    arena: &Arena<K>,
    content: &PatchContent<K>,
    is_wrap: bool,
    kind_of: impl Fn(u32, &[u8]) -> RefKind,
) -> Result<Vec<(u32, u32, RefKind)>, CommandError> {
    let arena_len = arena.len() as u32;
    let target_of = |td: &[u8]| {
        let &[a, b, c, d, ..] = td else {
            return Err(unsupported("ref node with short type_data"));
        };
        let target = u32::from_le_bytes([a, b, c, d]);
        if target >= arena_len {
            return Err(unsupported("ref target out of bounds"));
        }
        if target != 0 && arena.get_node(target).parent == u32::MAX {
            return Err(unsupported("detached ref target"));
        }
        Ok(target)
    };
    let mut refs = Vec::new();
    match content {
        PatchContent::Tree(sub) => {
            for sub_id in u32::from(is_wrap)..sub.len() as u32 {
                if sub.get_node(sub_id).node_type == REF_NODE_TYPE {
                    let td = sub.get_type_data(sub_id);
                    let target = target_of(td)?;
                    refs.push((sub_id, target, kind_of(target, td)));
                }
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
                    let target = target_of(td)?;
                    refs.push((id, target, kind_of(target, td)));
                } else {
                    stack.extend_from_slice(arena.get_children(id));
                }
            }
        }
    }
    Ok(refs)
}

/// A ref takes its target's whole position, edits spliced beside it included; a
/// self-ref is the anchor alone, since its own place is what the replacement fills.
fn resolve_target(
    target: u32,
    anchor: u32,
    position_slots: &FxHashMap<u32, Vec<u32>>,
    kind: RefKind,
) -> Vec<u32> {
    if kind == RefKind::Gone {
        return Vec::new();
    }
    if target == anchor {
        return vec![anchor];
    }
    position_slots
        .get(&target)
        .cloned()
        .unwrap_or_else(|| vec![target])
}

/// Resolve a grafted payload's placeholder refs in place and return the final
/// slot ids (see [`GraftMode`] for how the top-level nodes are treated).
fn resolve_grafted<K: ArenaKind>(
    arena: &mut Arena<K>,
    roots: &[u32],
    placeholders: &[(u32, u32, RefKind)],
    anchor: u32,
    position_slots: &FxHashMap<u32, Vec<u32>>,
    mode: GraftMode,
) -> Vec<u32> {
    let mut resolved_placeholders: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
    let mut placeholder_parents: Vec<u32> = Vec::new();
    let mut seen_parents: FxHashSet<u32> = FxHashSet::default();
    let mut top_level: FxHashMap<u32, (u32, RefKind)> = FxHashMap::default();
    for &(ph, target, kind) in placeholders {
        // Top-level placeholders have no parent and are resolved below. Checking
        // that directly avoids scanning every root for every placeholder.
        let parent = arena.get_node(ph).parent;
        if parent == u32::MAX {
            top_level.insert(ph, (target, kind));
            continue;
        }
        resolved_placeholders.insert(ph, resolve_target(target, anchor, position_slots, kind));
        if seen_parents.insert(parent) {
            placeholder_parents.push(parent);
        }
    }
    // Resolve every placeholder under a parent in one rewrite. Multi-node
    // replacements commonly put all refs under one synthetic root, so doing a
    // child-list rewrite per ref would itself be quadratic.
    for parent in placeholder_parents {
        let current = arena.get_children(parent).to_vec();
        let mut new_list: Vec<u32> = Vec::with_capacity(current.len());
        for &child in &current {
            if let Some(ids) = resolved_placeholders.get(&child) {
                new_list.extend_from_slice(ids);
            } else {
                new_list.push(child);
            }
        }
        arena.set_children(parent, &new_list);
    }
    let mut out: Vec<u32> = Vec::with_capacity(roots.len());
    for &r in roots {
        if let Some(&(target, kind)) = top_level.get(&r) {
            out.extend(resolve_target(target, anchor, position_slots, kind));
        } else {
            out.push(r);
        }
    }
    if mode == GraftMode::Splice
        && out.len() == 1
        && arena.get_node(out[0]).node_type == K::ROOT_TAG
    {
        return arena.get_children(out[0]).to_vec();
    }
    out
}

/// Materialize the pending grouped sibling splices for `parent`: walk its
/// current child list once, expanding each anchored slot in place. No-op if
/// nothing is pending for `parent`. Slots whose anchor is gone report in `stranded`.
fn flush_pending_splices<K: ArenaKind>(
    arena: &mut Arena<K>,
    pending: &mut FxHashMap<u32, FxHashMap<u32, Vec<u32>>>,
    parent: u32,
    stranded: &mut Vec<u32>,
) {
    let Some(slots) = pending.remove(&parent) else {
        return;
    };
    let current = arena.get_children(parent).to_vec();
    let grown: usize = slots.values().map(Vec::len).sum();
    let mut new_list: Vec<u32> = Vec::with_capacity(current.len() + grown);
    let mut spliced = 0usize;
    for &child in &current {
        match slots.get(&child) {
            Some(slot) => {
                new_list.extend_from_slice(slot);
                spliced += 1;
            }
            None => new_list.push(child),
        }
    }
    if spliced != slots.len() {
        // A removal whose node is already gone has nothing left to do.
        stranded.extend(
            slots
                .iter()
                .filter(|(anchor, slot)| !slot.is_empty() && !current.contains(anchor))
                .map(|(&anchor, _)| anchor),
        );
    }
    arena.set_children(parent, &new_list);
}

/// Move `src`'s shape onto node 0: type, type_data, position, node_data, and
/// children (re-parented to 0). `src` itself becomes unreachable garbage. The
/// in-place equivalent of "the replacement becomes the new root".
fn overwrite_root_with<K: ArenaKind>(arena: &mut Arena<K>, src: u32) {
    let src_node = *arena.get_node(src);
    let children = arena.get_children(src).to_vec();
    {
        let root = arena.get_node_mut(0);
        root.node_type = src_node.node_type;
        root.data_offset = src_node.data_offset;
        root.data_len = src_node.data_len;
        root.start_offset = src_node.start_offset;
        root.end_offset = src_node.end_offset;
        root.start_line = src_node.start_line;
        root.start_column = src_node.start_column;
        root.end_line = src_node.end_line;
        root.end_column = src_node.end_column;
    }
    arena.set_children(0, &children);
    match arena.node_data.remove(&src) {
        Some(data) => {
            arena.node_data.insert(0, data);
        }
        None => {
            arena.node_data.remove(&0);
        }
    }
}

/// Make node 0 an empty root (no children, no data, zeroed position) for a
/// `Remove` on the root. The node type is kept so the document still
/// serializes (as empty).
fn clear_root<K: ArenaKind>(arena: &mut Arena<K>) {
    {
        let root = arena.get_node_mut(0);
        root.data_offset = 0;
        root.data_len = 0;
        root.start_offset = 0;
        root.end_offset = 0;
        root.start_line = 0;
        root.start_column = 0;
        root.end_line = 0;
        root.end_column = 0;
    }
    arena.set_children(0, &[]);
    arena.node_data.remove(&0);
}

/// Shallow-copy node 0 into a fresh node: same shape, node_data moved over,
/// children re-parented under the copy. Used when a wrapper takes over the
/// root slot and the old root becomes its first child.
fn copy_root_shallow<K: ArenaKind>(arena: &mut Arena<K>) -> u32 {
    let root = *arena.get_node(0);
    let copy = arena.alloc_node(root.node_type);
    {
        let n = arena.get_node_mut(copy);
        n.data_offset = root.data_offset;
        n.data_len = root.data_len;
        n.start_offset = root.start_offset;
        n.end_offset = root.end_offset;
        n.start_line = root.start_line;
        n.start_column = root.start_column;
        n.end_line = root.end_line;
        n.end_column = root.end_column;
    }
    if let Some(data) = arena.node_data.remove(&0) {
        arena.node_data.insert(copy, data);
    }
    let children = arena.get_children(0).to_vec();
    arena.set_children(copy, &children);
    copy
}

/// Apply structural patches by editing the arena in place; detached nodes
/// stay behind as unreachable garbage (every consumer traverses from root).
/// Anchors are processed in ref-dependency order so a ref always takes its
/// target's post-patch state. Pathological re-entrant shapes (refs targeting
/// an anchor's own ancestors, ref-dependency cycles, discarding self-refs
/// over patched descendants) and sibling inserts on the root error with
/// [`CommandError::UnsupportedPatchShape`].
///
/// Root-anchored patches are first-class: `Replace` overwrites node 0 with
/// the replacement (keeping the root id), `Remove` empties the document,
/// `Wrap` moves the old root under the wrapper which takes over node 0, and
/// the child edits behave as on any other node.
///
/// Validate-then-mutate contract: every fallible check runs before the first
/// arena mutation, so on `Err` the arena is untouched.
///
/// Returns the anchors whose patch could not be applied: those inside a subtree
/// that an ancestor's `Remove`/`Replace` genuinely discarded (moot, since the
/// plugin chose to drop that subtree), those an earlier apply already
/// detached from their parent, whose sibling splice has nowhere to land. A
/// *passed-through* child is not dropped this way: it is spliced back by a
/// `REF_NODE_TYPE` node (see [`REF_NODE_TYPE`]), keeping its id so a patch
/// queued on it still applies.
pub fn apply_patches_in_place<K: ArenaKind>(
    arena: &mut Arena<K>,
    patches: &[Patch<K>],
) -> Result<Vec<u32>, CommandError> {
    apply_patches_impl(arena, patches, false)
}

/// Like [`apply_patches_in_place`], but a patch stranded inside a discarded
/// subtree is an error: fails with [`CommandError::PatchOnRemovedSubtree`]
/// (carrying the lowest stranded anchor) *before* the first mutation, so on
/// `Err` the arena is untouched.
pub fn apply_patches_strict<K: ArenaKind>(
    arena: &mut Arena<K>,
    patches: &[Patch<K>],
) -> Result<(), CommandError> {
    apply_patches_impl(arena, patches, true).map(|_| ())
}

fn apply_patches_impl<K: ArenaKind>(
    arena: &mut Arena<K>,
    patches: &[Patch<K>],
    strict: bool,
) -> Result<Vec<u32>, CommandError> {
    let arena_len = arena.len() as u32;
    let mut patch_map: FxHashMap<u32, Vec<usize>> = FxHashMap::default();
    let mut anchor_order: Vec<u32> = Vec::new();
    for (pi, patch) in patches.iter().enumerate() {
        let anchor = patch_anchor(patch);
        if anchor >= arena_len {
            return Err(unsupported("anchor out of bounds"));
        }
        // A never-attached node's u32::MAX parent would send the ancestor walks out of bounds
        if anchor != 0 && arena.get_node(anchor).parent == u32::MAX {
            return Err(unsupported("detached anchor"));
        }
        if anchor == 0
            && matches!(
                patch,
                Patch::InsertBefore { .. } | Patch::InsertAfter { .. }
            )
        {
            return Err(unsupported("sibling insert on root (root has no siblings)"));
        }
        if let std::collections::hash_map::Entry::Vacant(e) = patch_map.entry(anchor) {
            e.insert(Vec::new());
            anchor_order.push(anchor);
        }
        patch_map.get_mut(&anchor).unwrap().push(pi);
    }

    let mut plans: FxHashMap<u32, AnchorPlan> = FxHashMap::default();
    for (&anchor, group) in &patch_map {
        let mut removed = false;
        let mut winning_replace = None;
        let mut wrapped = false;
        let mut winning_set_children = None;
        for &pi in group {
            match &patches[pi] {
                Patch::Remove { .. } => removed = true,
                Patch::Replace { .. } => winning_replace = Some(pi),
                Patch::Wrap { .. } => wrapped = true,
                Patch::SetChildren { .. } => winning_set_children = Some(pi),
                _ => {}
            }
        }
        // A removal is final, so it beats any replacement of the same node.
        if removed {
            winning_replace = None;
        }
        let replaced_by_one = winning_replace
            .and_then(|pi| patch_payload(&patches[pi]))
            .is_some_and(|c| is_one_new_node(arena, c, anchor == 0));
        plans.insert(
            anchor,
            AnchorPlan {
                removed,
                gone: removed || winning_replace.is_some(),
                winning_replace,
                replaced_by_one,
                wrapped,
                winning_set_children,
                dropped: false,
            },
        );
    }

    // A ROOT-typed wrapper only arises from a parsed raw payload; treating it
    // as the wrapper silently degrades the wrap into sibling insertion.
    for patch in patches {
        if let Patch::Wrap { parent_tree, .. } = patch
            && match parent_tree {
                PatchContent::Tree(t) => !t.is_empty() && t.get_node(0).node_type == K::ROOT_TAG,
                PatchContent::Grafted(roots) => roots
                    .first()
                    .is_some_and(|&r| arena.get_node(r).node_type == K::ROOT_TAG),
            }
        {
            return Err(unsupported("wrap parent is a document root"));
        }
    }

    let is_inside = |target: u32, ancestor: u32| {
        let mut cur = target;
        while cur != 0 {
            cur = arena.get_node(cur).parent;
            if cur == ancestor {
                return true;
            }
        }
        false
    };
    let mut scan = RefScan::default();

    // Replacements first: one that names its anchor decides where that anchor's child edits land.
    let mut kept: Vec<u32> = Vec::new();
    for (&anchor, plan) in &plans {
        let Some(pi) = plan.winning_replace else {
            continue;
        };
        let p = &patches[pi];
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
                    if sub.is_empty() {
                        return Err(unsupported("empty keep_children payload"));
                    }
                    if sub.get_node(0).node_type == REF_NODE_TYPE
                        || sub.get_node(0).node_type == K::ROOT_TAG
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
        let mut refs = scan_refs(arena, content, false, |target, td| {
            if td.get(4) == Some(&REF_KIND_SLOT) || is_inside(target, anchor) {
                RefKind::Arranged
            } else if plans.get(&target).is_some_and(|plan| plan.removed) {
                RefKind::Gone
            } else {
                RefKind::Placed
            }
        })?;
        let names_itself = anchor != 0 && refs.iter().any(|&(_, target, _)| target == anchor);
        if names_itself {
            kept.push(anchor);
        } else if plan.replaced_by_one && plan.winning_set_children.is_some() {
            // A new children list for the node that replaced this anchor discards the replacement's own.
            for r in &mut refs {
                r.2 = RefKind::Gone;
            }
        }
        scan.record(pi, anchor, content, refs);
    }
    for anchor in kept {
        if let Some(plan) = plans.get_mut(&anchor) {
            plan.gone = false;
            plan.replaced_by_one = false;
        }
    }

    // `discards` maps a patched ancestor to "its original children are discarded" for the location walks below.
    let mut discards: FxHashMap<u32, bool> = FxHashMap::default();
    for (&anchor, plan) in &plans {
        let keeps_children = plan.winning_replace.is_some_and(|pi| {
            matches!(
                patches[pi],
                Patch::Replace {
                    keep_children: true,
                    ..
                }
            )
        });
        let new_children = plan.winning_set_children.is_some() && plan.takes_child_edits();
        if new_children || (plan.gone && !keeps_children) {
            discards.insert(anchor, true);
        }
    }

    // Refs count only in payloads the apply actually emits.
    for (&anchor, plan) in &plans {
        for &pi in &patch_map[&anchor] {
            let p = &patches[pi];
            let used = match p {
                Patch::Replace { .. } | Patch::Remove { .. } => false,
                Patch::Wrap { .. } => !plan.removed,
                Patch::SetChildren { .. } => {
                    plan.winning_set_children == Some(pi) && plan.takes_child_edits()
                }
                Patch::PrependChild { .. } | Patch::AppendChild { .. } => plan.takes_child_edits(),
                Patch::InsertBefore { .. } | Patch::InsertAfter { .. } => true,
            };
            if !used {
                continue;
            }
            let Some(content) = patch_payload(p) else {
                continue;
            };
            let rebuilds = matches!(p, Patch::SetChildren { .. });
            let is_wrap = matches!(p, Patch::Wrap { .. });
            let refs = scan_refs(arena, content, is_wrap, |target, td| {
                if td.get(4) == Some(&REF_KIND_SLOT) || (rebuilds && is_inside(target, anchor)) {
                    RefKind::Arranged
                } else if plans.get(&target).is_some_and(|plan| plan.removed) {
                    RefKind::Gone
                } else {
                    RefKind::Placed
                }
            })?;
            scan.record(pi, anchor, content, refs);
        }
    }
    let RefScan {
        uses: ref_uses,
        positions: mut ref_positions,
        placeholders: mut ref_placeholders,
        targets: ref_targets,
        gone: gone_targets,
    } = scan;

    // Only a non-root replacement may name its own anchor; anywhere else the payload would contain itself.
    for (&pi, refs) in ref_positions.iter().chain(&ref_placeholders) {
        let patch = &patches[pi];
        let anchor = patch_anchor(patch);
        if (anchor == 0 || !matches!(patch, Patch::Replace { .. }))
            && refs.iter().any(|&(_, target, _)| target == anchor)
        {
            return Err(CommandError::NodeInsideItself(NodeLabel::of(arena, anchor)));
        }
    }

    // A node's fate follows its decider chain, nearest ancestor first: a ref
    // target rescues the region only if some LIVE anchor splices it, so
    // rescues cascade and must settle by fixpoint. With no refs there is
    // nothing to rescue or splice: the first discarding ancestor decides.
    let mut target_inner_patched: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
    let mut deciders: FxHashMap<u32, (Vec<u32>, bool)> = FxHashMap::default();
    let mut dropped_set: FxHashSet<u32> = FxHashSet::default();
    let mut claimed: FxHashSet<u32> = FxHashSet::default();
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
                    return Err(CommandError::AncestorPlacedInside {
                        ancestor: NodeLabel::of(arena, target),
                        node: NodeLabel::of(arena, anchor),
                    });
                }
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
    }
    if !ref_positions.is_empty() || !ref_placeholders.is_empty() {
        // A node is in one place: placed at most once, arranged at most once, and a placement wins.
        let mut claims: FxHashMap<u32, (u32, u32, u32)> = FxHashMap::default(); // target -> (placed, self-refs, arranged)
        for (&pi, refs) in ref_positions.iter().chain(&ref_placeholders) {
            let referrer = patch_anchor(&patches[pi]);
            if dropped_set.contains(&referrer) {
                continue;
            }
            for &(_, target, kind) in refs {
                let count = claims.entry(target).or_default();
                match kind {
                    RefKind::Placed if target == referrer => count.1 += 1,
                    RefKind::Placed => count.0 += 1,
                    RefKind::Arranged => count.2 += 1,
                    RefKind::Gone => {}
                }
            }
        }
        for (&target, &(placed, self_refs, arranged)) in &claims {
            if placed + self_refs > 1 || arranged > 1 {
                return Err(CommandError::NodePlacedTwice(NodeLabel::of(arena, target)));
            }
            if placed + arranged > 0 {
                claimed.insert(target);
            }
        }
        for refs in ref_positions
            .values_mut()
            .chain(ref_placeholders.values_mut())
        {
            for (_, target, kind) in refs.iter_mut() {
                if *kind == RefKind::Arranged && claims.get(target).is_some_and(|c| c.0 > 0) {
                    *kind = RefKind::Gone;
                }
            }
        }
    }
    // A removal that won over a ref did what it asked for, so it is not reported as lost.
    dropped_set.retain(|anchor| {
        !(gone_targets.contains(anchor)
            && patch_map[anchor]
                .iter()
                .all(|&pi| matches!(patches[pi], Patch::Remove { .. })))
    });
    for (&anchor, plan) in plans.iter_mut() {
        if dropped_set.contains(&anchor) {
            plan.dropped = true;
        }
    }
    // A wrap needs something in the node's place, and a child edit the node or its one-node replacement.
    let lost_edits: Vec<u32> = plans
        .iter()
        .filter(|&(anchor, plan)| {
            !plan.dropped
                && ((plan.removed && plan.wrapped)
                    || (!plan.takes_child_edits()
                        && patch_map[anchor].iter().any(|&pi| {
                            matches!(
                                patches[pi],
                                Patch::PrependChild { .. }
                                    | Patch::AppendChild { .. }
                                    | Patch::SetChildren { .. }
                            )
                        })))
        })
        .map(|(&a, _)| a)
        .collect();
    // Strict callers reject stranded patches here, before the first arena
    // mutation, so the arena survives the error untouched.
    if strict && let Some(&anchor) = dropped_set.iter().chain(&lost_edits).min() {
        return Err(CommandError::PatchOnRemovedSubtree(NodeLabel::of(
            arena, anchor,
        )));
    }

    // Referring anchors wait for the target's patches and patches inside it.
    let order: Vec<u32> = if ref_uses.is_empty() {
        anchor_order.clone()
    } else {
        let mut deps: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
        for &(anchor, target) in &ref_uses {
            let mut d: Vec<u32> = Vec::new();
            if plans.contains_key(&target) {
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
        let mut cycle: Option<(u32, u32)> = None;
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
                            cycle = Some((dep, node));
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
            if cycle.is_some() {
                break;
            }
        }
        if let Some((a, b)) = cycle {
            return Err(CommandError::CircularPlacement(
                NodeLabel::of(arena, a),
                NodeLabel::of(arena, b),
            ));
        }
        out
    };

    // Grouping is safe only when nothing copies a spliced subtree; splicing per anchor is O(anchors × siblings).
    let defer_splices = ref_uses.is_empty()
        || (target_inner_patched.is_empty()
            && !ref_targets.iter().any(|t| {
                patch_map.get(t).is_some_and(|group| {
                    group.iter().any(|&pi| {
                        !matches!(
                            &patches[pi],
                            Patch::InsertBefore { .. } | Patch::InsertAfter { .. }
                        )
                    })
                })
            }));
    let mut pending_splices: FxHashMap<u32, FxHashMap<u32, Vec<u32>>> = FxHashMap::default();
    let mut position_slots: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
    let mut grafted: FxHashMap<usize, Vec<u32>> = FxHashMap::default();
    let mut wrap_resolved: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
    let mut redirect: FxHashMap<u32, u32> = FxHashMap::default();
    let mut stranded: Vec<u32> = Vec::new();

    // A splice into a parent that rebuilds its own child list later would be
    // overwritten by that rebuild, so it waits for it instead.
    let rebuild_at: FxHashMap<u32, usize> = order
        .iter()
        .enumerate()
        .filter(|(_, a)| {
            patch_map.get(a).is_some_and(|group| {
                group.iter().any(|&pi| {
                    matches!(
                        &patches[pi],
                        Patch::PrependChild { .. }
                            | Patch::AppendChild { .. }
                            | Patch::SetChildren { .. }
                            | Patch::Replace {
                                keep_children: true,
                                ..
                            }
                    )
                })
            })
        })
        .map(|(i, &a)| (a, i))
        .collect();

    // A claimed node leaves its old list first; a gone node's list is never read again, so it is skipped.
    let list_gone = |parent: &u32| {
        plans.get(parent).is_some_and(|plan| {
            plan.gone
                && plan.winning_replace.is_none_or(|pi| {
                    !matches!(
                        patches[pi],
                        Patch::Replace {
                            keep_children: true,
                            ..
                        }
                    )
                })
        })
    };
    let vacated: FxHashSet<u32> = claimed
        .iter()
        .map(|&t| arena.get_node(t).parent)
        .filter(|parent| !list_gone(parent))
        .collect();
    for parent in vacated {
        let rest: Vec<u32> = arena
            .get_children(parent)
            .iter()
            .copied()
            .filter(|child| !claimed.contains(child))
            .collect();
        arena.set_children(parent, &rest);
    }

    for (anchor_index, anchor) in order.into_iter().enumerate() {
        let plan = &plans[&anchor];
        if plan.dropped {
            continue;
        }
        let group: &[usize] = &patch_map[&anchor];
        let winning_replace = plan.winning_replace;
        let winning_set_children = plan.winning_set_children;
        let place_replaced = plan.place_replaced();
        let takes_child_edits = plan.takes_child_edits();
        let is_empty_wrapper = |content: &PatchContent<K>| match content {
            PatchContent::Tree(t) => t.is_empty(),
            PatchContent::Grafted(roots) => roots.is_empty(),
        };
        let empty_wrapper = plan.wrapped
            && group.iter().any(|&pi| {
                matches!(&patches[pi], Patch::Wrap { parent_tree, .. } if is_empty_wrapper(parent_tree))
            });
        // Read before a self-ref or the wrap wiring re-parents the anchor.
        let splice_parent = arena.get_node(anchor).parent;

        grafted.clear();
        for &pi in group {
            let p = &patches[pi];
            let used = match p {
                Patch::Replace { .. } => winning_replace == Some(pi),
                // Wrap payloads graft in the wiring below, at their final pool base.
                Patch::Wrap { .. } => false,
                Patch::SetChildren { .. } => winning_set_children == Some(pi) && takes_child_edits,
                Patch::PrependChild { .. } | Patch::AppendChild { .. } => takes_child_edits,
                Patch::Remove { .. } => false,
                _ => true,
            };
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
                let sub_pool_base = if new_tree.string_pool().is_empty() {
                    0
                } else {
                    arena.alloc_string(new_tree.string_pool()).offset
                };
                let node = *new_tree.get_node(0);
                if anchor == 0 {
                    // Overwrite node 0 in place so the root id stays stable.
                    {
                        let root = arena.get_node_mut(0);
                        root.node_type = node.node_type;
                        root.data_offset = 0;
                        root.data_len = 0;
                        root.start_offset = 0;
                        root.end_offset = 0;
                        root.start_line = 0;
                        root.start_column = 0;
                        root.end_line = 0;
                        root.end_column = 0;
                    }
                    let type_data = new_tree.get_type_data(0);
                    if !type_data.is_empty() {
                        if sub_pool_base != 0 {
                            let mut remapped = type_data.to_vec();
                            remap_string_refs::<K>(&mut remapped, node.node_type, sub_pool_base);
                            arena.set_type_data(0, &remapped);
                        } else {
                            arena.set_type_data(0, type_data);
                        }
                    }
                    match new_tree.get_node_data(0) {
                        Some(data) => {
                            let data = data.to_vec();
                            arena.set_node_data(0, data);
                        }
                        None => {
                            arena.node_data.remove(&0);
                        }
                    }
                    grafted.insert(pi, vec![0]);
                    continue;
                }
                // A child anchor's pending splice must land before this
                // anchor's children move to the replacement node.
                flush_pending_splices(arena, &mut pending_splices, anchor, &mut stranded);
                let original_children = arena.get_children(anchor).to_vec();
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
                grafted.insert(pi, vec![new_id]);
                continue;
            }
            let Some(content) = patch_payload(p) else {
                continue;
            };
            let preserve_root = anchor == 0 && matches!(p, Patch::Replace { .. });
            match content {
                PatchContent::Tree(sub) => {
                    let mut resolved: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
                    if let Some(positions) = ref_positions.get(&pi) {
                        for &(sub_id, target, kind) in positions {
                            resolved.insert(
                                sub_id,
                                resolve_target(target, anchor, &position_slots, kind),
                            );
                        }
                    }
                    grafted.insert(pi, graft_subtree(arena, sub, &resolved, preserve_root));
                }
                PatchContent::Grafted(roots) => {
                    static EMPTY: &[(u32, u32, RefKind)] = &[];
                    let placeholders = ref_placeholders
                        .get(&pi)
                        .map(Vec::as_slice)
                        .unwrap_or(EMPTY);
                    let ids = resolve_grafted(
                        arena,
                        roots,
                        placeholders,
                        anchor,
                        &position_slots,
                        if preserve_root {
                            GraftMode::KeepRoot
                        } else {
                            GraftMode::Splice
                        },
                    );
                    grafted.insert(pi, ids);
                }
            }
        }

        // The rebuild's degenerate empty wrapper suppresses the anchor's own
        // child-list patches (`copy_node_raw(.., false)` on re-entry).
        if takes_child_edits && (plan.gone || !empty_wrapper) {
            let has_own_edit = group.iter().any(|&pi| {
                matches!(
                    &patches[pi],
                    Patch::PrependChild { .. }
                        | Patch::AppendChild { .. }
                        | Patch::SetChildren { .. }
                )
            });
            if has_own_edit {
                let target = match winning_replace {
                    Some(wi) if plan.gone => grafted[&wi][0],
                    _ => anchor,
                };
                // A child anchor's pending splice must land before this anchor's
                // own child list is read; a set-children discards that list, so
                // there its splices are re-applied to the new one instead.
                if winning_set_children.is_none() {
                    flush_pending_splices(arena, &mut pending_splices, target, &mut stranded);
                }
                let mut new_list: Vec<u32> = Vec::new();
                for &pi in group {
                    if let Patch::PrependChild { .. } = &patches[pi] {
                        new_list.extend_from_slice(&grafted[&pi]);
                    }
                }
                if let Some(wi) = winning_set_children {
                    new_list.extend_from_slice(&grafted[&wi]);
                } else {
                    new_list.extend_from_slice(arena.get_children(target));
                }
                for &pi in group {
                    if let Patch::AppendChild { .. } = &patches[pi] {
                        new_list.extend_from_slice(&grafted[&pi]);
                    }
                }
                arena.set_children(target, &new_list);
                if winning_set_children.is_some() {
                    flush_pending_splices(arena, &mut pending_splices, target, &mut stranded);
                }
            }
        }

        let mut core: Vec<u32> = Vec::new();
        if place_replaced {
            if let Some(wi) = winning_replace {
                core.extend_from_slice(&grafted[&wi]);
            }
        } else {
            core.push(anchor);
        }
        if anchor == 0 && place_replaced {
            // The root has no parent list to splice into, so its replacement lands on node 0 itself.
            match core.as_slice() {
                // keep_children Replace already rewrote node 0 in place.
                [0] => {}
                [] => clear_root(arena),
                &[new_root] => overwrite_root_with(arena, new_root),
                many => {
                    let ids = many.to_vec();
                    arena.set_children(0, &ids);
                }
            }
            core = vec![0];
        }

        // Parity with emit_wrap_node: wrapper positions are NOT pool-rebased.
        for &wi in group {
            let Patch::Wrap { parent_tree, .. } = &patches[wi] else {
                continue;
            };
            if plan.removed || is_empty_wrapper(parent_tree) {
                continue;
            }
            wrap_resolved.clear();
            if let Some(positions) = ref_positions.get(&wi) {
                for &(sub_id, target, kind) in positions {
                    let ids = resolve_target(target, anchor, &position_slots, kind);
                    wrap_resolved.insert(sub_id, ids);
                }
            }
            if let (PatchContent::Grafted(roots), Some(placeholders)) =
                (parent_tree, ref_placeholders.get(&wi))
            {
                resolve_grafted(
                    arena,
                    roots,
                    placeholders,
                    anchor,
                    &position_slots,
                    GraftMode::Wrap,
                );
            }
            let wrapped = std::mem::take(&mut core);
            if anchor == 0 {
                // The wrapper takes over node 0 so the tree keeps its root id.
                flush_pending_splices(arena, &mut pending_splices, 0, &mut stranded);
                let old_root = copy_root_shallow(arena);
                match parent_tree {
                    PatchContent::Grafted(roots) => {
                        let wrapper_id = roots[0];
                        let wrapper = *arena.get_node(wrapper_id);
                        let mut wrapper_children: Vec<u32> = vec![old_root];
                        wrapper_children.extend_from_slice(arena.get_children(wrapper_id));
                        {
                            let root = arena.get_node_mut(0);
                            root.node_type = wrapper.node_type;
                            root.data_offset = wrapper.data_offset;
                            root.data_len = wrapper.data_len;
                            root.start_offset = wrapper.start_offset;
                            root.end_offset = wrapper.end_offset;
                            root.start_line = wrapper.start_line;
                            root.start_column = wrapper.start_column;
                            root.end_line = wrapper.end_line;
                            root.end_column = wrapper.end_column;
                        }
                        if let Some(data) = arena.node_data.remove(&wrapper_id) {
                            arena.node_data.insert(0, data);
                        }
                        arena.set_children(0, &wrapper_children);
                    }
                    PatchContent::Tree(parent_tree) => {
                        let sub_pool = parent_tree.string_pool();
                        let source_base = if sub_pool.is_empty() {
                            0u32
                        } else {
                            arena.alloc_string(sub_pool).offset
                        };
                        let wrapper = *parent_tree.get_node(0);
                        {
                            let root = arena.get_node_mut(0);
                            root.node_type = wrapper.node_type;
                            root.data_offset = 0;
                            root.data_len = 0;
                            root.start_offset = wrapper.start_offset;
                            root.end_offset = wrapper.end_offset;
                            root.start_line = wrapper.start_line;
                            root.start_column = wrapper.start_column;
                            root.end_line = wrapper.end_line;
                            root.end_column = wrapper.end_column;
                        }
                        let wrapper_data = parent_tree.get_type_data(0).to_vec();
                        if !wrapper_data.is_empty() {
                            if source_base != 0 {
                                let mut remapped = wrapper_data;
                                remap_string_refs::<K>(
                                    &mut remapped,
                                    wrapper.node_type,
                                    source_base,
                                );
                                arena.set_type_data(0, &remapped);
                            } else {
                                arena.set_type_data(0, &wrapper_data);
                            }
                        }
                        match parent_tree.get_node_data(0) {
                            Some(data) => {
                                let data = data.to_vec();
                                arena.set_node_data(0, data);
                            }
                            None => {
                                arena.node_data.remove(&0);
                            }
                        }
                        let mut wrapper_children: Vec<u32> = vec![old_root];
                        for c in parent_tree.get_children(0).to_vec() {
                            graft_node(
                                arena,
                                parent_tree,
                                c,
                                source_base,
                                &wrap_resolved,
                                &mut wrapper_children,
                                0,
                            );
                        }
                        arena.set_children(0, &wrapper_children);
                    }
                }
                core.push(0);
            } else if let PatchContent::Grafted(roots) = parent_tree {
                // Grafted wrapper is already in the arena; what holds the anchor's place comes first.
                let wrapper_id = roots[0];
                let mut wrapper_children: Vec<u32> = wrapped;
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
                    NodePosition {
                        start_offset: wrapper.start_offset,
                        end_offset: wrapper.end_offset,
                        start_line: wrapper.start_line,
                        start_column: wrapper.start_column,
                        end_line: wrapper.end_line,
                        end_column: wrapper.end_column,
                    },
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
                let mut wrapper_children: Vec<u32> = wrapped;
                for c in parent_tree.get_children(0).to_vec() {
                    graft_node(
                        arena,
                        parent_tree,
                        c,
                        source_base,
                        &wrap_resolved,
                        &mut wrapper_children,
                        0,
                    );
                }
                arena.set_children(wrapper_id, &wrapper_children);
                core.push(wrapper_id);
            }
        }

        let mut slot: Vec<u32> = Vec::new();
        for &pi in group {
            if let Patch::InsertBefore { .. } = &patches[pi] {
                slot.extend_from_slice(&grafted[&pi]);
            }
        }
        slot.extend_from_slice(&core);
        for &pi in group {
            if let Patch::InsertAfter { .. } = &patches[pi] {
                slot.extend_from_slice(&grafted[&pi]);
            }
        }

        if anchor != 0 && !claimed.contains(&anchor) && slot.as_slice() != [anchor] {
            let parent = redirect
                .get(&splice_parent)
                .copied()
                .unwrap_or(splice_parent);
            if defer_splices || rebuild_at.get(&parent).is_some_and(|&i| i > anchor_index) {
                // Recorded before deferring: a ref resolving later still needs
                // to know what this anchor became.
                if !ref_uses.is_empty() {
                    position_slots.insert(anchor, slot.clone());
                }
                pending_splices
                    .entry(parent)
                    .or_default()
                    .insert(anchor, slot);
                continue;
            }
            let current = arena.get_children(parent).to_vec();
            let mut new_list: Vec<u32> = Vec::with_capacity(current.len() + slot.len());
            let mut found = false;
            for &child in &current {
                if child == anchor {
                    new_list.extend_from_slice(&slot);
                    found = true;
                } else {
                    new_list.push(child);
                }
            }
            if !found && !slot.is_empty() {
                stranded.push(anchor);
            }
            arena.set_children(parent, &new_list);
        }
        if !ref_uses.is_empty() {
            position_slots.insert(anchor, slot);
        }
    }

    let parents: Vec<u32> = pending_splices.keys().copied().collect();
    for parent in parents {
        flush_pending_splices(arena, &mut pending_splices, parent, &mut stranded);
    }

    // `rebuild` never carried utf16_offsets into the new arena; match it.
    arena.utf16_offsets = Vec::new();

    let mut dropped: Vec<u32> = plans
        .iter()
        .filter(|(_, plan)| plan.dropped)
        .map(|(&a, _)| a)
        .collect();
    dropped.extend(lost_edits);
    dropped.extend(stranded);
    dropped.sort_unstable();
    dropped.dedup();

    Ok(dropped)
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
    use satteri_arena::{ArenaBuilder, Hast, Mdast};

    use super::*;
    use crate::mdast::MdastNodeType;

    /// Old `rebuild` contract on a fresh clone: dropped -> error.
    fn rebuild<K: ArenaKind>(
        arena: &Arena<K>,
        patches: &[Patch<K>],
    ) -> Result<Arena<K>, CommandError> {
        let mut arena = arena.clone();
        apply_patches_strict(&mut arena, patches)?;
        Ok(arena)
    }

    fn rebuild_lenient<K: ArenaKind>(
        arena: &Arena<K>,
        patches: &[Patch<K>],
    ) -> Result<(Arena<K>, Vec<u32>), CommandError> {
        let mut arena = arena.clone();
        let dropped = apply_patches_in_place(&mut arena, patches)?;
        Ok((arena, dropped))
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
        if arena.is_empty() { 0 } else { walk(arena, 0) }
    }

    /// Build the "# Hello\n\nWorld" arena for testing.
    fn build_hello_world() -> Arena<Mdast> {
        use satteri_arena::StringRef;

        use crate::mdast::codec::{encode_heading_data, encode_string_ref_data};

        let source = "# Hello\n\nWorld".to_string();
        let mut b = ArenaBuilder::<Mdast>::new(source);

        b.open_node(MdastNodeType::Root as u8);
        b.set_position_current(NodePosition {
            start_offset: 0,
            end_offset: 14,
            start_line: 1,
            start_column: 1,
            end_line: 2,
            end_column: 6,
        });

        b.open_node(MdastNodeType::Heading as u8);
        b.set_position_current(NodePosition {
            start_offset: 0,
            end_offset: 7,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 8,
        });
        b.set_data_current(&encode_heading_data(1));

        b.open_node(MdastNodeType::Text as u8);
        b.set_position_current(NodePosition {
            start_offset: 2,
            end_offset: 7,
            start_line: 1,
            start_column: 3,
            end_line: 1,
            end_column: 8,
        });
        b.set_data_current(&encode_string_ref_data(StringRef::new(2, 5)));
        b.close_node(); // text

        b.close_node(); // heading

        b.open_node(MdastNodeType::Paragraph as u8);
        b.set_position_current(NodePosition {
            start_offset: 9,
            end_offset: 14,
            start_line: 2,
            start_column: 1,
            end_line: 2,
            end_column: 6,
        });

        b.open_node(MdastNodeType::Text as u8);
        b.set_position_current(NodePosition {
            start_offset: 9,
            end_offset: 14,
            start_line: 2,
            start_column: 1,
            end_line: 2,
            end_column: 6,
        });
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
        b.open_node(HastNodeType::Root as u8);

        b.open_node(HastNodeType::Element as u8);
        // Element type_data: tag_ref(0..8), prop_count(8..12), pad(12..16)
        let tag = b.alloc_string("h1");
        let mut td = vec![0u8; 16];
        td[0..4].copy_from_slice(&tag.offset.to_le_bytes());
        td[4..8].copy_from_slice(&tag.len.to_le_bytes());
        b.set_data_current(&td);

        b.open_node(HastNodeType::Text as u8);
        let text = b.alloc_string("Hello");
        b.set_data_current(&encode_string_ref_data(text));
        b.close_node(); // text

        b.close_node(); // h1
        b.close_node(); // root
        let orig = b.finish();

        // Build wrapper: div element
        let mut wb = ArenaBuilder::<Hast>::new(String::new());
        wb.open_node(HastNodeType::Element as u8);
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

    /// A root-wrapped payload is the shape a parsed raw string produces.
    #[test]
    fn wrap_with_root_payload_is_rejected() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];

        let mut wb = ArenaBuilder::<Mdast>::new(String::new());
        wb.open_node(MdastNodeType::Root as u8);
        wb.open_node(MdastNodeType::Html as u8);
        wb.close_node();
        wb.close_node();
        let payload = wb.finish();

        let patches = vec![Patch::Wrap {
            node_id: heading_id,
            parent_tree: PatchContent::Tree(payload),
        }];
        match rebuild(&orig, &patches) {
            Err(CommandError::UnsupportedPatchShape(_)) => {}
            other => panic!("expected UnsupportedPatchShape, got {other:?}"),
        }
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
    /// for every prop set, each carrying the accumulated attribute list, so
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

    #[test]
    fn a_wrap_or_child_edit_on_a_removed_node_is_dropped_and_reported() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];
        let edits: [fn(u32) -> Patch<Mdast>; 3] = [
            |node_id| Patch::Wrap {
                node_id,
                parent_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Blockquote)),
            },
            |node_id| Patch::PrependChild {
                node_id,
                child_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Break)),
            },
            |node_id| Patch::AppendChild {
                node_id,
                child_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Break)),
            },
        ];
        for edit in edits {
            let patches = vec![
                edit(heading_id),
                Patch::Remove {
                    node_id: heading_id,
                },
            ];
            let (arena, dropped) = rebuild_lenient(&orig, &patches).unwrap();
            assert_eq!(dropped, vec![heading_id]);
            assert_eq!(arena.get_children(0), &orig.get_children(0)[1..]);
            match rebuild(&orig, &patches) {
                Err(CommandError::PatchOnRemovedSubtree(node)) => assert_eq!(node.id, heading_id),
                other => panic!("expected PatchOnRemovedSubtree, got {other:?}"),
            }
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
            Err(CommandError::PatchOnRemovedSubtree(node)) => assert_eq!(node.id, text_in_heading),
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
        // inside it, the kind of pair a nested-directive transform produces.
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
        let (arena, dropped) =
            rebuild_lenient(&orig, &patches).expect("lenient rebuild should not error");
        assert_eq!(dropped, vec![text_in_heading]);
        // The heading replacement still applied: root's first child is the new Paragraph.
        let root_children = arena.get_children(0);
        assert_eq!(
            arena.get_node(root_children[0]).node_type,
            MdastNodeType::Paragraph as u8
        );
    }

    /// Same shape as the stranding test, but the replacement *references* the
    /// original child via a `REF_NODE_TYPE` node instead of discarding it. The
    /// child's own patch then applies (text → Break) and nothing strands. This
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
        replacement.open_node(REF_NODE_TYPE);
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
        let (arena, dropped) =
            rebuild_lenient(&orig, &patches).expect("lenient rebuild should not error");
        assert!(
            dropped.is_empty(),
            "the referenced child should not strand: {dropped:?}"
        );
        // root > blockquote > break (the referenced text, transformed in place).
        let bq = arena.get_children(0)[0];
        assert_eq!(
            arena.get_node(bq).node_type,
            MdastNodeType::Blockquote as u8
        );
        let bq_children = arena.get_children(bq);
        assert_eq!(bq_children.len(), 1);
        assert_eq!(
            arena.get_node(bq_children[0]).node_type,
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
        let (arena, dropped) =
            rebuild_lenient(&orig, &patches).expect("lenient rebuild should not error");
        assert_eq!(dropped, vec![text_in_heading, text_in_para]);
        // Both removals applied: the root is now empty.
        assert_eq!(arena.get_children(0).len(), 0);
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
        b.open_node(REF_NODE_TYPE);
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
            Err(CommandError::AncestorPlacedInside { ancestor, node }) => {
                assert_eq!((ancestor.id, node.id), (0, heading_id));
            }
            other => panic!("expected AncestorPlacedInside, got {other:?}"),
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
            Err(CommandError::AncestorPlacedInside { ancestor, node }) => {
                assert_eq!((ancestor.id, node.id), (0, para_id));
            }
            other => panic!("expected AncestorPlacedInside, got {other:?}"),
        }
    }

    #[test]
    fn child_edits_cannot_reference_their_own_anchor() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];
        for patch in [
            Patch::AppendChild {
                node_id: heading_id,
                child_tree: PatchContent::Tree(ref_arena(heading_id, None)),
            },
            Patch::PrependChild {
                node_id: heading_id,
                child_tree: PatchContent::Tree(ref_arena(heading_id, None)),
            },
            Patch::SetChildren {
                node_id: heading_id,
                new_children: PatchContent::Tree(ref_arena(heading_id, Some(MdastNodeType::Root))),
            },
            Patch::Replace {
                node_id: 0,
                new_tree: PatchContent::Tree(ref_arena(0, Some(MdastNodeType::Root))),
                keep_children: false,
            },
        ] {
            let mut arena = orig.clone();
            let before = arena.to_raw_buffer();
            assert!(matches!(
                apply_patches_strict(&mut arena, &[patch]),
                Err(CommandError::NodeInsideItself(_))
            ));
            assert_eq!(
                arena.to_raw_buffer(),
                before,
                "validation must not mutate the arena"
            );
        }
    }

    /// A never-attached anchor (parent == u32::MAX) must error, not panic the ancestor walks.
    #[test]
    fn detached_anchor_errors_cleanly() {
        let mut orig = build_hello_world();
        let orphan = orig.alloc_node(MdastNodeType::Paragraph as u8);

        let patches = vec![Patch::Remove { node_id: orphan }];
        match rebuild(&orig, &patches) {
            Err(CommandError::UnsupportedPatchShape("detached anchor")) => {}
            other => panic!("expected detached-anchor error, got {other:?}"),
        }
    }

    /// A payload ref naming a never-attached node must error, not panic the deciders walk.
    #[test]
    fn detached_ref_target_errors_cleanly() {
        let mut orig = build_hello_world();
        let orphan = orig.alloc_node(MdastNodeType::Paragraph as u8);
        let para_id = orig.get_children(0)[1];

        let patches = vec![Patch::InsertAfter {
            node_id: para_id,
            new_tree: PatchContent::Tree(ref_arena(orphan, None)),
        }];
        match rebuild(&orig, &patches) {
            Err(CommandError::UnsupportedPatchShape("detached ref target")) => {}
            other => panic!("expected detached-ref-target error, got {other:?}"),
        }
    }

    /// An empty keep_children payload must error, not panic on `get_node(0)`.
    #[test]
    fn empty_keep_children_payload_errors() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];

        let patches = vec![Patch::Replace {
            node_id: heading_id,
            new_tree: PatchContent::Tree(ArenaBuilder::<Mdast>::new(String::new()).finish()),
            keep_children: true,
        }];
        match rebuild(&orig, &patches) {
            Err(CommandError::UnsupportedPatchShape("empty keep_children payload")) => {}
            other => panic!("expected empty-payload error, got {other:?}"),
        }
    }

    /// `Replace` on the root: the replacement's shape lands on node 0 (the
    /// root id is stable), and the old subtree is discarded.
    #[test]
    fn replace_root_with_single_node() {
        let orig = build_hello_world();

        let patches = vec![Patch::Replace {
            node_id: 0,
            new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Paragraph)),
            keep_children: false,
        }];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

        assert_eq!(
            rebuilt.get_node(0).node_type,
            MdastNodeType::Paragraph as u8
        );
        assert_eq!(rebuilt.get_children(0).len(), 0);
        assert_eq!(reachable_count(&rebuilt), 1);
    }

    /// `Replace` on the root with a Root-typed payload: node 0 takes the
    /// payload root's shape and children.
    #[test]
    fn replace_root_with_root_payload_swaps_children() {
        let orig = build_hello_world();

        let mut b = ArenaBuilder::<Mdast>::new(String::new());
        b.open_node(MdastNodeType::Root as u8);
        b.open_node(MdastNodeType::ThematicBreak as u8);
        b.close_node();
        b.open_node(MdastNodeType::Break as u8);
        b.close_node();
        b.close_node();
        let replacement = b.finish();

        let patches = vec![Patch::Replace {
            node_id: 0,
            new_tree: PatchContent::Tree(replacement),
            keep_children: false,
        }];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

        assert_eq!(rebuilt.get_node(0).node_type, MdastNodeType::Root as u8);
        let types: Vec<u8> = rebuilt
            .get_children(0)
            .iter()
            .map(|&id| rebuilt.get_node(id).node_type)
            .collect();
        assert_eq!(
            types,
            vec![
                MdastNodeType::ThematicBreak as u8,
                MdastNodeType::Break as u8
            ]
        );
    }

    /// `Replace { keep_children: true }` on the root: node 0 takes the
    /// payload's type/data but keeps its original children.
    #[test]
    fn replace_root_keep_children() {
        let orig = build_hello_world();

        let patches = vec![Patch::Replace {
            node_id: 0,
            new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Blockquote)),
            keep_children: true,
        }];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

        assert_eq!(
            rebuilt.get_node(0).node_type,
            MdastNodeType::Blockquote as u8
        );
        let types: Vec<u8> = rebuilt
            .get_children(0)
            .iter()
            .map(|&id| rebuilt.get_node(id).node_type)
            .collect();
        assert_eq!(
            types,
            vec![MdastNodeType::Heading as u8, MdastNodeType::Paragraph as u8]
        );
        assert_eq!(reachable_count(&rebuilt), orig.len());
    }

    /// A patch on a kept child still applies under a keep_children root
    /// replace, including when the child's patch is issued first.
    #[test]
    fn replace_root_keep_children_applies_descendant_patch() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];

        let patches = vec![
            Patch::InsertBefore {
                node_id: heading_id,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::ThematicBreak)),
            },
            Patch::Replace {
                node_id: 0,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Blockquote)),
                keep_children: true,
            },
        ];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

        assert_eq!(
            rebuilt.get_node(0).node_type,
            MdastNodeType::Blockquote as u8
        );
        let types: Vec<u8> = rebuilt
            .get_children(0)
            .iter()
            .map(|&id| rebuilt.get_node(id).node_type)
            .collect();
        assert_eq!(
            types,
            vec![
                MdastNodeType::ThematicBreak as u8,
                MdastNodeType::Heading as u8,
                MdastNodeType::Paragraph as u8,
            ]
        );
    }

    /// `Remove` on the root empties the document: node 0 stays a root with
    /// no children, matching the old rebuild's empty arena render.
    #[test]
    fn remove_root_empties_document() {
        let orig = build_hello_world();

        let patches = vec![Patch::Remove { node_id: 0 }];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

        assert_eq!(rebuilt.get_node(0).node_type, MdastNodeType::Root as u8);
        assert_eq!(rebuilt.get_children(0).len(), 0);
        assert_eq!(reachable_count(&rebuilt), 1);
    }

    /// `Wrap` on the root: the wrapper takes over node 0 and the old root
    /// (with all its children) becomes the wrapper's first child.
    #[test]
    fn wrap_root_moves_old_root_under_wrapper() {
        let orig = build_hello_world();

        let patches = vec![Patch::Wrap {
            node_id: 0,
            parent_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Blockquote)),
        }];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

        assert_eq!(
            rebuilt.get_node(0).node_type,
            MdastNodeType::Blockquote as u8
        );
        let wrapped = rebuilt.get_children(0);
        assert_eq!(wrapped.len(), 1);
        let old_root = wrapped[0];
        assert_eq!(
            rebuilt.get_node(old_root).node_type,
            MdastNodeType::Root as u8
        );
        let types: Vec<u8> = rebuilt
            .get_children(old_root)
            .iter()
            .map(|&id| rebuilt.get_node(id).node_type)
            .collect();
        assert_eq!(
            types,
            vec![MdastNodeType::Heading as u8, MdastNodeType::Paragraph as u8]
        );
        // Everything reachable: wrapper + old root + heading + text + para + text.
        assert_eq!(reachable_count(&rebuilt), orig.len() + 1);
    }

    /// `Wrap` composes with the root's own child edits: the append lands
    /// inside the old root before it moves under the wrapper, like any
    /// non-root wrap + append combination.
    #[test]
    fn wrap_root_composes_with_append_child() {
        let orig = build_hello_world();

        let patches = vec![
            Patch::AppendChild {
                node_id: 0,
                child_tree: PatchContent::Tree(single_node_arena(MdastNodeType::ThematicBreak)),
            },
            Patch::Wrap {
                node_id: 0,
                parent_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Blockquote)),
            },
        ];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

        let old_root = rebuilt.get_children(0)[0];
        let types: Vec<u8> = rebuilt
            .get_children(old_root)
            .iter()
            .map(|&id| rebuilt.get_node(id).node_type)
            .collect();
        assert_eq!(
            types,
            vec![
                MdastNodeType::Heading as u8,
                MdastNodeType::Paragraph as u8,
                MdastNodeType::ThematicBreak as u8,
            ]
        );
    }

    /// `Wrap` on the root while a descendant is edited in the same buffer.
    #[test]
    fn wrap_root_while_descendant_edited() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];

        let patches = vec![
            Patch::Wrap {
                node_id: 0,
                parent_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Blockquote)),
            },
            Patch::Replace {
                node_id: heading_id,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::ThematicBreak)),
                keep_children: false,
            },
        ];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

        let old_root = rebuilt.get_children(0)[0];
        let types: Vec<u8> = rebuilt
            .get_children(old_root)
            .iter()
            .map(|&id| rebuilt.get_node(id).node_type)
            .collect();
        assert_eq!(
            types,
            vec![
                MdastNodeType::ThematicBreak as u8,
                MdastNodeType::Paragraph as u8,
            ]
        );
    }

    /// Sibling inserts on the root stay rejected: the root has no siblings.
    #[test]
    fn insert_before_root_errors() {
        let orig = build_hello_world();

        let patches = vec![Patch::InsertBefore {
            node_id: 0,
            new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Break)),
        }];
        match rebuild(&orig, &patches) {
            Err(CommandError::UnsupportedPatchShape(
                "sibling insert on root (root has no siblings)",
            )) => {}
            other => panic!("expected sibling-insert-on-root error, got {other:?}"),
        }
    }

    #[test]
    fn insert_after_root_errors() {
        let orig = build_hello_world();

        let patches = vec![Patch::InsertAfter {
            node_id: 0,
            new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Break)),
        }];
        match rebuild(&orig, &patches) {
            Err(CommandError::UnsupportedPatchShape(
                "sibling insert on root (root has no siblings)",
            )) => {}
            other => panic!("expected sibling-insert-on-root error, got {other:?}"),
        }
    }

    /// A discarding root replace strands descendant patches like any other
    /// discarding replace: lenient drops them, strict errors before mutating.
    #[test]
    fn replace_root_strands_descendant_patch() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];

        let patches = vec![
            Patch::Replace {
                node_id: 0,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Paragraph)),
                keep_children: false,
            },
            Patch::Remove {
                node_id: heading_id,
            },
        ];
        let (arena, dropped) =
            rebuild_lenient(&orig, &patches).expect("lenient rebuild should not error");
        assert_eq!(dropped, vec![heading_id]);
        assert_eq!(arena.get_node(0).node_type, MdastNodeType::Paragraph as u8);
    }

    /// The strict variant rejects stranded patches BEFORE mutating: on error
    /// the arena is untouched.
    #[test]
    fn strict_errors_before_mutating_on_stranded_patch() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];
        let text_in_heading = orig.get_children(heading_id)[0];

        let mut work = orig.clone();
        let patches = vec![
            Patch::Remove {
                node_id: heading_id,
            },
            Patch::Replace {
                node_id: text_in_heading,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Break)),
                keep_children: false,
            },
        ];
        match apply_patches_strict(&mut work, &patches) {
            Err(CommandError::PatchOnRemovedSubtree(node)) => assert_eq!(node.id, text_in_heading),
            other => panic!("expected PatchOnRemovedSubtree, got {other:?}"),
        }
        // Untouched on error: same node count, same child lists.
        assert_eq!(work.len(), orig.len());
        assert_eq!(work.get_children(0), orig.get_children(0));
        assert_eq!(work.children, orig.children);
    }

    /// Grouped splices: many anchors under one parent, replaces and inserts
    /// interleaved, land exactly where the per-anchor splice put them.
    #[test]
    fn many_anchors_under_one_parent_splice_in_order() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];
        let para_id = orig.get_children(0)[1];

        let patches = vec![
            Patch::InsertBefore {
                node_id: heading_id,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::ThematicBreak)),
            },
            Patch::InsertAfter {
                node_id: heading_id,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Break)),
            },
            Patch::Replace {
                node_id: para_id,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Blockquote)),
                keep_children: false,
            },
            Patch::InsertBefore {
                node_id: para_id,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Code)),
            },
        ];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

        let types: Vec<u8> = rebuilt
            .get_children(0)
            .iter()
            .map(|&id| rebuilt.get_node(id).node_type)
            .collect();
        assert_eq!(
            types,
            vec![
                MdastNodeType::ThematicBreak as u8,
                MdastNodeType::Heading as u8,
                MdastNodeType::Break as u8,
                MdastNodeType::Code as u8,
                MdastNodeType::Blockquote as u8,
            ]
        );
    }

    /// A pending sibling splice under an anchor must land before that
    /// anchor's own child-list edit reads its children.
    #[test]
    fn pending_child_splice_lands_before_parent_own_edit() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];
        let text_in_heading = orig.get_children(heading_id)[0];

        let patches = vec![
            Patch::InsertBefore {
                node_id: text_in_heading,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Break)),
            },
            Patch::AppendChild {
                node_id: heading_id,
                child_tree: PatchContent::Tree(single_node_arena(MdastNodeType::ThematicBreak)),
            },
        ];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

        let heading_children = rebuilt.get_children(rebuilt.get_children(0)[0]);
        let types: Vec<u8> = heading_children
            .iter()
            .map(|&id| rebuilt.get_node(id).node_type)
            .collect();
        assert_eq!(
            types,
            vec![
                MdastNodeType::Break as u8,
                MdastNodeType::Text as u8,
                MdastNodeType::ThematicBreak as u8,
            ]
        );
    }

    /// A pending sibling splice must also land before a keep_children
    /// replace moves the parent's children to the replacement node.
    #[test]
    fn pending_child_splice_lands_before_keep_children_replace() {
        let orig = build_hello_world();
        let heading_id = orig.get_children(0)[0];
        let text_in_heading = orig.get_children(heading_id)[0];

        let patches = vec![
            Patch::InsertBefore {
                node_id: text_in_heading,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Break)),
            },
            Patch::Replace {
                node_id: heading_id,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Paragraph)),
                keep_children: true,
            },
        ];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");

        let wrapper = rebuilt.get_children(0)[0];
        assert_eq!(
            rebuilt.get_node(wrapper).node_type,
            MdastNodeType::Paragraph as u8
        );
        let types: Vec<u8> = rebuilt
            .get_children(wrapper)
            .iter()
            .map(|&id| rebuilt.get_node(id).node_type)
            .collect();
        assert_eq!(
            types,
            vec![MdastNodeType::Break as u8, MdastNodeType::Text as u8]
        );
    }
}
