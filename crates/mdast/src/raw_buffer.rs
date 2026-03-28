//! Raw buffer export/import for zero-copy transfer.
//!
//! Wire format: `[BufferHeader][nodes...][children u32s][type_data bytes][source UTF-8]`

use crate::arena::MdastArena;
use crate::node::{MdastNode, StringRef, NODE_STRUCT_SIZE};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BufferError {
    TooShort,
    BadMagic,
    VersionMismatch,
    NodeSizeMismatch,
    InvalidUtf8,
    OutOfBounds,
}

impl std::fmt::Display for BufferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BufferError::TooShort => write!(f, "buffer too short"),
            BufferError::BadMagic => write!(f, "bad magic bytes"),
            BufferError::VersionMismatch => write!(f, "version mismatch"),
            BufferError::NodeSizeMismatch => write!(f, "MdastNode size mismatch"),
            BufferError::InvalidUtf8 => write!(f, "source is not valid UTF-8"),
            BufferError::OutOfBounds => write!(f, "offset out of bounds"),
        }
    }
}

impl std::error::Error for BufferError {}

pub const BUFFER_MAGIC: [u8; 4] = *b"MDAR";
pub const BUFFER_VERSION: u32 = 1;

/// Wire-format header placed at the very start of the exported buffer.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct BufferHeader {
    pub magic: [u8; 4],
    pub version: u32,
    pub node_struct_size: u32,
    pub node_count: u32,
    pub nodes_offset: u32,
    pub children_count: u32,
    pub children_offset: u32,
    pub type_data_len: u32,
    pub type_data_offset: u32,
    pub source_len: u32,
    pub source_offset: u32,
}

const HEADER_SIZE: usize = std::mem::size_of::<BufferHeader>();

impl MdastArena {
    /// Serialize to a flat byte buffer:
    /// `[BufferHeader][nodes][children u32s][type_data][source]`
    pub fn to_raw_buffer(&self) -> Vec<u8> {
        let nodes_bytes = self.nodes.len() * NODE_STRUCT_SIZE;
        let children_bytes = self.children.len() * 4;
        let type_data_bytes = self.type_data.len();
        let source_bytes = self.source.len();

        let nodes_offset = HEADER_SIZE as u32;
        let children_offset = nodes_offset + nodes_bytes as u32;
        let type_data_offset = children_offset + children_bytes as u32;
        let source_offset = type_data_offset + type_data_bytes as u32;

        let header = BufferHeader {
            magic: BUFFER_MAGIC,
            version: BUFFER_VERSION,
            node_struct_size: NODE_STRUCT_SIZE as u32,
            node_count: self.nodes.len() as u32,
            nodes_offset,
            children_count: self.children.len() as u32,
            children_offset,
            type_data_len: self.type_data.len() as u32,
            type_data_offset,
            source_len: self.source.len() as u32,
            source_offset,
        };

        let total = source_offset as usize + source_bytes;
        let mut buf = Vec::with_capacity(total);

        let header_bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(&header as *const BufferHeader as *const u8, HEADER_SIZE)
        };
        buf.extend_from_slice(header_bytes);

        let nodes_slice: &[u8] =
            unsafe { std::slice::from_raw_parts(self.nodes.as_ptr() as *const u8, nodes_bytes) };
        buf.extend_from_slice(nodes_slice);

        let children_slice: &[u8] = unsafe {
            std::slice::from_raw_parts(self.children.as_ptr() as *const u8, children_bytes)
        };
        buf.extend_from_slice(children_slice);

        buf.extend_from_slice(&self.type_data);

        buf.extend_from_slice(self.source.as_bytes());

        buf
    }

    /// Deserialize from a raw buffer into an `MdastView` (read-only, borrows
    /// the buffer).
    pub fn from_raw_buffer(buf: &[u8]) -> Result<MdastView<'_>, BufferError> {
        if buf.len() < HEADER_SIZE {
            return Err(BufferError::TooShort);
        }
        // Read header by copy (it's small).
        let header: BufferHeader = unsafe {
            let mut h = std::mem::MaybeUninit::<BufferHeader>::uninit();
            std::ptr::copy_nonoverlapping(buf.as_ptr(), h.as_mut_ptr() as *mut u8, HEADER_SIZE);
            h.assume_init()
        };

        if header.magic != BUFFER_MAGIC {
            return Err(BufferError::BadMagic);
        }
        if header.version != BUFFER_VERSION {
            return Err(BufferError::VersionMismatch);
        }
        if header.node_struct_size as usize != NODE_STRUCT_SIZE {
            return Err(BufferError::NodeSizeMismatch);
        }

        let nodes_end =
            header.nodes_offset as usize + header.node_count as usize * NODE_STRUCT_SIZE;
        let children_end = header.children_offset as usize + header.children_count as usize * 4;
        let type_data_end = header.type_data_offset as usize + header.type_data_len as usize;
        let source_end = header.source_offset as usize + header.source_len as usize;

        if nodes_end > buf.len()
            || children_end > buf.len()
            || type_data_end > buf.len()
            || source_end > buf.len()
        {
            return Err(BufferError::OutOfBounds);
        }

        let source_bytes = &buf[header.source_offset as usize..source_end];
        std::str::from_utf8(source_bytes).map_err(|_| BufferError::InvalidUtf8)?;

        Ok(MdastView { header, buf })
    }
}

