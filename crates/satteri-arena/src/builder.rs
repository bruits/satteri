use crate::{ArenaKind, ArenaNode, Document, NodePosition, StringRef, TypeDataWriter};

/// Builder for an owned arena. Borrowed documents use the same builder and node layout.
pub type ArenaBuilder<K> = DocumentBuilder<'static, K>;

/// Collect children while constructing a tree in depth-first order.
pub struct DocumentBuilder<'a, K: ArenaKind> {
    arena: Document<'a, K>,
    stack: Vec<OpenNode>,
    pending_children: Vec<u32>,
}
// Each open node owns a suffix of pending_children. Closing it moves that
// suffix into the arena and appends its ID to its parent's pending children.
struct OpenNode {
    id: u32,
    children_start: u32,
}

impl<'a, K: ArenaKind> DocumentBuilder<'a, K> {
    pub fn new(source: String) -> Self {
        Self::from_arena(Document::new(source))
    }

    pub fn add_leaf(&mut self, node_type: u8) -> u32 {
        self.add_leaf_with_position(node_type, NodePosition::default(), &[])
    }

    pub fn begin_data_current(&mut self) -> TypeDataWriter {
        self.arena.begin_type_data(self.current_node_id())
    }

    pub fn finish_data_current(&mut self, writer: TypeDataWriter) {
        self.arena.finish_type_data(writer);
    }

    pub fn from_arena(arena: Document<'a, K>) -> Self {
        Self {
            pending_children: Vec::with_capacity(arena.children.capacity()),
            arena,
            stack: Vec::with_capacity(16),
        }
    }

    #[inline(always)]
    pub fn open_node(&mut self, node_type: u8) -> u32 {
        let id = self.arena.alloc_node(node_type);
        self.stack.push(OpenNode {
            id,
            children_start: self.pending_children.len() as u32,
        });
        id
    }

    #[inline(always)]
    pub fn open_node_with_position(
        &mut self,
        node_type: u8,
        position: NodePosition,
        data: &[u8],
    ) -> u32 {
        let parent = self.stack.last().map_or(u32::MAX, |node| node.id);
        let id = self.push_leaf(parent, node_type, position, data);
        self.stack.push(OpenNode {
            id,
            children_start: self.pending_children.len() as u32,
        });
        id
    }

    #[inline(always)]
    pub fn close_node(&mut self) -> u32 {
        let OpenNode { id, children_start } = self.stack.pop().expect("open node");
        let start = children_start as usize;
        let count = self.pending_children.len() - start;
        let node = &mut self.arena.nodes[id as usize];
        node.children_start = self.arena.children.len() as u32;
        node.children_count = count as u32;
        self.arena
            .children
            .extend_from_slice(&self.pending_children[start..]);
        self.pending_children.truncate(start);
        if let Some(parent) = self.stack.last() {
            self.arena.nodes[id as usize].parent = parent.id;
            self.pending_children.push(id);
        }
        id
    }

    #[inline(always)]
    pub fn add_leaf_with_position(
        &mut self,
        node_type: u8,
        position: NodePosition,
        data: &[u8],
    ) -> u32 {
        let parent = self.stack.last().map_or(u32::MAX, |node| node.id);
        let id = self.push_leaf(parent, node_type, position, data);
        if parent != u32::MAX {
            self.pending_children.push(id);
        }
        id
    }

    #[inline(always)]
    fn push_leaf(
        &mut self,
        parent: u32,
        node_type: u8,
        position: NodePosition,
        data: &[u8],
    ) -> u32 {
        let node_id = self.arena.nodes.len() as u32;
        let (data_offset, data_len) = if data.is_empty() {
            (0, 0)
        } else {
            let offset = self.arena.type_data.len() as u32;
            self.arena.type_data.extend_from_slice(data);
            self.arena.pad_type_data_tail(data.len());
            (offset, data.len() as u32)
        };
        self.arena.nodes.push(ArenaNode {
            id: node_id,
            node_type,
            _pad: [0; 3],
            parent,
            start_offset: position.start_offset,
            end_offset: position.end_offset,
            start_line: position.start_line,
            start_column: position.start_column,
            end_line: position.end_line,
            end_column: position.end_column,
            children_start: 0,
            children_count: 0,
            data_offset,
            data_len,
        });
        node_id
    }

    #[inline(always)]
    pub fn add_only_child_with_position(
        &mut self,
        parent: u32,
        node_type: u8,
        position: NodePosition,
        data: &[u8],
    ) -> u32 {
        assert_eq!(self.arena.nodes[parent as usize].children_count, 0);
        assert_ne!(self.stack.last().map(|node| node.id), Some(parent));
        let id = self.push_leaf(parent, node_type, position, data);
        let start = self.arena.children.len() as u32;
        self.arena.children.push(id);
        self.arena.nodes[parent as usize].children_start = start;
        self.arena.nodes[parent as usize].children_count = 1;
        id
    }

    #[inline(always)]
    pub fn set_position_current(&mut self, position: NodePosition) {
        let id = self.current_node_id();
        self.arena.set_position(id, position);
    }

    #[inline(always)]
    pub fn set_data_current(&mut self, data: &[u8]) {
        let id = self.current_node_id();
        self.arena.set_type_data(id, data);
    }

    pub fn alloc_string(&mut self, text: &str) -> StringRef {
        self.arena.alloc_string(text)
    }

    #[inline(always)]
    pub fn current_node_id(&self) -> u32 {
        self.stack.last().expect("open node").id
    }

    pub fn stack_depth(&self) -> usize {
        self.stack.len()
    }

    pub fn stack_node_id(&self, depth: usize) -> Option<u32> {
        self.stack.get(depth).map(|node| node.id)
    }

    #[inline(always)]
    pub fn last_sibling_id(&self) -> Option<u32> {
        self.current_pending_children().last().copied()
    }

    pub fn current_pending_children(&self) -> &[u32] {
        let start = self
            .stack
            .last()
            .map_or(0, |node| node.children_start as usize);
        &self.pending_children[start..]
    }

    pub fn sort_current_pending_children_by_source_order(&mut self) {
        let start = self
            .stack
            .last()
            .map_or(0, |node| node.children_start as usize);
        let arena = &self.arena;
        self.pending_children[start..].sort_by_key(|&id| {
            let node = arena.get_node(id);
            (node.end_offset, node.start_offset)
        });
    }

    pub fn update_leaf(&mut self, id: u32, position: NodePosition, data: &[u8]) {
        self.arena.set_position(id, position);
        self.arena.set_type_data(id, data);
    }

    pub fn arena_ref(&self) -> &Document<'a, K> {
        &self.arena
    }

    pub fn arena_mut(&mut self) -> &mut Document<'a, K> {
        &mut self.arena
    }

    pub fn finish(mut self) -> Document<'a, K> {
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
        builder.set_position_current(crate::NodePosition {
            start_offset: 0,
            end_offset: 5,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 6,
        });
        builder.set_data_current(&[42u8]);
        builder.close_node();
        let arena = builder.finish();
        let node = arena.get_node(id);
        assert_eq!(node.start_offset, 0);
        assert_eq!(node.end_offset, 5);
        assert_eq!(node.data_len, 1);
    }
}
