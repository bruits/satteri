use std::marker::PhantomData;

use crate::arena::{Arena, TypeDataWriter};
use crate::kind::ArenaKind;
use crate::node::StringRef;

/// Builds an `Arena<K>` using an open/close node pattern suitable for
/// depth-first tree construction (e.g. SAX-style parsers).
///
/// `K` matches the kind of the inner arena; container ops are generic over
/// `K`, so most builder code is unaware of the marker.
pub struct ArenaBuilder<K: ArenaKind> {
    arena: Arena<K>,
    /// Stack of `(node_id, children_start_in_pending)`.
    stack: Vec<(u32, u32)>,
    /// Flat buffer collecting child IDs for all open nodes.
    /// Each stack frame's children are `pending[children_start..]` when it's the top frame.
    pending_children: Vec<u32>,
    _kind: PhantomData<fn() -> K>,
}

impl<K: ArenaKind> ArenaBuilder<K> {
    /// Construct an empty builder of the requested kind. Callers must
    /// declare the kind explicitly, e.g. `ArenaBuilder::<Mdast>::new(s)`.
    pub fn new(source: String) -> Self {
        ArenaBuilder {
            arena: Arena::<K>::new(source),
            stack: Vec::new(),
            pending_children: Vec::new(),
            _kind: PhantomData,
        }
    }

    /// Create a builder wrapping a pre-allocated arena.
    pub fn from_arena(arena: Arena<K>) -> Self {
        let cap = arena.children.capacity();
        ArenaBuilder {
            arena,
            stack: Vec::with_capacity(16),
            pending_children: Vec::with_capacity(cap),
            _kind: PhantomData,
        }
    }

    /// Create a builder with pre-allocated capacity based on an existing arena's size.
    pub fn with_capacity_from<L: ArenaKind>(source: String, hint: &Arena<L>) -> Self {
        ArenaBuilder {
            arena: Arena::<K>::with_capacity(
                source,
                hint.nodes.len(),
                hint.children.len(),
                hint.type_data.len(),
            ),
            stack: Vec::with_capacity(16),
            pending_children: Vec::with_capacity(hint.children.len()),
            _kind: PhantomData,
        }
    }

    pub fn open_node(&mut self, node_type: u8) -> u32 {
        let node_id = self.arena.alloc_node(node_type);
        let start = self.pending_children.len() as u32;
        self.stack.push((node_id, start));
        node_id
    }

    /// Alias for `open_node`, kept for call-site clarity in HAST code.
    pub fn open_node_raw(&mut self, node_type: u8) -> u32 {
        self.open_node(node_type)
    }

    pub fn close_node(&mut self) -> u32 {
        let (node_id, children_start) = self
            .stack
            .pop()
            .expect("close_node called with empty stack");

        let children_start = children_start as usize;
        let children = &self.pending_children[children_start..];

        // Copy children to the arena's flat array.
        let arena_start = self.arena.children.len() as u32;
        self.arena.children.extend_from_slice(children);
        let node = &mut self.arena.nodes[node_id as usize];
        node.children_start = arena_start;
        node.children_count = (self.pending_children.len() - children_start) as u32;
        for i in children_start..self.pending_children.len() {
            self.arena.nodes[self.pending_children[i] as usize].parent = node_id;
        }

        // Truncate the pending buffer back to where this frame started.
        self.pending_children.truncate(children_start);

        // Register as child of parent.
        if let Some((parent_id, _)) = self.stack.last() {
            self.arena.nodes[node_id as usize].parent = *parent_id;
            self.pending_children.push(node_id);
        }

        node_id
    }

    /// Add a leaf node without the overhead of a full open/close cycle.
    pub fn add_leaf(&mut self, node_type: u8) -> u32 {
        let node_id = self.arena.alloc_node(node_type);

        // Register as child of parent directly.
        if let Some((parent_id, _)) = self.stack.last() {
            self.arena.nodes[node_id as usize].parent = *parent_id;
            self.pending_children.push(node_id);
        }

        node_id
    }

    /// Alias for `add_leaf`, kept for call-site clarity in HAST code.
    pub fn add_leaf_raw(&mut self, node_type: u8) -> u32 {
        self.add_leaf(node_type)
    }