/// A read-only view over a raw buffer produced by `MdastArena::to_raw_buffer`.
pub struct MdastView<'a> {
    pub(crate) header: BufferHeader,
    pub(crate) buf: &'a [u8],
}

impl std::fmt::Debug for MdastView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MdastView")
            .field("node_count", &self.header.node_count)
            .field("buf_len", &self.buf.len())
            .finish()
    }
}

impl<'a> MdastView<'a> {
    pub fn get_node(&self, node_id: u32) -> &MdastNode {
        assert!(
            (node_id as usize) < self.header.node_count as usize,
            "node_id out of range"
        );
        let offset = self.header.nodes_offset as usize + node_id as usize * NODE_STRUCT_SIZE;
        unsafe { &*(self.buf[offset..].as_ptr() as *const MdastNode) }
    }

    /// Get all child IDs for a node as a `&[u32]`.
    pub fn get_children(&self, node_id: u32) -> &[u32] {
        let node = self.get_node(node_id);
        let start = node.children_start as usize;
        let count = node.children_count as usize;
        if count == 0 {
            return &[];
        }
        let byte_start = self.header.children_offset as usize + start * 4;
        let byte_end = byte_start + count * 4;
        let bytes = &self.buf[byte_start..byte_end];
        unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const u32, count) }
    }

    pub fn get_type_data(&self, node_id: u32) -> &[u8] {
        let node = self.get_node(node_id);
        let start = self.header.type_data_offset as usize + node.data_offset as usize;
        let end = start + node.data_len as usize;
        &self.buf[start..end]
    }

    pub fn get_source(&self) -> &str {
        let start = self.header.source_offset as usize;
        let end = start + self.header.source_len as usize;
        // Safety: validated in from_raw_buffer
        unsafe { std::str::from_utf8_unchecked(&self.buf[start..end]) }
    }

    pub fn get_str(&self, string_ref: StringRef) -> &str {
        let source = self.get_source();
        let start = string_ref.offset as usize;
        let end = start + string_ref.len as usize;
        &source[start..end]
    }

    pub fn node_count(&self) -> u32 {
        self.header.node_count
    }

    /// Convert this read-only view into an owned `MdastArena` by copying all data.
    pub fn to_arena(&self) -> MdastArena {
        let node_count = self.header.node_count as usize;
        let nodes_start = self.header.nodes_offset as usize;
        let nodes: Vec<MdastNode> = (0..node_count)
            .map(|i| {
                let offset = nodes_start + i * NODE_STRUCT_SIZE;
                let mut node = std::mem::MaybeUninit::<MdastNode>::uninit();
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        self.buf[offset..].as_ptr(),
                        node.as_mut_ptr() as *mut u8,
                        NODE_STRUCT_SIZE,
                    );
                    node.assume_init()
                }
            })
            .collect();

        let children_count = self.header.children_count as usize;
        let children_start = self.header.children_offset as usize;
        let children: Vec<u32> = (0..children_count)
            .map(|i| {
                let offset = children_start + i * 4;
                u32::from_ne_bytes(self.buf[offset..offset + 4].try_into().unwrap())
            })
            .collect();

        let td_start = self.header.type_data_offset as usize;
        let td_len = self.header.type_data_len as usize;
        let type_data = self.buf[td_start..td_start + td_len].to_vec();

        let source = self.get_source().to_string();

        MdastArena {
            nodes,
            children,
            type_data,
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::MdastBuilder;
    use crate::node::MdastNodeType;

    fn simple_arena() -> MdastArena {
        let mut builder = MdastBuilder::new("Hello, world!".to_string());
        builder.open_node(MdastNodeType::Root);
        builder.open_node(MdastNodeType::Paragraph);
        builder.add_leaf(MdastNodeType::Text);
        builder.close_node();
        builder.close_node();
        builder.finish()
    }

    #[test]
    fn header_magic_and_version() {
        let arena = simple_arena();
        let buf = arena.to_raw_buffer();
        assert_eq!(&buf[..4], b"MDAR");
        // version at offset 4 (after magic[4])
        let version = u32::from_ne_bytes(buf[4..8].try_into().unwrap());
        assert_eq!(version, BUFFER_VERSION);
    }

    #[test]
    fn round_trip_node_count() {
        let arena = simple_arena();
        let buf = arena.to_raw_buffer();
        let view = MdastArena::from_raw_buffer(&buf).unwrap();
        assert_eq!(view.node_count(), arena.len() as u32);
    }

    #[test]
    fn bad_magic_rejected() {
        let arena = simple_arena();
        let mut buf = arena.to_raw_buffer();
        buf[0] = b'X';
        let err = MdastArena::from_raw_buffer(&buf).unwrap_err();
        assert_eq!(err, BufferError::BadMagic);
    }

    #[test]
    fn round_trip_source() {
        let arena = simple_arena();
        let buf = arena.to_raw_buffer();
        let view = MdastArena::from_raw_buffer(&buf).unwrap();
        assert_eq!(view.get_source(), "Hello, world!");
    }

    #[test]
    fn round_trip_children() {
        let arena = simple_arena();
        let original_children: Vec<u32> = arena.get_children(0).to_vec();
        let buf = arena.to_raw_buffer();
        let view = MdastArena::from_raw_buffer(&buf).unwrap();
        assert_eq!(view.get_children(0), original_children.as_slice());
    }
}
