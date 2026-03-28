//! `ReadMdast` — a trait for read-only access to an arena.
//!
//! Implemented by both `MdastArena` (owned) and `MdastView<'_>` (zero-copy view
//! over a raw buffer). Code that only needs to read the tree (e.g. HAST
//! conversion, HTML serialization) can be generic over this trait.

use crate::node::{MdastNode, StringRef};

pub trait ReadMdast {
    fn get_node(&self, node_id: u32) -> &MdastNode;
    fn get_children(&self, node_id: u32) -> &[u32];
    fn get_type_data(&self, node_id: u32) -> &[u8];
    fn get_str(&self, string_ref: StringRef) -> &str;
    fn source(&self) -> &str;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl ReadMdast for crate::arena::MdastArena {
    fn get_node(&self, node_id: u32) -> &MdastNode {
        self.get_node(node_id)
    }
    fn get_children(&self, node_id: u32) -> &[u32] {
        self.get_children(node_id)
    }
    fn get_type_data(&self, node_id: u32) -> &[u8] {
        self.get_type_data(node_id)
    }
    fn get_str(&self, string_ref: StringRef) -> &str {
        self.get_str(string_ref)
    }
    fn source(&self) -> &str {
        self.source()
    }
    fn len(&self) -> usize {
        self.len()
    }
}

impl ReadMdast for crate::raw_buffer::MdastView<'_> {
    fn get_node(&self, node_id: u32) -> &MdastNode {
        self.get_node(node_id)
    }
    fn get_children(&self, node_id: u32) -> &[u32] {
        self.get_children(node_id)
    }
    fn get_type_data(&self, node_id: u32) -> &[u8] {
        self.get_type_data(node_id)
    }
    fn get_str(&self, string_ref: StringRef) -> &str {
        self.get_str(string_ref)
    }
    fn source(&self) -> &str {
        self.get_source()
    }
    fn len(&self) -> usize {
        self.node_count() as usize
    }
}