    /// Add a leaf node with position and type data in one call (avoids repeated node lookups).
    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub fn add_leaf_full(
        &mut self,
        node_type: u8,
        start_offset: u32,
        end_offset: u32,
        start_line: u32,
        start_column: u32,
        end_line: u32,
        end_column: u32,
        data: &[u8],
    ) -> u32 {
        let node_id = self.arena.alloc_node(node_type);

        // Set position directly.
        let node = &mut self.arena.nodes[node_id as usize];
        node.start_offset = start_offset;
        node.end_offset = end_offset;
        node.start_line = start_line;
        node.start_column = start_column;
        node.end_line = end_line;
        node.end_column = end_column;

        // Set type data.
        if !data.is_empty() {
            let offset = self.arena.type_data.len() as u32;
            self.arena.type_data.extend_from_slice(data);
            let node = &mut self.arena.nodes[node_id as usize];
            node.data_offset = offset;
            node.data_len = data.len() as u32;
        }

        // Register as child of parent.
        if let Some((parent_id, _)) = self.stack.last() {
            self.arena.nodes[node_id as usize].parent = *parent_id;
            self.pending_children.push(node_id);
        }

        node_id
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_position_current(
        &mut self,
        start_offset: u32,
        end_offset: u32,
        start_line: u32,
        start_column: u32,
        end_line: u32,
        end_column: u32,
    ) {
        let node_id = self
            .stack
            .last()
            .expect("set_position_current called with empty stack")
            .0;
        self.arena.set_position(
            node_id,
            start_offset,
            end_offset,
            start_line,
            start_column,
            end_line,
            end_column,
        );
    }

    pub fn set_data_current(&mut self, data: &[u8]) {
        let node_id = self
            .stack
            .last()
            .expect("set_data_current called with empty stack")
            .0;
        self.arena.set_type_data(node_id, data);
    }

    /// Begin writing variable-length type data for the current node.
    /// Write bytes directly to `self.arena_mut().type_data`, then call `finish_data_current`.
    pub fn begin_data_current(&mut self) -> TypeDataWriter {
        let node_id = self
            .stack
            .last()
            .expect("begin_data_current called with empty stack")
            .0;
        self.arena.begin_type_data(node_id)
    }

    /// Finish writing variable-length type data.
    pub fn finish_data_current(&mut self, writer: TypeDataWriter) {
        self.arena.finish_type_data(writer);
    }

    pub fn alloc_string(&mut self, s: &str) -> StringRef {
        self.arena.alloc_string(s)
    }

    pub fn current_node_id(&self) -> u32 {
        self.stack
            .last()
            .expect("current_node_id called with empty stack")
            .0
    }

    pub fn stack_depth(&self) -> usize {
        self.stack.len()
    }

    /// Index 0 is the bottom of the stack (root).
    pub fn stack_node_id(&self, depth: usize) -> Option<u32> {
        self.stack.get(depth).map(|(id, _)| *id)
    }

    pub fn last_sibling_id(&self) -> Option<u32> {
        let children_start = self.stack.last().map(|(_, cs)| *cs as usize).unwrap_or(0);
        if self.pending_children.len() > children_start {
            self.pending_children.last().copied()
        } else {
            None
        }
    }

    /// Children already collected for the node currently on top of the stack.
    /// Unlike `Arena::get_children`, this works before the node is closed.
    pub fn current_pending_children(&self) -> &[u32] {
        let children_start = self.stack.last().map(|(_, cs)| *cs as usize).unwrap_or(0);
        &self.pending_children[children_start..]
    }

    /// Stable-sort the current node's pending children into source order,
    /// keyed by `(end_offset, start_offset)`. End is the faithful proxy
    /// because some block starts get extended *back* (e.g. a setext
    /// heading inheriting a preceding definition's start).
    pub fn sort_current_pending_children_by_source_order(&mut self) {
        let children_start = self.stack.last().map(|(_, cs)| *cs as usize).unwrap_or(0);
        let nodes = &self.arena.nodes;
        self.pending_children[children_start..].sort_by_key(|&id| {
            let n = &nodes[id as usize];
            (n.end_offset, n.start_offset)
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_leaf_full(
        &mut self,
        node_id: u32,
        start_offset: u32,
        end_offset: u32,
        start_line: u32,
        start_column: u32,
        end_line: u32,
        end_column: u32,
        data: &[u8],
    ) {
        let node = &mut self.arena.nodes[node_id as usize];
        node.start_offset = start_offset;
        node.end_offset = end_offset;
        node.start_line = start_line;
        node.start_column = start_column;
        node.end_line = end_line;
        node.end_column = end_column;
        node.data_offset = self.arena.type_data.len() as u32;
        node.data_len = data.len() as u32;
        self.arena.type_data.extend_from_slice(data);
    }

    pub fn arena_ref(&self) -> &Arena<K> {
        &self.arena
    }

    pub fn arena_mut(&mut self) -> &mut Arena<K> {
        &mut self.arena
    }

    /// Auto-closes any remaining open nodes before returning the arena.
    pub fn finish(mut self) -> Arena<K> {
        while !self.stack.is_empty() {
            self.close_node();
        }
        self.arena
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_open_close() {
        let mut builder: ArenaBuilder<crate::kind::Mdast> =
            ArenaBuilder::new("# Hello".to_string());
        let root = builder.open_node(0);
        let heading = builder.open_node(2);
        let text = builder.add_leaf(10);
        let heading_closed = builder.close_node();
        let root_closed = builder.close_node();
        assert_eq!(heading_closed, heading);
        assert_eq!(root_closed, root);

        let arena = builder.finish();
        assert_eq!(arena.len(), 3);
        assert_eq!(arena.get_children(root), &[heading]);
        assert_eq!(arena.get_children(heading), &[text]);
        assert_eq!(arena.get_node(text).parent, heading);
        assert_eq!(arena.get_node(heading).parent, root);
    }

    #[test]
    fn finish_closes_open_nodes() {
        let mut builder: ArenaBuilder<crate::kind::Mdast> = ArenaBuilder::new(String::new());
        builder.open_node(0);
        builder.open_node(1);
        builder.add_leaf(10);
        // Do NOT close explicitly. finish() should handle it.
        let arena = builder.finish();
        assert_eq!(arena.len(), 3);
    }

    #[test]
    fn leaf_has_no_children() {
        let mut builder: ArenaBuilder<crate::kind::Mdast> = ArenaBuilder::new(String::new());
        builder.open_node(0);
        let leaf = builder.add_leaf(14);
        builder.close_node();
        let arena = builder.finish();
        assert_eq!(arena.get_children(leaf), &[] as &[u32]);
    }

    #[test]
    fn position_and_data_current() {
        let mut builder: ArenaBuilder<crate::kind::Mdast> = ArenaBuilder::new("hello".to_string());
        let id = builder.open_node(10);
        builder.set_position_current(0, 5, 1, 1, 1, 6);
        builder.set_data_current(&[42u8]);
        builder.close_node();
        let arena = builder.finish();
        let node = arena.get_node(id);
        assert_eq!(node.start_offset, 0);
        assert_eq!(node.end_offset, 5);
        assert_eq!(node.data_len, 1);
    }
}
