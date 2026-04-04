//! Raw buffer export for zero-copy transfer.
//!
//! Wire format: `[Header][nodes...][children u32s][type_data bytes][source UTF-8]`

use crate::arena::Arena;
use crate::node::NODE_STRUCT_SIZE;

const BUFFER_MAGIC: [u8; 4] = *b"MDAR";
const BUFFER_VERSION: u32 = 1;

/// Wire-format header placed at the very start of the exported buffer.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct BufferHeader {
    magic: [u8; 4],
    version: u32,
    node_struct_size: u32,
    node_count: u32,
    nodes_offset: u32,
    children_count: u32,
    children_offset: u32,
    type_data_len: u32,
    type_data_offset: u32,
    source_len: u32,
    source_offset: u32,
}

const HEADER_SIZE: usize = std::mem::size_of::<BufferHeader>();

impl Arena {
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

}
