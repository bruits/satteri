//! Storage-independent read access used by semantic conversion and rendering.

use rustc_hash::FxHashMap;

use crate::{Arena, ArenaKind, ArenaNode, StringRef};

/// Read an already-resolved tree without requiring a particular node or string pool layout.
/// String references must resolve for the lifetime of the borrowed view.
pub trait ArenaRead<K: ArenaKind> {
    fn get_node(&self, id: u32) -> ArenaNode;
    fn get_children(&self, id: u32) -> &[u32];
    fn get_type_data(&self, id: u32) -> &[u8];
    fn get_str(&self, value: StringRef) -> &str;
    fn len(&self) -> usize;
    fn pool_len(&self) -> usize;
    fn source_len(&self) -> u32;
    /// Append the logical pool without changing the offsets of its references.
    fn append_pool(&self, out: &mut String);
    fn node_data(&self) -> &FxHashMap<u32, Vec<u8>>;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn get_node_data(&self, id: u32) -> Option<&[u8]> {
        self.node_data().get(&id).map(Vec::as_slice)
    }
}

impl<K: ArenaKind> ArenaRead<K> for Arena<K> {
    #[inline(always)]
    fn get_node(&self, id: u32) -> ArenaNode {
        *self.get_node(id)
    }
    fn get_children(&self, id: u32) -> &[u32] {
        self.get_children(id)
    }
    fn get_type_data(&self, id: u32) -> &[u8] {
        self.get_type_data(id)
    }
    fn get_str(&self, value: StringRef) -> &str {
        self.get_str(value)
    }
    fn len(&self) -> usize {
        self.len()
    }
    fn pool_len(&self) -> usize {
        self.string_pool.len()
    }
    fn source_len(&self) -> u32 {
        self.source_len
    }
    fn append_pool(&self, out: &mut String) {
        out.push_str(&self.string_pool);
    }
    fn node_data(&self) -> &FxHashMap<u32, Vec<u8>> {
        &self.node_data
    }
}

// Existing arena consumers also accept references obtained through Box, Arc,
// and lock guards. Preserve those deref-coercion call sites for generic readers.
impl<K: ArenaKind, T: core::ops::Deref> ArenaRead<K> for T
where
    T::Target: ArenaRead<K>,
{
    #[inline(always)]
    fn get_node(&self, id: u32) -> ArenaNode {
        self.deref().get_node(id)
    }
    fn get_children(&self, id: u32) -> &[u32] {
        self.deref().get_children(id)
    }
    fn get_type_data(&self, id: u32) -> &[u8] {
        self.deref().get_type_data(id)
    }
    fn get_str(&self, value: StringRef) -> &str {
        self.deref().get_str(value)
    }
    fn len(&self) -> usize {
        self.deref().len()
    }
    fn pool_len(&self) -> usize {
        self.deref().pool_len()
    }
    fn source_len(&self) -> u32 {
        self.deref().source_len()
    }
    fn append_pool(&self, out: &mut String) {
        self.deref().append_pool(out);
    }
    fn node_data(&self) -> &FxHashMap<u32, Vec<u8>> {
        self.deref().node_data()
    }
}
