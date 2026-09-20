//! Source-backed semantic document built by the existing grammar traversal.
//! Syntax records remain transient; semantic records have stable, dense IDs.
use rustc_hash::FxHashMap;
use satteri_arena::ArenaRead;
use satteri_arena::{Arena, ArenaNode, Mdast, NodePosition, NodeStore, StringRef};

/// Resolve a document using the same grammar and semantic passes as owned arenas.
pub fn parse(
    source: &str,
    options: crate::Options,
    track_positions: bool,
) -> (SourceDocument<'_>, Vec<(usize, String)>) {
    crate::arena_build::parse_document(source, options, track_positions, false, None)
}

/// Rebind empty-source reusable storage without retaining the previous input.
pub fn parse_reusing<'a>(
    source: &'a str,
    options: crate::Options,
    track_positions: bool,
    storage: Option<SourceDocument<'static>>,
) -> (SourceDocument<'a>, Vec<(usize, String)>) {
    crate::arena_build::parse_document(source, options, track_positions, false, storage)
}

/// Resolved Markdown with borrowed source text and owned computed values.
/// Materialization preserves node IDs and positions without reparsing.
#[derive(Debug)]
pub struct SourceDocument<'a> {
    nodes: NodeStore,
    materialized_nodes: Vec<ArenaNode>,
    source: &'a str,
    pub(crate) extra: String,
    pub(crate) children: Vec<u32>,
    pub(crate) type_data: Vec<u8>,
    pub(crate) node_data: FxHashMap<u32, Vec<u8>>,
    pub(crate) utf16_offsets: Vec<(u32, u32)>,
    pub(crate) parse_options: u32,
}
impl<'a> SourceDocument<'a> {
    pub(crate) fn new(source: &'a str, count: usize) -> Self {
        Self {
            source,
            nodes: NodeStore::with_capacity(count),
            materialized_nodes: Vec::new(),
            extra: String::new(),
            children: Vec::with_capacity(count),
            type_data: Vec::with_capacity(count * 8),
            node_data: FxHashMap::default(),
            utf16_offsets: Vec::new(),
            parse_options: 0,
        }
    }
    pub(crate) fn from_arena(source: &'a str, mut arena: Arena<Mdast>) -> Self {
        arena.reset();
        Self {
            source,
            nodes: NodeStore::new(),
            materialized_nodes: arena.nodes,
            extra: arena.string_pool,
            children: arena.children,
            type_data: arena.type_data,
            node_data: arena.node_data,
            utf16_offsets: arena.utf16_offsets,
            parse_options: 0,
        }
    }
    pub(crate) fn ensure_node_capacity(&mut self, estimate: impl FnOnce() -> usize) {
        // Owned-arena reuse keeps the public node buffer, not a second compact
        // high-water mark. Size fresh compact storage for this input instead.
        if self.nodes.capacity() == 0 {
            self.nodes.reserve(estimate());
        }
    }
    /// Own the source and expand compact records into the public arena layout.
    pub fn into_materialized(mut self) -> Arena<Mdast> {
        self.extra.insert_str(0, self.source);
        let mut out = Arena::<Mdast>::new(self.extra);
        self.materialized_nodes.extend(self.nodes.iter());
        out.nodes = self.materialized_nodes;
        out.children = self.children;
        out.type_data = self.type_data;
        out.node_data = self.node_data;
        out.utf16_offsets = self.utf16_offsets;
        out.source_len = self.source.len() as u32;
        out.parse_options = self.parse_options;
        out.mdx = self.parse_options & crate::Options::ENABLE_MDX.bits() != 0;
        out
    }
    pub fn into_reusable(self) -> SourceDocument<'static> {
        self.rebind("")
    }
    pub(crate) fn rebind<'b>(self, source: &'b str) -> SourceDocument<'b> {
        let Self {
            mut nodes,
            mut materialized_nodes,
            mut extra,
            mut children,
            mut type_data,
            mut node_data,
            mut utf16_offsets,
            ..
        } = self;
        nodes.clear();
        materialized_nodes.clear();
        extra.clear();
        children.clear();
        type_data.clear();
        node_data.clear();
        utf16_offsets.clear();
        SourceDocument {
            source,
            nodes,
            materialized_nodes,
            extra,
            children,
            type_data,
            node_data,
            utf16_offsets,
            parse_options: 0,
        }
    }
    pub fn retained_bytes(&self) -> usize {
        self.nodes.storage_bytes()
            + self.materialized_nodes.capacity() * size_of::<ArenaNode>()
            + self.extra.capacity()
            + self.children.capacity() * size_of::<u32>()
            + self.type_data.capacity()
            + self.utf16_offsets.capacity() * size_of::<(u32, u32)>()
            + self.node_data.capacity() * (size_of::<(u32, Vec<u8>)>() + 1)
            + self.node_data.values().map(Vec::capacity).sum::<usize>()
    }
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
    #[inline(always)]
    pub fn get_node(&self, id: u32) -> ArenaNode {
        self.nodes.get(id as usize)
    }
    #[inline(always)]
    pub fn set_node(&mut self, node: ArenaNode) {
        *self.nodes.edit(node.id as usize) = node;
    }
    #[inline(always)]
    pub fn get_node_mut(&mut self, id: u32) -> satteri_arena::node_store::NodeEdit<'_> {
        self.nodes.edit(id as usize)
    }
    #[inline(always)]
    pub fn alloc_node(&mut self, node_type: u8) -> u32 {
        let id = self.nodes.len() as u32;
        self.nodes.push(ArenaNode::new(id, node_type));
        id
    }
    #[inline(always)]
    pub fn get_children(&self, id: u32) -> &[u32] {
        let node = self.nodes.get(id as usize);
        let start = node.children_start as usize;
        &self.children[start..start + node.children_count as usize]
    }
    #[inline(always)]
    fn set_parent(&mut self, id: u32, parent: u32) {
        self.nodes.set_parent(id as usize, parent);
    }
    pub fn set_children(&mut self, id: u32, children: &[u32]) {
        self.nodes.set_children_range(
            id as usize,
            self.children.len() as u32,
            children.len() as u32,
        );
        self.children.extend_from_slice(children);
        for &child in children {
            self.set_parent(child, id);
        }
    }
    pub fn replace_node_with_children(&mut self, id: u32, replacement: &[u32]) {
        let parent = self.get_node(id).parent;
        let old = self.get_children(parent);
        let mut new = Vec::with_capacity(old.len() + replacement.len());
        for &child in old {
            if child == id {
                new.extend_from_slice(replacement);
            } else {
                new.push(child);
            }
        }
        self.set_children(parent, &new);
    }
    #[inline(always)]
    pub fn get_type_data(&self, id: u32) -> &[u8] {
        let n = self.get_node(id);
        &self.type_data[n.data_offset as usize..(n.data_offset + n.data_len) as usize]
    }
    #[inline(always)]
    pub fn get_type_data_mut(&mut self, id: u32) -> &mut [u8] {
        let n = self.get_node(id);
        &mut self.type_data[n.data_offset as usize..(n.data_offset + n.data_len) as usize]
    }
    pub fn set_type_data(&mut self, id: u32, bytes: &[u8]) {
        self.nodes
            .set_data_range(id as usize, self.type_data.len() as u32, bytes.len() as u32);
        self.type_data.extend_from_slice(bytes);
        self.type_data
            .resize(self.type_data.len().next_multiple_of(4), 0);
    }
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    pub fn set_position(
        &mut self,
        id: u32,
        start: u32,
        end: u32,
        sl: u32,
        sc: u32,
        el: u32,
        ec: u32,
    ) {
        self.nodes.set_position(
            id as usize,
            NodePosition {
                start_offset: start,
                end_offset: end,
                start_line: sl,
                start_column: sc,
                end_line: el,
                end_column: ec,
            },
        );
    }
    #[inline(always)]
    pub fn get_str(&self, sr: StringRef) -> &str {
        if sr.len == 0 {
            return "";
        }
        let start = sr.offset as usize;
        let end = start + sr.len as usize;
        if start < self.source.len() {
            &self.source[start..end]
        } else {
            &self.extra[start - self.source.len()..end - self.source.len()]
        }
    }
    pub fn alloc_string(&mut self, text: &str) -> StringRef {
        let sr = StringRef::new(
            (self.source.len() + self.extra.len()) as u32,
            text.len() as u32,
        );
        self.extra.push_str(text);
        sr
    }
    pub fn append_string(&mut self, previous: StringRef, extra: &str) -> StringRef {
        if previous.offset as usize >= self.source.len()
            && (previous.offset + previous.len) as usize == self.source.len() + self.extra.len()
        {
            self.extra.push_str(extra);
            return StringRef::new(previous.offset, previous.len + extra.len() as u32);
        }
        let offset = (self.source.len() + self.extra.len()) as u32;
        let start = previous.offset as usize;
        let end = start + previous.len as usize;
        if start < self.source.len() {
            self.extra.push_str(&self.source[start..end]);
        } else {
            self.extra
                .extend_from_within(start - self.source.len()..end - self.source.len());
        }
        self.extra.push_str(extra);
        StringRef::new(offset, previous.len + extra.len() as u32)
    }
    pub fn set_node_data(&mut self, id: u32, bytes: Vec<u8>) {
        self.node_data.insert(id, bytes);
    }
    pub fn get_node_data(&self, id: u32) -> Option<&[u8]> {
        self.node_data.get(&id).map(Vec::as_slice)
    }
    /// Materialize without reparsing or remapping IDs. All records are semantic;
    /// source and computed-string offsets remain unchanged.
    pub fn materialize(&self) -> Arena<Mdast> {
        self.materialize_into(Arena::<Mdast>::new(String::new()))
    }
    pub fn materialize_into(&self, mut out: Arena<Mdast>) -> Arena<Mdast> {
        out.reset();
        out.string_pool.push_str(self.source);
        out.string_pool.push_str(&self.extra);
        out.source_len = self.source.len() as u32;
        out.parse_options = self.parse_options;
        out.mdx = self.parse_options & crate::Options::ENABLE_MDX.bits() != 0;
        out.type_data.extend_from_slice(&self.type_data);
        out.nodes.reserve(self.len());
        for id in 0..self.len() {
            out.nodes.push(self.get_node(id as u32));
        }
        out.children.extend_from_slice(&self.children);
        out.node_data.clone_from(&self.node_data);
        out.utf16_offsets.extend_from_slice(&self.utf16_offsets);
        out
    }
}
impl ArenaRead<Mdast> for SourceDocument<'_> {
    #[inline(always)]
    fn get_node(&self, id: u32) -> ArenaNode {
        self.get_node(id)
    }
    #[inline(always)]
    fn get_children(&self, id: u32) -> &[u32] {
        self.get_children(id)
    }
    #[inline(always)]
    fn get_type_data(&self, id: u32) -> &[u8] {
        self.get_type_data(id)
    }
    #[inline(always)]
    fn get_str(&self, sr: StringRef) -> &str {
        self.get_str(sr)
    }
    #[inline(always)]
    fn len(&self) -> usize {
        self.len()
    }
    #[inline(always)]
    fn source_len(&self) -> u32 {
        self.source.len() as u32
    }
    #[inline(always)]
    fn append_pool(&self, out: &mut String) {
        out.push_str(self.source);
        out.push_str(&self.extra);
    }
    #[inline(always)]
    fn pool_len(&self) -> usize {
        self.source.len() + self.extra.len()
    }
    #[inline(always)]
    fn node_data(&self) -> &FxHashMap<u32, Vec<u8>> {
        &self.node_data
    }
}
pub(crate) struct DocumentBuilder<'a> {
    pub arena: SourceDocument<'a>,
    stack: Vec<(u32, u32)>,
    pending_children: Vec<u32>,
}
impl<'a> DocumentBuilder<'a> {
    pub fn from_arena(arena: SourceDocument<'a>) -> Self {
        Self {
            pending_children: Vec::with_capacity(arena.children.capacity()),
            arena,
            stack: Vec::with_capacity(16),
        }
    }
    #[inline(always)]
    pub fn open_node(&mut self, tag: u8) -> u32 {
        let id = self.arena.alloc_node(tag);
        self.stack.push((id, self.pending_children.len() as u32));
        id
    }
    #[inline(always)]
    pub fn open_node_with_position(&mut self, tag: u8, position: NodePosition, data: &[u8]) -> u32 {
        let parent = self.stack.last().map_or(u32::MAX, |v| v.0);
        let id = self.push_leaf(parent, tag, position, data);
        self.stack.push((id, self.pending_children.len() as u32));
        id
    }
    #[inline(always)]
    pub fn close_node(&mut self) -> u32 {
        let (id, start) = self.stack.pop().expect("open node");
        let start = start as usize;
        let count = self.pending_children.len() - start;
        self.arena.nodes.set_children_range(
            id as usize,
            self.arena.children.len() as u32,
            count as u32,
        );
        self.arena
            .children
            .extend_from_slice(&self.pending_children[start..]);
        self.pending_children.truncate(start);
        if let Some(&(parent, _)) = self.stack.last() {
            self.arena.set_parent(id, parent);
            self.pending_children.push(id);
        }
        id
    }
    #[inline(always)]
    pub fn add_leaf_with_position(&mut self, tag: u8, p: NodePosition, data: &[u8]) -> u32 {
        let parent = self.stack.last().map_or(u32::MAX, |v| v.0);
        let id = self.push_leaf(parent, tag, p, data);
        if parent != u32::MAX {
            self.pending_children.push(id);
        }
        id
    }
    #[inline(always)]
    fn push_leaf(&mut self, parent: u32, tag: u8, p: NodePosition, data: &[u8]) -> u32 {
        let offset = self.arena.type_data.len() as u32;
        self.arena.type_data.extend_from_slice(data);
        self.arena
            .type_data
            .resize(self.arena.type_data.len().next_multiple_of(4), 0);
        self.arena
            .nodes
            .push_leaf(tag, parent, p, offset, data.len() as u32)
    }
    #[inline(always)]
    pub fn add_only_child_with_position(
        &mut self,
        parent: u32,
        tag: u8,
        p: NodePosition,
        data: &[u8],
    ) -> u32 {
        assert_eq!(self.arena.nodes.children_count(parent as usize), 0);
        assert_ne!(self.stack.last().map(|v| v.0), Some(parent));
        let id = self.push_leaf(parent, tag, p, data);
        let start = self.arena.children.len() as u32;
        self.arena.children.push(id);
        self.arena
            .nodes
            .set_children_range(parent as usize, start, 1);
        id
    }
    #[inline(always)]
    pub fn set_position_current(&mut self, s: u32, e: u32, sl: u32, sc: u32, el: u32, ec: u32) {
        let id = self.current_node_id();
        self.arena.set_position(id, s, e, sl, sc, el, ec);
    }
    #[inline(always)]
    pub fn set_data_current(&mut self, data: &[u8]) {
        let id = self.current_node_id();
        self.arena.set_type_data(id, data);
    }
    pub fn alloc_string(&mut self, s: &str) -> StringRef {
        self.arena.alloc_string(s)
    }
    #[inline(always)]
    pub fn current_node_id(&self) -> u32 {
        self.stack.last().expect("open node").0
    }
    pub fn stack_depth(&self) -> usize {
        self.stack.len()
    }
    pub fn stack_node_id(&self, depth: usize) -> Option<u32> {
        self.stack.get(depth).map(|v| v.0)
    }
    #[inline(always)]
    pub fn last_sibling_id(&self) -> Option<u32> {
        let start = self.stack.last().map_or(0, |v| v.1 as usize);
        (self.pending_children.len() > start).then(|| *self.pending_children.last().unwrap())
    }
    pub fn current_pending_children(&self) -> &[u32] {
        let start = self.stack.last().map_or(0, |v| v.1 as usize);
        &self.pending_children[start..]
    }
    pub fn sort_current_pending_children_by_source_order(&mut self) {
        let start = self.stack.last().map_or(0, |v| v.1 as usize);
        let arena = &self.arena;
        self.pending_children[start..].sort_by_key(|&id| {
            let n = arena.get_node(id);
            (n.end_offset, n.start_offset)
        });
    }
    #[allow(clippy::too_many_arguments)]
    pub fn update_leaf_full(
        &mut self,
        id: u32,
        s: u32,
        e: u32,
        sl: u32,
        sc: u32,
        el: u32,
        ec: u32,
        data: &[u8],
    ) {
        self.arena.set_position(id, s, e, sl, sc, el, ec);
        self.arena.set_type_data(id, data);
    }
    pub fn arena_ref(&self) -> &SourceDocument<'a> {
        &self.arena
    }
    pub fn arena_mut(&mut self) -> &mut SourceDocument<'a> {
        &mut self.arena
    }
    pub fn finish(mut self) -> SourceDocument<'a> {
        while !self.stack.is_empty() {
            self.close_node();
        }
        self.arena
    }
}
