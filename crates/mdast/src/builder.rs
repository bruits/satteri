use crate::arena::MdastArena;
use crate::node::{MdastNodeType, StringRef};

/// Builds an `MdastArena` using an open/close node pattern suitable for
/// depth-first tree construction (e.g. SAX-style parsers).
pub struct MdastBuilder {
    arena: MdastArena,
    /// Stack of `(node_id, children_collected_so_far)`.
    stack: Vec<(u32, Vec<u32>)>,
}

impl MdastBuilder {
    pub fn new(source: String) -> Self {
        MdastBuilder {
            arena: MdastArena::new(source),
            stack: Vec::new(),
        }
    }

    pub fn open_node(&mut self, node_type: MdastNodeType) -> u32 {
        let node_id = self.arena.alloc_node(node_type);
        self.stack.push((node_id, Vec::new()));
        node_id
    }

    pub fn close_node(&mut self) -> u32 {
        let (node_id, children) = self
            .stack
            .pop()
            .expect("close_node called with empty stack");

        self.arena.set_children(node_id, &children);

        if let Some((parent_id, parent_children)) = self.stack.last_mut() {
            parent_children.push(node_id);
            self.arena.set_parent(node_id, *parent_id);
        }

        node_id
    }

    pub fn add_leaf(&mut self, node_type: MdastNodeType) -> u32 {
        self.open_node(node_type);
        self.close_node()
    }

    /// Bypasses the `MdastNodeType` enum for building HAST or other non-MDAST
    /// arenas that share the same binary format.
    pub fn open_node_raw(&mut self, node_type_byte: u8) -> u32 {
        let node_id = self.arena.alloc_node_raw(node_type_byte);
        self.stack.push((node_id, Vec::new()));
        node_id
    }

    pub fn add_leaf_raw(&mut self, node_type_byte: u8) -> u32 {
        self.open_node_raw(node_type_byte);
        self.close_node()
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

    pub fn alloc_string(&mut self, s: &str) -> StringRef {
        self.arena.alloc_string(s)
    }

    /// For reclassifying nodes during parsing (e.g. Link → LinkReference).
    pub fn change_node_type(&mut self, node_id: u32, new_type: MdastNodeType) {
        self.arena.change_node_type(node_id, new_type);
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

    pub fn arena_ref(&self) -> &MdastArena {
        &self.arena
    }

    pub fn current_children_mut(&mut self) -> &mut Vec<u32> {
        &mut self.stack.last_mut().expect("empty stack").1
    }

    pub fn arena_mut(&mut self) -> &mut MdastArena {
        &mut self.arena
    }

    /// Auto-closes any remaining open nodes before returning the arena.
    pub fn finish(mut self) -> MdastArena {
        while !self.stack.is_empty() {
            self.close_node();
        }
        self.arena
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::MdastNodeType;

    #[test]
    fn simple_open_close() {
        let mut builder = MdastBuilder::new("# Hello".to_string());
        let root = builder.open_node(MdastNodeType::Root);
        let heading = builder.open_node(MdastNodeType::Heading);
        let text = builder.add_leaf(MdastNodeType::Text);
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
        let mut builder = MdastBuilder::new(String::new());
        builder.open_node(MdastNodeType::Root);
        builder.open_node(MdastNodeType::Paragraph);
        builder.add_leaf(MdastNodeType::Text);
        // Do NOT close explicitly — finish() should handle it.
        let arena = builder.finish();
        assert_eq!(arena.len(), 3);
    }

    #[test]
    fn leaf_has_no_children() {
        let mut builder = MdastBuilder::new(String::new());
        builder.open_node(MdastNodeType::Root);
        let leaf = builder.add_leaf(MdastNodeType::Break);
        builder.close_node();
        let arena = builder.finish();
        assert_eq!(arena.get_children(leaf), &[] as &[u32]);
    }

    #[test]
    fn position_and_data_current() {
        let mut builder = MdastBuilder::new("hello".to_string());
        let id = builder.open_node(MdastNodeType::Text);
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
