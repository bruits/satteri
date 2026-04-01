use std::collections::HashMap;

use crate::node::{MdastNode, MdastNodeType, StringRef};

/// The central arena that owns all nodes and associated data for one parse.
///
/// Strings are NOT copied — the arena holds the source and nodes reference it
/// via `StringRef` (byte offset + length into `source`).
#[derive(Debug, Clone)]
pub struct MdastArena {
    /// All nodes in order of creation.
    pub(crate) nodes: Vec<MdastNode>,
    /// Flat array of child node IDs, indexed by node.children_start..+children_count.
    pub(crate) children: Vec<u32>,
    /// Variable-length type-specific data, packed.
    pub(crate) type_data: Vec<u8>,
    pub(crate) source: String,
    /// Per-node `data` blobs (JSON bytes), set by JS plugins.
    pub(crate) node_data: HashMap<u32, Vec<u8>>,
}

impl MdastArena {
    pub fn new(source: String) -> Self {
        MdastArena {
            nodes: Vec::new(),
            children: Vec::new(),
            type_data: Vec::new(),
            source,
            node_data: HashMap::new(),
        }
    }

    /// The returned ID equals the node's index in `self.nodes`.
    pub fn alloc_node(&mut self, node_type: MdastNodeType) -> u32 {
        let id = self.nodes.len() as u32;
        self.nodes.push(MdastNode::new(id, node_type));
        id
    }

    /// For building HAST or other non-MDAST arenas that share the binary
    /// format, bypassing `MdastNodeType` validation.
    pub fn alloc_node_raw(&mut self, node_type_byte: u8) -> u32 {
        let id = self.nodes.len() as u32;
        let mut node = MdastNode::new(id, MdastNodeType::Root); // placeholder
        node.node_type = node_type_byte;
        self.nodes.push(node);
        id
    }

    pub fn set_parent(&mut self, node_id: u32, parent_id: u32) {
        self.nodes[node_id as usize].parent = parent_id;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_position(
        &mut self,
        node_id: u32,
        start_offset: u32,
        end_offset: u32,
        start_line: u32,
        start_column: u32,
        end_line: u32,
        end_column: u32,
    ) {
        let node = &mut self.nodes[node_id as usize];
        node.start_offset = start_offset;
        node.end_offset = end_offset;
        node.start_line = start_line;
        node.start_column = start_column;
        node.end_line = end_line;
        node.end_column = end_column;
    }

    /// Appends to the shared flat children array — calling this more than
    /// once on the same node orphans the previous entries.
    pub fn set_children(&mut self, node_id: u32, child_ids: &[u32]) {
        let start = self.children.len() as u32;
        self.children.extend_from_slice(child_ids);
        let node = &mut self.nodes[node_id as usize];
        node.children_start = start;
        node.children_count = child_ids.len() as u32;
        for &child_id in child_ids {
            self.nodes[child_id as usize].parent = node_id;
        }
    }

    pub fn set_type_data(&mut self, node_id: u32, data: &[u8]) {
        let offset = self.type_data.len() as u32;
        self.type_data.extend_from_slice(data);
        let node = &mut self.nodes[node_id as usize];
        node.data_offset = offset;
        node.data_len = data.len() as u32;
    }

    pub fn get_node(&self, node_id: u32) -> &MdastNode {
        &self.nodes[node_id as usize]
    }

    pub fn get_node_mut(&mut self, node_id: u32) -> &mut MdastNode {
        &mut self.nodes[node_id as usize]
    }

    pub fn get_children(&self, node_id: u32) -> &[u32] {
        let node = &self.nodes[node_id as usize];
        let start = node.children_start as usize;
        let end = start + node.children_count as usize;
        &self.children[start..end]
    }

    pub fn get_str(&self, string_ref: StringRef) -> &str {
        let start = string_ref.offset as usize;
        let end = start + string_ref.len as usize;
        &self.source[start..end]
    }

    pub fn get_node_data(&self, node_id: u32) -> Option<&[u8]> {
        self.node_data.get(&node_id).map(|v| v.as_slice())
    }

    pub fn set_node_data(&mut self, node_id: u32, data: Vec<u8>) {
        if data.is_empty() {
            self.node_data.remove(&node_id);
        } else {
            self.node_data.insert(node_id, data);
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// For computed strings not present verbatim in the source (e.g. decoded
    /// character references, normalised identifiers, synthesised alt text).
    pub fn alloc_string(&mut self, s: &str) -> StringRef {
        let offset = self.source.len() as u32;
        let len = s.len() as u32;
        self.source.push_str(s);
        StringRef::new(offset, len)
    }

    /// Used when we discover at close time that a Link/Image is actually
    /// a reference.
    pub fn change_node_type(&mut self, node_id: u32, new_type: MdastNodeType) {
        self.nodes[node_id as usize].node_type = new_type as u8;
    }

    /// Exposed for tests and the raw buffer layer.
    pub fn arena_type_data(&self) -> &[u8] {
        &self.type_data
    }

    pub fn get_type_data(&self, node_id: u32) -> &[u8] {
        let node = &self.nodes[node_id as usize];
        let start = node.data_offset as usize;
        let end = start + node.data_len as usize;
        &self.type_data[start..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_and_retrieve() {
        let mut arena = MdastArena::new("hello world".to_string());
        let id = arena.alloc_node(MdastNodeType::Text);
        assert_eq!(id, 0);
        assert_eq!(arena.len(), 1);
        let node = arena.get_node(id);
        assert_eq!(node.node_type, MdastNodeType::Text as u8);
    }

    #[test]
    fn set_position_roundtrip() {
        let mut arena = MdastArena::new(String::new());
        let id = arena.alloc_node(MdastNodeType::Paragraph);
        arena.set_position(id, 0, 10, 1, 1, 1, 11);
        let node = arena.get_node(id);
        assert_eq!(node.start_offset, 0);
        assert_eq!(node.end_offset, 10);
        assert_eq!(node.start_line, 1);
        assert_eq!(node.end_column, 11);
    }

    #[test]
    fn set_children_updates_parent() {
        let mut arena = MdastArena::new(String::new());
        let parent = arena.alloc_node(MdastNodeType::Paragraph);
        let child1 = arena.alloc_node(MdastNodeType::Text);
        let child2 = arena.alloc_node(MdastNodeType::Text);
        arena.set_children(parent, &[child1, child2]);
        assert_eq!(arena.get_children(parent), &[child1, child2]);
        assert_eq!(arena.get_node(child1).parent, parent);
        assert_eq!(arena.get_node(child2).parent, parent);
    }

    #[test]
    fn get_str_works() {
        let source = "Hello, world!".to_string();
        let arena = MdastArena::new(source);
        let sr = StringRef::new(7, 5);
        assert_eq!(arena.get_str(sr), "world");
    }

    #[test]
    fn type_data_roundtrip() {
        let mut arena = MdastArena::new(String::new());
        let id = arena.alloc_node(MdastNodeType::Heading);
        arena.set_type_data(id, &[2u8]);
        let node = arena.get_node(id);
        assert_eq!(node.data_len, 1);
        let stored = &arena.type_data[node.data_offset as usize..][..node.data_len as usize];
        assert_eq!(stored, &[2u8]);
    }
}
