use std::borrow::Cow;
use std::marker::PhantomData;

use rustc_hash::FxHashMap;

use crate::kind::ArenaKind;
use crate::node::{ArenaNode, NodePosition, StringRef};

/// An owned document, suitable for plugin handles and wire serialization.
///
/// `K` is a phantom marker (`Mdast` or `Hast`) that distinguishes
/// otherwise-identical arenas at the type level so cross-kind misroutes are
/// caught at compile time. Container ops are generic over `K`; only code
/// that decodes `node_type` or the wire layout of `type_data` pins a kind.
///
/// This is the same storage as a borrowed [`Document`], with no source borrow.
pub type Arena<K> = Document<'static, K>;

/// Resolved tree with one node layout and either borrowed or owned source text.
/// String references address the logical source-plus-computed-string pool.
#[derive(Debug)]
pub struct Document<'a, K: ArenaKind> {
    borrowed_source: Option<&'a str>,
    /// All nodes in order of creation.
    pub nodes: Vec<ArenaNode>,
    /// Flat array of child node IDs, indexed by node.children_start..+children_count.
    pub children: Vec<u32>,
    /// Variable-length type-specific data, packed.
    pub type_data: Vec<u8>,
    /// Owned string storage. Contains only computed values while the source is
    /// borrowed, and source followed by computed values after `into_owned`.
    /// Use `get_str` to resolve logical offsets in either ownership mode.
    pub string_pool: String,
    /// Original input length, also the boundary between source and computed
    /// values in the logical pool. Independent of source ownership.
    pub source_len: u32,
    /// Per-node `data` blobs (JSON bytes), set by JS plugins.
    pub node_data: FxHashMap<u32, Vec<u8>>,
    /// Whether this arena was parsed in MDX mode.
    pub mdx: bool,
    /// The pulldown-cmark Options bits used to parse this arena.
    /// Stored so that re-parsing (e.g. after plugin mutations) uses the same options.
    pub parse_options: u32,
    /// Per-node `(utf16_start, utf16_end)` parallel to `nodes`. Populated by
    /// `arena_build` at end-of-parse (skipped for ASCII sources where the
    /// UTF-16 offset equals the byte offset). Read by `to_raw_buffer` and the
    /// walk to skip the second `LineIndex` build + per-node
    /// `byte_to_utf16_offset` lookup that would otherwise re-traverse the
    /// source. Empty means "not precomputed": readers fall back to live
    /// conversion.
    pub utf16_offsets: Vec<(u32, u32)>,
    pub(crate) _kind: PhantomData<fn() -> K>,
}

impl<K: ArenaKind> Default for Document<'_, K> {
    fn default() -> Self {
        Self::new(String::new())
    }
}

impl<K: ArenaKind> Clone for Document<'_, K> {
    fn clone(&self) -> Self {
        Document {
            borrowed_source: self.borrowed_source,
            nodes: self.nodes.clone(),
            children: self.children.clone(),
            type_data: self.type_data.clone(),
            string_pool: self.string_pool.clone(),
            source_len: self.source_len,
            node_data: self.node_data.clone(),
            mdx: self.mdx,
            parse_options: self.parse_options,
            utf16_offsets: self.utf16_offsets.clone(),
            _kind: PhantomData,
        }
    }
}

impl<'a, K: ArenaKind> Document<'a, K> {
    /// Construct an empty arena of the requested kind. Callers must declare
    /// the kind explicitly, e.g. `Arena::<Mdast>::new(source)` or via a
    /// type-annotated binding `let a: Arena<Hast> = Arena::new(source);`.
    pub fn new(source: String) -> Self {
        Self::with_capacity(source, 0, 0, 0)
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
        Document {
            borrowed_source: None,
            nodes: Vec::with_capacity(node_count),
            children: Vec::with_capacity(children_count),
            type_data: Vec::with_capacity(type_data_len),
            string_pool: source,
            source_len,
            node_data: FxHashMap::default(),
            mdx: false,
            parse_options: 0,
            utf16_offsets: Vec::new(),
            _kind: PhantomData,
        }
    }

