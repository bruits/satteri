use std::marker::PhantomData;

use rustc_hash::FxHashMap;

use crate::kind::ArenaKind;
use crate::node::{ArenaNode, StringRef};

/// The central arena that owns all nodes and associated data for one parse.
///
/// `K` is a phantom marker (`Mdast` or `Hast`) that distinguishes
/// otherwise-identical arenas at the type level so cross-kind misroutes are
/// caught at compile time. Container ops are generic over `K`; only code
/// that decodes `node_type` or the wire layout of `type_data` pins a kind.
///
/// Strings are NOT copied. The arena holds the source and nodes reference it
/// via `StringRef` (byte offset + length into `source`).
#[derive(Debug)]
pub struct Arena<K: ArenaKind> {
    /// All nodes in order of creation.
    pub nodes: Vec<ArenaNode>,
    /// Flat array of child node IDs, indexed by node.children_start..+children_count.
    pub children: Vec<u32>,
    /// Variable-length type-specific data, packed.
    pub type_data: Vec<u8>,
    /// The original input followed by a string-interning heap: `alloc_string`
    /// appends computed strings (decoded entities, image alt/url, code values)
    /// past the input so every `StringRef` is a uniform offset+len.
    /// `source_len` marks the boundary; for the input as written use `source()`.
    pub string_pool: String,
    /// Byte length of the original-input prefix of `string_pool`. Set at
    /// construction; preserved across rebuild and mdast→hast conversion, which
    /// copy the full pool to keep `StringRef` offsets valid.
    pub source_len: u32,
    /// Per-node `data` blobs (JSON bytes), set by JS plugins.
    pub node_data: FxHashMap<u32, Vec<u8>>,
    /// Whether this arena was parsed in MDX mode.
    pub mdx: bool,
    /// The pulldown-cmark Options bits used to parse this arena.
    /// Stored so that re-parsing (e.g. after plugin mutations) uses the same options.
    pub parse_options: u32,
    /// Per-node `(cp_start, cp_end)` parallel to `nodes`. Populated by
    /// `arena_build` at end-of-parse (skipped for ASCII sources where
    /// cp == byte). Read by `to_raw_buffer` to skip the second
    /// `LineIndex` build + per-node `byte_to_cp_offset` lookup that
    /// would otherwise re-traverse the source. Empty means "not
    /// precomputed" — readers fall back to live conversion.
    pub cp_offsets: Vec<(u32, u32)>,
    pub(crate) _kind: PhantomData<fn() -> K>,
}

impl<K: ArenaKind> Clone for Arena<K> {
    fn clone(&self) -> Self {
        Arena {
            nodes: self.nodes.clone(),
            children: self.children.clone(),
            type_data: self.type_data.clone(),
            string_pool: self.string_pool.clone(),
            source_len: self.source_len,
            node_data: self.node_data.clone(),
            mdx: self.mdx,
            parse_options: self.parse_options,
            cp_offsets: self.cp_offsets.clone(),
            _kind: PhantomData,
        }
    }
}

impl<K: ArenaKind> Arena<K> {
    /// Construct an empty arena of the requested kind. Callers must declare
    /// the kind explicitly, e.g. `Arena::<Mdast>::new(source)` or via a
    /// type-annotated binding `let a: Arena<Hast> = Arena::new(source);`.
    pub fn new(source: String) -> Self {
        let source_len = source.len() as u32;
        Arena {
            nodes: Vec::new(),
            children: Vec::new(),
            type_data: Vec::new(),
            string_pool: source,
            source_len,
            node_data: FxHashMap::default(),
            mdx: false,
            parse_options: 0,
            cp_offsets: Vec::new(),
            _kind: PhantomData,
        }
    }

    /// Construct an empty arena of the requested kind with pre-allocated
    /// capacity for nodes, children, and type data.
    pub fn with_capacity(
        source: String,
        node_count: usize,
        children_count: usize,
        type_data_len: usize,
    ) -> Self {
        let source_len = source.len() as u32;
        Arena {
            nodes: Vec::with_capacity(node_count),
            children: Vec::with_capacity(children_count),
            type_data: Vec::with_capacity(type_data_len),
            string_pool: source,
            source_len,
            node_data: FxHashMap::default(),
            mdx: false,
            parse_options: 0,
            cp_offsets: Vec::new(),
            _kind: PhantomData,
        }
    }

    /// Allocate a new node. The returned ID equals the node's index in `self.nodes`.
    pub fn alloc_node(&mut self, node_type: u8) -> u32 {
        let id = self.nodes.len() as u32;
        self.nodes.push(ArenaNode::new(id, node_type));
        id
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
        // Position mutation invalidates the cached byte→cp offsets.
        self.cp_offsets.clear();
    }

    /// Appends to the shared flat children array, calling this more than
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

    /// Begin writing variable-length type data for a node.
    /// Returns the start offset; call `finish_type_data` when done.
    pub fn begin_type_data(&mut self, node_id: u32) -> TypeDataWriter {
        let offset = self.type_data.len() as u32;
        self.nodes[node_id as usize].data_offset = offset;
        TypeDataWriter {
            node_id,
            start: offset,
        }
    }

