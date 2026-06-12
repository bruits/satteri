use std::mem::size_of;

/// A reference into the source string, no allocation, just offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
pub struct StringRef {
    pub offset: u32,
    pub len: u32,
}

impl StringRef {
    pub fn new(offset: u32, len: u32) -> Self {
        Self { offset, len }
    }

    pub fn empty() -> Self {
        Self { offset: 0, len: 0 }
    }

    pub fn is_empty(self) -> bool {
        self.len == 0
    }

    /// Return the raw bytes of this StringRef (8 bytes, little-endian — the
    /// declared byte order for all in-arena scalars).
    pub fn as_bytes(self) -> [u8; 8] {
        let mut buf = [0u8; 8];
        buf[0..4].copy_from_slice(&self.offset.to_le_bytes());
        buf[4..8].copy_from_slice(&self.len.to_le_bytes());
        buf
    }

    /// Read a StringRef from raw bytes (inverse of `as_bytes`).
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            offset: u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            len: u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        }
    }
}

/// All positions use byte offsets and 1-based line/column numbers from the
/// source text.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ArenaNode {
    pub id: u32,
    pub node_type: u8,
    pub _pad: [u8; 3],
    pub parent: u32,
    pub start_offset: u32,
    pub end_offset: u32,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    /// Index into Arena::children where this node's children start.
    pub children_start: u32,
    pub children_count: u32,
    /// Byte offset into Arena::type_data for this node's extra data.
    pub data_offset: u32,
    pub data_len: u32,
}

pub const NODE_STRUCT_SIZE: usize = size_of::<ArenaNode>();

impl ArenaNode {
    pub fn new(id: u32, node_type: u8) -> Self {
        ArenaNode {
            id,
            node_type,
            _pad: [0u8; 3],
            parent: u32::MAX, // sentinel: no parent
            start_offset: 0,
            end_offset: 0,
            start_line: 0,
            start_column: 0,
            end_line: 0,
            end_column: 0,
            children_start: 0,
            children_count: 0,
            data_offset: 0,
            data_len: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arena_node_size_pinned() {
        assert_eq!(
            size_of::<ArenaNode>(),
            52,
            "ArenaNode size changed, update NODE_STRUCT_SIZE callers"
        );
    }

    #[test]
    fn string_ref_is_8_bytes() {
        assert_eq!(size_of::<StringRef>(), 8);
    }
}