    /// Clear all state but keep allocated capacity, for arena pooling. The
    /// caller must repopulate `source` and reset `mdx` / `parse_options` for
    /// the next document.
    pub fn reset(&mut self) {
        self.borrowed_source = None;
        self.source_len = 0;
        self.nodes.clear();
        self.children.clear();
        self.type_data.clear();
        self.string_pool.clear();
        self.node_data.clear();
        self.utf16_offsets.clear();
        self.mdx = false;
        self.parse_options = 0;
    }

    /// Allocate a new node. The returned ID equals the node's index in `self.nodes`.
    pub fn alloc_node(&mut self, node_type: u8) -> u32 {
        let id = self.nodes.len() as u32;
        self.nodes.push(ArenaNode::new(id, node_type));
        id
    }

    pub fn set_position(&mut self, node_id: u32, position: NodePosition) {
        let node = &mut self.nodes[node_id as usize];
        node.start_offset = position.start_offset;
        node.end_offset = position.end_offset;
        node.start_line = position.start_line;
        node.start_column = position.start_column;
        node.end_line = position.end_line;
        node.end_column = position.end_column;
        self.utf16_offsets.clear();
    }

    pub fn set_node_type(&mut self, id: u32, node_type: u8) {
        self.nodes[id as usize].node_type = node_type;
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

    /// Blob starts must stay 4-byte aligned: the JS readers take `Uint32Array` views over `type_data`.
    #[inline]
    pub fn pad_type_data_tail(&mut self, blob_len: usize) {
        let pad = blob_len.wrapping_neg() & 3;
        if pad != 0 {
            self.type_data.resize(self.type_data.len() + pad, 0);
        }
    }

    pub fn set_type_data(&mut self, node_id: u32, data: &[u8]) {
        let offset = self.type_data.len() as u32;
        debug_assert_eq!(offset & 3, 0);
        self.type_data.extend_from_slice(data);
        self.pad_type_data_tail(data.len());
        let node = &mut self.nodes[node_id as usize];
        node.data_offset = offset;
        node.data_len = data.len() as u32;
    }

    /// Begin writing variable-length type data for a node.
    /// Returns the start offset; call `finish_type_data` when done.
    pub fn begin_type_data(&mut self, node_id: u32) -> TypeDataWriter {
        let offset = self.type_data.len() as u32;
        debug_assert_eq!(offset & 3, 0);
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
        self.pad_type_data_tail(len as usize);
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

    pub fn get_str(&self, value: StringRef) -> &str {
        if value.is_empty() {
            return "";
        }
        let start = value.offset as usize;
        let end = start + value.len as usize;
        let source = self.borrowed_source.unwrap_or("");
        if start < source.len() {
            &source[start..end]
        } else {
            &self.string_pool[start - source.len()..end - source.len()]
        }
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

    /// The owned part of the string pool. For borrowed documents, use
    /// `append_pool` for the complete logical pool or `get_str` for a value.
    pub fn string_pool(&self) -> &str {
        &self.string_pool
    }

    /// The original input as written, without the `alloc_string` heap.
    pub fn source(&self) -> &str {
        self.borrowed_source
            .unwrap_or_else(|| &self.string_pool[..self.source_len as usize])
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
        let offset = self.pool_len() as u32;
        let len = s.len() as u32;
        self.string_pool.push_str(s);
        StringRef::new(offset, len)
    }

    /// Concatenate `prev` + `extra` in the pool without a temporary buffer;
    /// grows `prev` in place when it is the pool's tail.
    pub fn append_string(&mut self, prev: StringRef, extra: &str) -> StringRef {
        let source = self.borrowed_source.unwrap_or("");
        let pool_len = self.pool_len();
        let start = prev.offset as usize;
        let end = start + prev.len as usize;
        if start >= source.len() && end == pool_len {
            self.string_pool.push_str(extra);
            return StringRef::new(prev.offset, prev.len + extra.len() as u32);
        }
        if start < source.len() {
            self.string_pool.push_str(&source[start..end]);
        } else {
            self.string_pool
                .extend_from_within(start - source.len()..end - source.len());
        }
        self.string_pool.push_str(extra);
        StringRef::new(pool_len as u32, prev.len + extra.len() as u32)
    }

    /// Logical string pool length; references address source followed by computed strings.
    pub fn pool_len(&self) -> usize {
        self.borrowed_source.map_or(0, str::len) + self.string_pool.len()
    }

    /// Append the complete logical pool, including borrowed source text.
    pub fn append_pool(&self, out: &mut String) {
        out.push_str(self.borrowed_source.unwrap_or(""));
        out.push_str(&self.string_pool);
    }

    /// Contiguous logical pool for wire export. Owned documents need no copy.
    pub(crate) fn contiguous_pool(&self) -> Cow<'_, str> {
        match self.borrowed_source {
            None => Cow::Borrowed(&self.string_pool),
            Some(source) if self.string_pool.is_empty() => Cow::Borrowed(source),
            Some(_) => {
                let mut pool = String::with_capacity(self.pool_len());
                self.append_pool(&mut pool);
                Cow::Owned(pool)
            }
        }
    }

    /// Construct an empty document borrowing `source`, with capacity estimated
    /// for `count` nodes. Use `get_str`, not `string_pool`, to resolve references.
    pub fn borrowed(source: &'a str, count: usize) -> Self {
        let mut document = Self::with_capacity(String::new(), count, count, count * 8);
        document.borrowed_source = Some(source);
        document.source_len = source.len() as u32;
        document
    }

    /// Own the source without rebuilding nodes or changing string references.
    pub fn into_owned(mut self) -> Arena<K> {
        if let Some(source) = self.borrowed_source.take() {
            self.string_pool.insert_str(0, source);
        }
        Document {
            borrowed_source: None,
            ..self
        }
    }

    /// Clear per-document state, retain buffers, and release the source borrow.
    pub fn into_reusable(self) -> Arena<K> {
        self.rebind("")
    }

    /// Discard the tree and metadata, retaining buffers for a new source borrow.
    /// Unlike `into_owned`, this does not preserve nodes or string references.
    pub fn rebind<'b>(mut self, source: &'b str) -> Document<'b, K> {
        self.reset();
        self.source_len = source.len() as u32;
        Document {
            borrowed_source: Some(source),
            ..self
        }
    }