    /// Finish writing variable-length type data started by `begin_type_data`.
    pub fn finish_type_data(&mut self, writer: TypeDataWriter) {
        let len = self.type_data.len() as u32 - writer.start;
        self.nodes[writer.node_id as usize].data_len = len;
    }

    pub fn get_node(&self, node_id: u32) -> &ArenaNode {
        &self.nodes[node_id as usize]
    }

    pub fn get_node_mut(&mut self, node_id: u32) -> &mut ArenaNode {
        &mut self.nodes[node_id as usize]
    }

    pub fn get_children(&self, node_id: u32) -> &[u32] {
        let node = &self.nodes[node_id as usize];
        let start = node.children_start as usize;
        let end = start + node.children_count as usize;
        &self.children[start..end]
    }

    pub fn replace_node_with_children(&mut self, node_id: u32, replacement_children: &[u32]) {
        let parent_id = self.nodes[node_id as usize].parent;
        let parent_children: Vec<u32> = self.get_children(parent_id).to_vec();
        let mut new_children =
            Vec::with_capacity(parent_children.len() + replacement_children.len());
        for &child_id in &parent_children {
            if child_id == node_id {
                new_children.extend_from_slice(replacement_children);
            } else {
                new_children.push(child_id);
            }
        }
        self.set_children(parent_id, &new_children);
    }

    pub fn get_str(&self, string_ref: StringRef) -> &str {
        let start = string_ref.offset as usize;
        let end = start + string_ref.len as usize;
        &self.string_pool[start..end]
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

    /// The full pool (input + `alloc_string` heap), the buffer `StringRef`s
    /// index into. Most callers want `get_str`; for the input as written,
    /// use `source()`.
    pub fn string_pool(&self) -> &str {
        &self.string_pool
    }

    /// The original input as written, without the `alloc_string` heap.
    pub fn source(&self) -> &str {
        &self.string_pool[..self.source_len as usize]
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
        let offset = self.string_pool.len() as u32;
        let len = s.len() as u32;
        self.string_pool.push_str(s);
        StringRef::new(offset, len)
    }

    pub fn get_type_data(&self, node_id: u32) -> &[u8] {
        let node = &self.nodes[node_id as usize];
        let start = node.data_offset as usize;
        let end = start + node.data_len as usize;
        &self.type_data[start..end]
    }
}

/// Handle for tracking in-progress variable-length type data writes.
pub struct TypeDataWriter {
    pub(crate) node_id: u32,
    pub(crate) start: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::Mdast;

    #[test]
    fn alloc_and_retrieve() {
        let mut arena: Arena<Mdast> = Arena::new("hello world".to_string());
        let id = arena.alloc_node(0);
        assert_eq!(id, 0);
        assert_eq!(arena.len(), 1);
        let node = arena.get_node(id);
        assert_eq!(node.node_type, 0);
    }

    #[test]
    fn set_position_roundtrip() {
        let mut arena: Arena<Mdast> = Arena::new(String::new());
        let id = arena.alloc_node(0);
        arena.set_position(id, 0, 10, 1, 1, 1, 11);
        let node = arena.get_node(id);
        assert_eq!(node.start_offset, 0);
        assert_eq!(node.end_offset, 10);
        assert_eq!(node.start_line, 1);
        assert_eq!(node.end_column, 11);
    }

    #[test]
    fn set_children_updates_parent() {
        let mut arena: Arena<Mdast> = Arena::new(String::new());
        let parent = arena.alloc_node(0);
        let child1 = arena.alloc_node(0);
        let child2 = arena.alloc_node(0);
        arena.set_children(parent, &[child1, child2]);
        assert_eq!(arena.get_children(parent), &[child1, child2]);
        assert_eq!(arena.get_node(child1).parent, parent);
        assert_eq!(arena.get_node(child2).parent, parent);
    }

    #[test]
    fn get_str_works() {
        let source = "Hello, world!".to_string();
        let arena: Arena<Mdast> = Arena::new(source);
        let sr = StringRef::new(7, 5);
        assert_eq!(arena.get_str(sr), "world");
    }

    #[test]
    fn alloc_string_does_not_leak_into_source() {
        let mut arena: Arena<Mdast> = Arena::new("# Hello".to_string());
        let sr = arena.alloc_string("synthesised");
        assert_eq!(arena.get_str(sr), "synthesised");
        assert_eq!(arena.string_pool(), "# Hellosynthesised");
        assert_eq!(arena.source(), "# Hello");
    }

    #[test]
    fn type_data_roundtrip() {
        let mut arena: Arena<Mdast> = Arena::new(String::new());
        let id = arena.alloc_node(0);
        arena.set_type_data(id, &[2u8]);
        let node = arena.get_node(id);
        assert_eq!(node.data_len, 1);
        let stored = &arena.type_data[node.data_offset as usize..][..node.data_len as usize];
        assert_eq!(stored, &[2u8]);
    }
}
