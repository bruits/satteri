//! Arena patching: apply structural patches to the arena in place.

use rustc_hash::{FxHashMap, FxHashSet};
use satteri_arena::{Arena, ArenaKind, ArenaNode, Hast, Mdast};

use crate::commands::{CommandError, NodeLabel};

/// Sentinel `node_type` for a *reference* node inside a payload: the existing
/// node whose id is stored in this node's type_data (u32 LE) goes here, moved
/// from wherever it is when the patch applies. Higher than any real MDAST
/// (≤104) or HAST (≤14) type. A removed node resolves to nothing, and a payload
/// can name a node once.
pub const REF_NODE_TYPE: u8 = 0xFF;

/// Sentinel `node_type` standing for the anchor's children as they are when the patch applies.
pub const KEEP_CHILDREN_NODE_TYPE: u8 = 0xFE;

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
pub(crate) fn remap_string_refs<K: ArenaKind>(data: &mut [u8], node_type: u8, base: u32) {
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

pub fn patch_anchor<K: ArenaKind>(patch: &Patch<K>) -> u32 {
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

fn unsupported(reason: &'static str) -> CommandError {
    CommandError::UnsupportedPatchShape(reason)
}

/// Copy a free-standing tree into `arena` as orphans, returning the ids to
/// splice; a root-wrapped tree contributes its root's children.
pub fn graft_tree<K: ArenaKind>(arena: &mut Arena<K>, tree: &Arena<K>) -> Vec<u32> {
    copy_tree(arena, tree, Top::Splice)
}

/// Apply `patches` in order by editing the arena in place; detached nodes stay
/// behind as unreachable garbage (every consumer traverses from root). Returns
/// the anchors of patches that did nothing because their node was no longer in
/// the tree. On `Err` the patches before the failing one stay applied.
pub fn apply_patches_in_place<K: ArenaKind>(
    arena: &mut Arena<K>,
    patches: &[Patch<K>],
) -> Result<Vec<u32>, CommandError> {
    let mut applier = PatchApplier::default();
    let result = patches
        .iter()
        .try_for_each(|patch| applier.apply(arena, patch).map(drop));
    let dropped = applier.finish(arena);
    result.map(|()| dropped)
}

/// Like [`apply_patches_in_place`], but a patch whose node is no longer in the
/// tree fails with [`CommandError::PatchOnRemovedSubtree`].
pub fn apply_patches_strict<K: ArenaKind>(
    arena: &mut Arena<K>,
    patches: &[Patch<K>],
) -> Result<(), CommandError> {
    let mut applier = PatchApplier::default();
    let result = patches.iter().try_for_each(|patch| {
        if applier.apply(arena, patch)? {
            Ok(())
        } else {
            let node = NodeLabel::of(arena, patch_anchor(patch));
            Err(CommandError::PatchOnRemovedSubtree(node))
        }
    });
    applier.finish(arena);
    result
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Top {
    /// A root-wrapped payload contributes its root's children.
    Splice,
    Node,
}

const NONE: u32 = u32::MAX;

/// Applies patches one at a time, each to the tree the previous ones left.
/// Call [`PatchApplier::finish`] once the last patch is applied.
#[derive(Default)]
pub struct PatchApplier {
    /// A removed node stays removed: content naming it later leaves it out.
    removed: FxHashSet<u32>,
    lists: ChildLists,
    dropped: Vec<u32>,
}

impl PatchApplier {
    /// Apply one patch. `Ok(false)` means its node was not in the tree, so it did nothing.
    pub fn apply<K: ArenaKind>(
        &mut self,
        arena: &mut Arena<K>,
        patch: &Patch<K>,
    ) -> Result<bool, CommandError> {
        let anchor = patch_anchor(patch);
        if anchor as usize >= arena.len() {
            return Err(CommandError::InvalidNodeId(anchor));
        }
        match patch {
            Patch::Remove { .. } => self.remove(arena, anchor),
            _ if !in_tree(arena, anchor) => {
                self.dropped.push(anchor);
                return Ok(false);
            }
            Patch::Replace { new_tree, .. } => self.replace(arena, anchor, new_tree)?,
            Patch::InsertBefore { new_tree, .. } => {
                self.insert_sibling(arena, anchor, new_tree, false)?
            }
            Patch::InsertAfter { new_tree, .. } => {
                self.insert_sibling(arena, anchor, new_tree, true)?
            }
            Patch::PrependChild { child_tree, .. } => {
                self.insert_child(arena, anchor, child_tree, false)?
            }
            Patch::AppendChild { child_tree, .. } => {
                self.insert_child(arena, anchor, child_tree, true)?
            }
            Patch::SetChildren { new_children, .. } => {
                self.set_children(arena, anchor, new_children)?
            }
            Patch::Wrap { parent_tree, .. } => self.wrap(arena, anchor, parent_tree)?,
        }
        Ok(true)
    }

    /// Run an edit that leaves the tree's shape alone, such as a field write, if `node_id` is still in the tree.
    pub fn edit_node<K: ArenaKind>(
        &mut self,
        arena: &mut Arena<K>,
        node_id: u32,
        edit: impl FnOnce(&mut Arena<K>) -> Result<(), CommandError>,
    ) -> Result<(), CommandError> {
        if in_tree(arena, node_id) {
            edit(arena)
        } else {
            self.dropped.push(node_id);
            Ok(())
        }
    }

    /// Write the edited child lists back into the arena, returning the nodes of the edits that were dropped.
    pub fn finish<K: ArenaKind>(self, arena: &mut Arena<K>) -> Vec<u32> {
        for (parent, first) in &self.lists.heads {
            arena.set_children(*parent, &self.lists.collect(first.0));
        }
        self.dropped
    }

    fn remove<K: ArenaKind>(&mut self, arena: &mut Arena<K>, node_id: u32) {
        self.removed.insert(node_id);
        if node_id == 0 {
            self.clear(arena, 0);
            clear_root_fields(arena);
        } else {
            self.unlink(arena, node_id);
        }
    }

    fn replace<K: ArenaKind>(
        &mut self,
        arena: &mut Arena<K>,
        anchor: u32,
        content: &PatchContent<K>,
    ) -> Result<(), CommandError> {
        if anchor == 0 {
            let mut tops = materialize(arena, content, Top::Node);
            let refs = self.claim_refs(arena, &tops, 0, 0, false)?;
            refs.place(arena, &mut tops);
            self.clear(arena, 0);
            match *tops.as_slice() {
                [] => clear_root_fields(arena),
                [root] => {
                    copy_fields(arena, 0, root);
                    let children = arena.get_children(root).to_vec();
                    self.set(arena, 0, &children);
                }
                _ => self.set(arena, 0, &tops),
            }
            return Ok(());
        }
        let parent = arena.get_node(anchor).parent;
        let fragment = is_fragment(arena, content);
        let mut tops = materialize(arena, content, Top::Splice);
        let one_new_node = !fragment
            && matches!(*tops.as_slice(), [only] if !is_marker(arena.get_node(only).node_type));
        let refs = self.claim_refs(arena, &tops, parent, anchor, true)?;
        if one_new_node && !refs.names_anchor {
            refs.place(arena, &mut tops);
            self.take_over(arena, anchor, tops[0]);
            return Ok(());
        }
        let next = self.lists.next_sibling(arena, parent, anchor);
        self.unlink(arena, anchor);
        refs.place(arena, &mut tops);
        self.insert_before(arena, parent, next, &tops);
        Ok(())
    }

    fn insert_sibling<K: ArenaKind>(
        &mut self,
        arena: &mut Arena<K>,
        anchor: u32,
        content: &PatchContent<K>,
        after: bool,
    ) -> Result<(), CommandError> {
        if anchor == 0 {
            return Err(unsupported("sibling insert on root (root has no siblings)"));
        }
        let parent = arena.get_node(anchor).parent;
        let mut tops = materialize(arena, content, Top::Splice);
        self.claim_refs(arena, &tops, parent, anchor, false)?
            .place(arena, &mut tops);
        let at = if after {
            self.lists.next_sibling(arena, parent, anchor)
        } else {
            anchor
        };
        self.insert_before(arena, parent, at, &tops);
        Ok(())
    }

    fn insert_child<K: ArenaKind>(
        &mut self,
        arena: &mut Arena<K>,
        anchor: u32,
        content: &PatchContent<K>,
        append: bool,
    ) -> Result<(), CommandError> {
        let mut tops = materialize(arena, content, Top::Splice);
        self.claim_refs(arena, &tops, anchor, anchor, false)?
            .place(arena, &mut tops);
        let at = if append {
            NONE
        } else {
            self.lists.open(arena, anchor).0
        };
        self.insert_before(arena, anchor, at, &tops);
        Ok(())
    }

    fn set_children<K: ArenaKind>(
        &mut self,
        arena: &mut Arena<K>,
        anchor: u32,
        content: &PatchContent<K>,
    ) -> Result<(), CommandError> {
        let mut tops = materialize(arena, content, Top::Splice);
        self.claim_refs(arena, &tops, anchor, anchor, false)?
            .place(arena, &mut tops);
        self.set(arena, anchor, &tops);
        Ok(())
    }

    fn wrap<K: ArenaKind>(
        &mut self,
        arena: &mut Arena<K>,
        anchor: u32,
        content: &PatchContent<K>,
    ) -> Result<(), CommandError> {
        let Some(&wrapper) = materialize(arena, content, Top::Node).first() else {
            return Ok(());
        };
        if arena.get_node(wrapper).node_type == K::ROOT_TAG {
            return Err(unsupported("wrap parent is a document root"));
        }
        let parent = if anchor == 0 {
            0
        } else {
            arena.get_node(anchor).parent
        };
        let mut inner = arena.get_children(wrapper).to_vec();
        self.claim_refs(arena, &inner, parent, anchor, false)?
            .place(arena, &mut inner);
        let wrapped = if anchor == 0 {
            let root_type = arena.get_node(0).node_type;
            let old_root = arena.alloc_node(root_type);
            copy_fields(arena, old_root, 0);
            let root_children = self.lists.children(arena, 0);
            self.clear(arena, 0);
            arena.set_children(old_root, &root_children);
            copy_fields(arena, 0, wrapper);
            old_root
        } else {
            anchor
        };
        let mut children = Vec::with_capacity(inner.len() + 1);
        children.push(wrapped);
        children.extend_from_slice(&inner);
        if anchor == 0 {
            self.set(arena, 0, &children);
        } else {
            let next = self.lists.next_sibling(arena, parent, anchor);
            self.unlink(arena, anchor);
            arena.set_children(wrapper, &children);
            self.insert_before(arena, parent, next, &[wrapper]);
        }
        Ok(())
    }

    /// Check and detach every node the payload names but the anchor; `attach` is where the content lands.
    fn claim_refs<K: ArenaKind>(
        &mut self,
        arena: &mut Arena<K>,
        tops: &[u32],
        attach: u32,
        anchor: u32,
        allow_self: bool,
    ) -> Result<Refs, CommandError> {
        let mut refs = Refs::default();
        let mut stack: Vec<u32> = tops.to_vec();
        let mut named: Vec<u32> = Vec::new();
        while let Some(id) = stack.pop() {
            named.clear();
            match arena.get_node(id).node_type {
                REF_NODE_TYPE => named.push(ref_target(arena, id)?),
                KEEP_CHILDREN_NODE_TYPE => named.extend(self.lists.children(arena, anchor)),
                _ => {
                    stack.extend_from_slice(arena.get_children(id));
                    continue;
                }
            }
            let start = refs.targets.len() as u32;
            for &target in &named {
                if !refs.seen.insert(target) {
                    return Err(CommandError::NodePlacedTwice(NodeLabel::of(arena, target)));
                }
                if target == anchor {
                    if !allow_self {
                        return Err(CommandError::NodeInsideItself(NodeLabel::of(arena, anchor)));
                    }
                    refs.names_anchor = true;
                } else if self.removed.contains(&target) {
                    continue;
                } else if is_ancestor_or_self(arena, target, attach) {
                    return Err(CommandError::AncestorPlacedInside {
                        ancestor: NodeLabel::of(arena, target),
                        node: NodeLabel::of(arena, anchor),
                    });
                } else {
                    self.unlink(arena, target);
                }
                refs.targets.push(target);
            }
            refs.markers.insert(id, (start, refs.targets.len() as u32));
        }
        Ok(refs)
    }

    /// `anchor` becomes `src` where it stands, keeping its id.
    fn take_over<K: ArenaKind>(&mut self, arena: &mut Arena<K>, anchor: u32, src: u32) {
        copy_fields(arena, anchor, src);
        let children = arena.get_children(src).to_vec();
        self.set(arena, anchor, &children);
    }

    fn unlink<K: ArenaKind>(&mut self, arena: &mut Arena<K>, id: u32) {
        let parent = arena.get_node(id).parent;
        if parent != NONE {
            self.lists.unlink(arena, parent, id);
            arena.get_node_mut(id).parent = NONE;
        }
    }

    fn clear<K: ArenaKind>(&mut self, arena: &mut Arena<K>, parent: u32) {
        for child in self.lists.children(arena, parent) {
            if arena.get_node(child).parent == parent {
                arena.get_node_mut(child).parent = NONE;
            }
        }
        self.lists.heads.insert(parent, (NONE, NONE));
    }

    fn set<K: ArenaKind>(&mut self, arena: &mut Arena<K>, parent: u32, children: &[u32]) {
        self.clear(arena, parent);
        self.insert_before(arena, parent, NONE, children);
    }

    /// Link detached `nodes` under `parent` before `at`, or last when `at` is `NONE`.
    fn insert_before<K: ArenaKind>(
        &mut self,
        arena: &mut Arena<K>,
        parent: u32,
        at: u32,
        nodes: &[u32],
    ) {
        self.lists.insert_before(arena, parent, at, nodes);
        for &id in nodes {
            arena.get_node_mut(id).parent = parent;
        }
    }
}

/// The nodes a payload's ref and keep-children markers stand for, already detached from where they were.
#[derive(Default)]
struct Refs {
    /// Each marker's range in `targets`.
    markers: FxHashMap<u32, (u32, u32)>,
    targets: Vec<u32>,
    seen: FxHashSet<u32>,
    names_anchor: bool,
}

impl Refs {
    /// Swap each marker in the payload for the nodes it stands for.
    fn place<K: ArenaKind>(&self, arena: &mut Arena<K>, tops: &mut Vec<u32>) {
        if self.markers.is_empty() {
            return;
        }
        let mut holders: Vec<u32> = self
            .markers
            .keys()
            .map(|&marker| arena.get_node(marker).parent)
            .filter(|&holder| holder != NONE)
            .collect();
        holders.sort_unstable();
        holders.dedup();
        for holder in holders {
            let list = self.expand(arena.get_children(holder));
            arena.set_children(holder, &list);
        }
        *tops = self.expand(tops);
    }

    fn expand(&self, list: &[u32]) -> Vec<u32> {
        let mut out = Vec::with_capacity(list.len());
        for id in list {
            match self.markers.get(id) {
                Some(&(start, end)) => {
                    out.extend_from_slice(&self.targets[start as usize..end as usize])
                }
                None => out.push(*id),
            }
        }
        out
    }
}

/// Child lists edited this pass, linked so an edit beside a node costs the same at any width.
#[derive(Default)]
struct ChildLists {
    /// First and last child of each parent whose list is linked.
    heads: FxHashMap<u32, (u32, u32)>,
    prev: Vec<u32>,
    next: Vec<u32>,
}

impl ChildLists {
    fn open<K: ArenaKind>(&mut self, arena: &Arena<K>, parent: u32) -> (u32, u32) {
        if self.prev.len() < arena.len() {
            self.prev.resize(arena.len(), NONE);
            self.next.resize(arena.len(), NONE);
        }
        if let Some(&head) = self.heads.get(&parent) {
            return head;
        }
        let children = arena.get_children(parent);
        let mut last = NONE;
        for &child in children {
            self.prev[child as usize] = last;
            if last != NONE {
                self.next[last as usize] = child;
            }
            last = child;
        }
        if last != NONE {
            self.next[last as usize] = NONE;
        }
        let head = (children.first().copied().unwrap_or(NONE), last);
        self.heads.insert(parent, head);
        head
    }

    fn collect(&self, first: u32) -> Vec<u32> {
        let mut out = Vec::new();
        let mut cur = first;
        while cur != NONE {
            out.push(cur);
            cur = self.next[cur as usize];
        }
        out
    }

    fn children<K: ArenaKind>(&self, arena: &Arena<K>, parent: u32) -> Vec<u32> {
        match self.heads.get(&parent) {
            Some(&(first, _)) => self.collect(first),
            None => arena.get_children(parent).to_vec(),
        }
    }

    fn next_sibling<K: ArenaKind>(&mut self, arena: &Arena<K>, parent: u32, id: u32) -> u32 {
        self.open(arena, parent);
        self.next[id as usize]
    }

    fn unlink<K: ArenaKind>(&mut self, arena: &Arena<K>, parent: u32, id: u32) {
        let (first, _) = self.open(arena, parent);
        let (prev, next) = (self.prev[id as usize], self.next[id as usize]);
        let linked = if prev == NONE {
            first == id
        } else {
            self.next[prev as usize] == id
        };
        if !linked {
            debug_assert!(
                false,
                "node {id} is not in its parent {parent}'s child list"
            );
            return;
        }
        let head = self.heads.get_mut(&parent).expect("opened above");
        if prev == NONE {
            head.0 = next;
        } else {
            self.next[prev as usize] = next;
        }
        if next == NONE {
            head.1 = prev;
        } else {
            self.prev[next as usize] = prev;
        }
    }

    fn insert_before<K: ArenaKind>(
        &mut self,
        arena: &Arena<K>,
        parent: u32,
        at: u32,
        nodes: &[u32],
    ) {
        let (Some(&first), Some(&last)) = (nodes.first(), nodes.last()) else {
            return;
        };
        let (_, tail) = self.open(arena, parent);
        let before = if at == NONE {
            tail
        } else {
            self.prev[at as usize]
        };
        let mut prev = before;
        for &id in nodes {
            self.prev[id as usize] = prev;
            if prev != NONE {
                self.next[prev as usize] = id;
            }
            prev = id;
        }
        self.next[last as usize] = at;
        let head = self.heads.get_mut(&parent).expect("opened above");
        if before == NONE {
            head.0 = first;
        }
        if at == NONE {
            head.1 = last;
        } else {
            self.prev[at as usize] = last;
        }
    }
}

/// Whether `id` hangs from the root, rather than from something detached.
pub fn in_tree<K: ArenaKind>(arena: &Arena<K>, mut id: u32) -> bool {
    for _ in 0..arena.len() {
        if id == 0 {
            return true;
        }
        id = arena.get_node(id).parent;
        if id == NONE {
            return false;
        }
    }
    false
}

/// A root-wrapped payload, such as parsed raw content, is a list of nodes even when it holds one.
fn is_fragment<K: ArenaKind>(arena: &Arena<K>, content: &PatchContent<K>) -> bool {
    match content {
        PatchContent::Tree(sub) => !sub.is_empty() && sub.get_node(0).node_type == K::ROOT_TAG,
        PatchContent::Grafted(roots) => {
            matches!(*roots.as_slice(), [only] if arena.get_node(only).node_type == K::ROOT_TAG)
        }
    }
}

fn is_marker(node_type: u8) -> bool {
    node_type == REF_NODE_TYPE || node_type == KEEP_CHILDREN_NODE_TYPE
}

/// Give `dst` the shape of `src` (type, type_data, position, node_data), leaving its links alone.
fn copy_fields<K: ArenaKind>(arena: &mut Arena<K>, dst: u32, src: u32) {
    let src_node = *arena.get_node(src);
    let node = arena.get_node_mut(dst);
    *node = ArenaNode {
        id: node.id,
        parent: node.parent,
        children_start: node.children_start,
        children_count: node.children_count,
        ..src_node
    };
    match arena.node_data.get(&src).cloned() {
        Some(data) => arena.node_data.insert(dst, data),
        None => arena.node_data.remove(&dst),
    };
}

/// An empty document keeps its root type so it still serializes.
fn clear_root_fields<K: ArenaKind>(arena: &mut Arena<K>) {
    let root = arena.get_node_mut(0);
    *root = ArenaNode {
        parent: root.parent,
        children_start: root.children_start,
        children_count: root.children_count,
        ..ArenaNode::new(0, root.node_type)
    };
    arena.node_data.remove(&0);
}

fn is_ancestor_or_self<K: ArenaKind>(arena: &Arena<K>, ancestor: u32, mut id: u32) -> bool {
    loop {
        if id == ancestor {
            return true;
        }
        if id == 0 || id == NONE {
            return false;
        }
        id = arena.get_node(id).parent;
    }
}

fn ref_target<K: ArenaKind>(arena: &Arena<K>, ref_id: u32) -> Result<u32, CommandError> {
    let &[a, b, c, d, ..] = arena.get_type_data(ref_id) else {
        return Err(unsupported("ref node with short type_data"));
    };
    let target = u32::from_le_bytes([a, b, c, d]);
    if target as usize >= arena.len() || is_marker(arena.get_node(target).node_type) {
        return Err(CommandError::InvalidNodeId(target));
    }
    Ok(target)
}

fn materialize<K: ArenaKind>(
    arena: &mut Arena<K>,
    content: &PatchContent<K>,
    top: Top,
) -> Vec<u32> {
    match content {
        PatchContent::Grafted(roots) => {
            if top == Top::Splice
                && let [only] = *roots.as_slice()
                && arena.get_node(only).node_type == K::ROOT_TAG
            {
                return arena.get_children(only).to_vec();
            }
            roots.clone()
        }
        PatchContent::Tree(sub) => copy_tree(arena, sub, top),
    }
}

/// Payload coordinates are fragment-relative, so the copies keep the no-position sentinel.
fn copy_tree<K: ArenaKind>(arena: &mut Arena<K>, sub: &Arena<K>, top: Top) -> Vec<u32> {
    if sub.is_empty() {
        return Vec::new();
    }
    let tops: &[u32] = if top == Top::Splice && sub.get_node(0).node_type == K::ROOT_TAG {
        sub.get_children(0)
    } else {
        &[0]
    };
    let base = if sub.string_pool().is_empty() {
        0
    } else {
        arena.alloc_string(sub.string_pool()).offset
    };
    let mut copies = vec![NONE; sub.len()];
    let mut order = tops.to_vec();
    let mut i = 0;
    while i < order.len() {
        let id = order[i];
        let node_type = sub.get_node(id).node_type;
        let copy = arena.alloc_node(node_type);
        if let Some(data) = sub.get_node_data(id) {
            arena.set_node_data(copy, data.to_vec());
        }
        let type_data = sub.get_type_data(id);
        if base != 0 && !type_data.is_empty() && !is_marker(node_type) {
            let mut remapped = type_data.to_vec();
            remap_string_refs::<K>(&mut remapped, node_type, base);
            arena.set_type_data(copy, &remapped);
        } else if !type_data.is_empty() {
            arena.set_type_data(copy, type_data);
        }
        copies[id as usize] = copy;
        if !is_marker(node_type) {
            order.extend_from_slice(sub.get_children(id));
        }
        i += 1;
    }
    for &id in &order {
        let children = sub.get_children(id);
        if !children.is_empty() && !is_marker(sub.get_node(id).node_type) {
            let ids: Vec<u32> = children.iter().map(|&c| copies[c as usize]).collect();
            arena.set_children(copies[id as usize], &ids);
        }
    }
    tops.iter().map(|&t| copies[t as usize]).collect()
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
    use satteri_arena::{ArenaBuilder, Hast, Mdast, NodePosition};

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

    /// The payload `tree` with a marker for the replaced node's own children appended to its top node.
    fn with_kept_children<K: ArenaKind>(mut tree: Arena<K>) -> Arena<K> {
        let marker = tree.alloc_node(KEEP_CHILDREN_NODE_TYPE);
        let mut children = tree.get_children(0).to_vec();
        children.push(marker);
        tree.set_children(0, &children);
        tree
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

    /// Each `InsertAfter` lands right after the anchor, so the later one ends up first.
    #[test]
    fn a_later_insert_after_the_same_anchor_lands_first() {
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
                MdastNodeType::Break as u8,
                MdastNodeType::ThematicBreak as u8,
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
            },
            Patch::Replace {
                node_id: heading_id,
                new_tree: PatchContent::Tree(second),
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

    /// Each `PrependChild` lands first and each `AppendChild` last, in the order they apply.
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

        let new_heading_id = rebuilt.get_children(0)[0];
        let heading_children = rebuilt.get_children(new_heading_id);
        let types: Vec<u8> = heading_children
            .iter()
            .map(|&id| rebuilt.get_node(id).node_type)
            .collect();
        assert_eq!(
            types,
            vec![
                MdastNodeType::Break as u8,
                MdastNodeType::ThematicBreak as u8,
                MdastNodeType::Text as u8,
                MdastNodeType::Blockquote as u8,
                MdastNodeType::Paragraph as u8,
            ]
        );
    }

    /// An edit on a node no longer in the tree is dropped and reported, and the rest still applies.
    #[test]
    fn an_edit_on_a_node_that_left_the_tree_is_dropped_and_reported() {
        let mut orig = build_hello_world();
        let heading = orig.get_children(0)[0];
        let text_in_heading = orig.get_children(heading)[0];
        let para = orig.get_children(0)[1];
        let text_in_para = orig.get_children(para)[0];
        let orphan = orig.alloc_node(MdastNodeType::Paragraph as u8);
        let node = |node_type| PatchContent::Tree(single_node_arena(node_type));
        let remove = |node_id| Patch::Remove { node_id };
        let replace = |node_id, node_type| Patch::Replace {
            node_id,
            new_tree: node(node_type),
        };
        let append_break = |node_id| Patch::AppendChild {
            node_id,
            child_tree: node(MdastNodeType::Break),
        };
        // (patches, dropped nodes, root children left)
        let cases = [
            (
                vec![
                    remove(heading),
                    Patch::Wrap {
                        node_id: heading,
                        parent_tree: node(MdastNodeType::Blockquote),
                    },
                ],
                vec![heading],
                1,
            ),
            (
                vec![
                    remove(heading),
                    Patch::PrependChild {
                        node_id: heading,
                        child_tree: node(MdastNodeType::Break),
                    },
                ],
                vec![heading],
                1,
            ),
            (
                vec![remove(heading), append_break(heading)],
                vec![heading],
                1,
            ),
            (
                vec![
                    remove(heading),
                    Patch::InsertBefore {
                        node_id: text_in_heading,
                        new_tree: node(MdastNodeType::Break),
                    },
                ],
                vec![text_in_heading],
                1,
            ),
            (
                vec![
                    replace(heading, MdastNodeType::Paragraph),
                    replace(text_in_heading, MdastNodeType::Break),
                ],
                vec![text_in_heading],
                2,
            ),
            (
                vec![replace(0, MdastNodeType::Paragraph), append_break(heading)],
                vec![heading],
                0,
            ),
            (vec![remove(orphan), append_break(orphan)], vec![orphan], 2),
            (
                vec![
                    remove(heading),
                    remove(para),
                    replace(text_in_heading, MdastNodeType::Break),
                    Patch::InsertBefore {
                        node_id: text_in_para,
                        new_tree: node(MdastNodeType::Break),
                    },
                ],
                vec![text_in_heading, text_in_para],
                0,
            ),
        ];
        for (case, (patches, expected, root_children)) in cases.into_iter().enumerate() {
            let (arena, dropped) = rebuild_lenient(&orig, &patches).expect("rebuild failed");
            assert_eq!(dropped, expected, "case {case}");
            assert_eq!(arena.get_children(0).len(), root_children, "case {case}");
            match rebuild(&orig, &patches) {
                Err(CommandError::PatchOnRemovedSubtree(node)) => {
                    assert_eq!(node.id, expected[0], "case {case}")
                }
                other => panic!("case {case}: expected PatchOnRemovedSubtree, got {other:?}"),
            }
        }
    }

    /// A replacement naming the original child keeps it in the tree, so a later patch on it still applies.
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
            },
            Patch::Replace {
                node_id: text_in_heading,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Break)),
            },
        ];
        let (arena, dropped) = rebuild_lenient(&orig, &patches).expect("rebuild failed");
        assert!(
            dropped.is_empty(),
            "the named child stays in the tree: {dropped:?}"
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

    /// A replacement that keeps the node's children leaves them in the tree, so edits to them still apply.
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
                new_tree: PatchContent::Tree(with_kept_children(replacement)),
            },
            Patch::InsertBefore {
                node_id: text_in_heading,
                new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Break)),
            },
        ];
        let rebuilt = rebuild(&orig, &patches).expect("rebuild failed");
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
            },
        ] {
            let mut arena = orig.clone();
            assert!(matches!(
                apply_patches_strict(&mut arena, &[patch]),
                Err(CommandError::NodeInsideItself(_))
            ));
        }
    }

    /// Naming a node moves it from wherever it is, including from outside the tree.
    #[test]
    fn a_ref_to_a_detached_node_moves_it_in() {
        let mut orig = build_hello_world();
        let orphan = orig.alloc_node(MdastNodeType::Paragraph as u8);
        let para_id = orig.get_children(0)[1];

        let patches = vec![Patch::InsertAfter {
            node_id: para_id,
            new_tree: PatchContent::Tree(ref_arena(orphan, None)),
        }];
        let arena = rebuild(&orig, &patches).expect("rebuild failed");
        assert_eq!(arena.get_children(0)[2], orphan);
        assert_eq!(arena.get_node(orphan).parent, 0);
    }

    /// A ref past the end of the arena errors rather than indexing out of bounds.
    #[test]
    fn an_out_of_range_ref_errors() {
        let orig = build_hello_world();
        let para_id = orig.get_children(0)[1];
        let patches = vec![Patch::InsertAfter {
            node_id: para_id,
            new_tree: PatchContent::Tree(ref_arena(orig.len() as u32, None)),
        }];
        assert!(matches!(
            rebuild(&orig, &patches),
            Err(CommandError::InvalidNodeId(_))
        ));
    }

    /// `Replace` on the root: the replacement's shape lands on node 0 (the
    /// root id is stable), and the old subtree is discarded.
    #[test]
    fn replace_root_with_single_node() {
        let orig = build_hello_world();

        let patches = vec![Patch::Replace {
            node_id: 0,
            new_tree: PatchContent::Tree(single_node_arena(MdastNodeType::Paragraph)),
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

    /// A root replacement that keeps its children gives node 0 the payload's type and data.
    #[test]
    fn replace_root_keep_children() {
        let orig = build_hello_world();

        let patches = vec![Patch::Replace {
            node_id: 0,
            new_tree: PatchContent::Tree(with_kept_children(single_node_arena(
                MdastNodeType::Blockquote,
            ))),
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

    /// An edit on a kept child applies whether it comes before or after a root replacement keeping it.
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
                new_tree: PatchContent::Tree(with_kept_children(single_node_arena(
                    MdastNodeType::Blockquote,
                ))),
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

    /// `Remove` on the root empties the document: node 0 stays a root with no children.
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

    /// Replaces and inserts on several siblings land beside the right ones.
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

    /// A sibling inserted under a node is among the children a later edit of that node sees.
    #[test]
    fn an_insert_under_a_node_is_seen_by_its_later_child_edit() {
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

    /// A sibling inserted before a replacement that keeps the parent's children moves with them.
    #[test]
    fn an_insert_under_a_node_moves_with_its_kept_children() {
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
                new_tree: PatchContent::Tree(with_kept_children(single_node_arena(
                    MdastNodeType::Paragraph,
                ))),
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