    /// Retained buffer capacity, excluding borrowed source bytes.
    pub fn retained_bytes(&self) -> usize {
        self.nodes.capacity() * size_of::<ArenaNode>()
            + self.string_pool.capacity()
            + self.children.capacity() * size_of::<u32>()
            + self.type_data.capacity()
            + self.utf16_offsets.capacity() * size_of::<(u32, u32)>()
            + self.node_data.capacity() * (size_of::<(u32, Vec<u8>)>() + 1)
            + self.node_data.values().map(Vec::capacity).sum::<usize>()
    }

    pub fn get_type_data(&self, node_id: u32) -> &[u8] {
        let node = &self.nodes[node_id as usize];
        let start = node.data_offset as usize;
        let end = start + node.data_len as usize;
        &self.type_data[start..end]
    }

    /// For same-length fixups; `set_type_data` appends a copy and orphans the old bytes.
    pub fn get_type_data_mut(&mut self, node_id: u32) -> &mut [u8] {
        let node = &self.nodes[node_id as usize];
        let start = node.data_offset as usize;
        let end = start + node.data_len as usize;
        &mut self.type_data[start..end]
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
    use crate::Hast;
    use crate::kind::Mdast;

    #[test]
    fn borrowed_values_survive_ownership_and_rebinding_for_both_kinds() {
        fn check<K: ArenaKind>(text_tag: u8) {
            let source = String::from("α text");
            let mut document = Document::<K>::borrowed(&source, 2);
            let literal = StringRef::new(0, 2);
            let decoded = document.append_string(literal, "!");
            let decoded = document.append_string(decoded, "?");
            let node = document.alloc_node(text_tag);
            document.set_type_data(node, &decoded.as_bytes());
            document.set_node_data(node, br#"{"old":true}"#.to_vec());
            document.utf16_offsets.push((0, 1));
            document.mdx = true;
            document.parse_options = 42;
            assert_eq!(document.get_str(literal), "α");
            assert_eq!(document.get_str(decoded), "α!?");
            assert_eq!(document.source(), source);

            let wire = document.to_raw_buffer();
            let owned = document.into_owned();
            drop(source);
            assert_eq!(owned.to_raw_buffer(), wire);
            assert_eq!(owned.source(), "α text");
            assert_eq!(owned.get_str(decoded), "α!?");
            assert_eq!(owned.get_type_data(node), decoded.as_bytes());
            assert_eq!(
                owned.get_node_data(node),
                Some(br#"{"old":true}"#.as_slice())
            );
            assert_eq!(owned.utf16_offsets, [(0, 1)]);
            assert!(owned.mdx);
            assert_eq!(owned.parse_options, 42);

            let mut reused = owned.rebind("β");
            assert!(reused.is_empty());
            assert!(reused.node_data.is_empty());
            assert!(reused.utf16_offsets.is_empty());
            assert!(!reused.mdx);
            assert_eq!(reused.parse_options, 0);
            let value = reused.append_string(StringRef::new(0, 2), " new");
            let owned = reused.into_owned();
            assert_eq!(owned.source(), "β");
            assert_eq!(owned.get_str(value), "β new");
        }
        // Text tags from the registry; both codecs contain one StringRef.
        check::<Mdast>(10);
        check::<Hast>(2);
    }

    #[test]
    fn alloc_and_retrieve() {
        let mut arena: Arena<Mdast> = Arena::new("hello world".to_string());
        let id = arena.alloc_node(0);
        assert_eq!(id, 0);
        assert_eq!(arena.len(), 1);
        let node = arena.get_node(id);
        assert_eq!(node.node_type, 0);
    }

    /// The exhaustive destructure makes adding an `Arena` field without deciding its reset behavior a compile error.
    #[test]
    fn reset_clears_every_per_document_field() {
        let mut arena: Arena<Mdast> = Arena::new("junk source".to_string());
        let parent = arena.alloc_node(1);
        let child = arena.alloc_node(2);
        arena.set_children(parent, &[child]);
        arena.set_type_data(parent, &[9, 9, 9, 9]);
        arena.alloc_string("interned junk");
        arena.set_node_data(child, vec![1, 2, 3]);
        arena.utf16_offsets.push((7, 9));
        arena.mdx = true;
        arena.parse_options = 0xDEAD_BEEF;
        arena.source_len = 77;

        arena.reset();

        let Document {
            borrowed_source,
            nodes,
            children,
            type_data,
            string_pool,
            source_len,
            node_data,
            mdx,
            parse_options,
            utf16_offsets,
            _kind: _,
        } = &arena;
        assert!(nodes.is_empty());
        assert!(children.is_empty());
        assert!(type_data.is_empty());
        assert!(string_pool.is_empty());
        assert!(node_data.is_empty());
        assert!(utf16_offsets.is_empty());
        assert!(!mdx);
        assert_eq!(*parse_options, 0);
        assert!(borrowed_source.is_none());
        assert_eq!(*source_len, 0);
        assert!(arena.nodes.capacity() >= 2, "reset must keep capacity");
        assert!(arena.string_pool.capacity() >= "junk source".len());
    }

    #[test]
    fn set_position_roundtrip() {
        let mut arena: Arena<Mdast> = Arena::new(String::new());
        let id = arena.alloc_node(0);
        arena.set_position(
            id,
            NodePosition {
                start_offset: 0,
                end_offset: 10,
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 11,
            },
        );
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
