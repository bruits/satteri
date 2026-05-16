//! Raw buffer export for zero-copy transfer.
//!
//! Wire format: `[Header][nodes...][children u32s][type_data bytes][source UTF-8][node_data entries]`
//!
//! The header carries a `kind` u32 right after `magic` so JS readers can
//! assert the buffer matches the kind they expect (`MdastReader` vs
//! `HastReader`). Mismatch is loud rather than silent — without the tag,
//! materialising an MDAST buffer through `HastReader` would decode garbage
//! `node_type` bytes into the wrong variants because the two kinds share
//! overlapping numeric values.
//!
//! `node_data` is the per-node JSON blob set via `Arena::set_node_data`
//! (used for `data.meta` on code elements, plugin-set custom data, etc.).
//! Each entry is `[node_id: u32 LE][data_len: u32 LE][bytes...]` and
//! entries are written in ascending node_id order.

use crate::arena::Arena;
use crate::kind::ArenaKind;
use crate::line_index::LineIndex;
use crate::node::NODE_STRUCT_SIZE;

const BUFFER_MAGIC: [u8; 4] = *b"MDAR";

// Header field sizes (all u32 LE):
//   magic(4) + kind(4) + node_struct_size(4) + node_count(4) + nodes_offset(4)
//   + children_count(4) + children_offset(4) + type_data_len(4) + type_data_offset(4)
//   + source_len(4) + source_offset(4) + node_data_count(4) + node_data_offset(4)
//   = 52 bytes
const HEADER_SIZE: usize = 52;

impl<K: ArenaKind> Arena<K> {
    /// Clone the node vec with `start_offset`/`end_offset` rewritten as
    /// code-point offsets (using a one-shot `LineIndex` over the source).
    /// Only called on the non-ASCII serialization path.
    fn nodes_with_cp_offsets(&self) -> Vec<crate::node::ArenaNode> {
        let line_index = LineIndex::from_source(&self.source);
        let mut cursor = line_index.cursor();
        let mut out = Vec::with_capacity(self.nodes.len());
        for node in &self.nodes {
            let mut n = *node;
            // Empty positions (line == 0 && offset == 0) stay empty.
            if !(n.start_line == 0 && n.start_offset == 0) {
                n.start_offset = cursor.byte_to_cp_offset(n.start_offset);
                n.end_offset = cursor.byte_to_cp_offset(n.end_offset);
            }
            out.push(n);
        }
        out
    }

    /// Serialize to a flat byte buffer:
    /// `[Header][nodes][children u32s][type_data][source][node_data]`
    pub fn to_raw_buffer(&self) -> Vec<u8> {
        let nodes_bytes = self.nodes.len() * NODE_STRUCT_SIZE;
        let children_bytes = self.children.len() * 4;
        let type_data_bytes = self.type_data.len();
        let source_bytes = self.source.len();

        // Sort node_data entries by node_id for deterministic output.
        let mut node_data_entries: Vec<(u32, &Vec<u8>)> =
            self.node_data.iter().map(|(k, v)| (*k, v)).collect();
        node_data_entries.sort_by_key(|(id, _)| *id);
        let node_data_count = node_data_entries.len() as u32;
        let node_data_section_bytes: usize = node_data_entries
            .iter()
            .map(|(_, v)| 4 /* id */ + 4 /* len */ + v.len())
            .sum();

        let nodes_offset = HEADER_SIZE as u32;
        let children_offset = nodes_offset + nodes_bytes as u32;
        let type_data_offset = children_offset + children_bytes as u32;
        let source_offset = type_data_offset + type_data_bytes as u32;
        let node_data_offset = source_offset + source_bytes as u32;

        let total = node_data_offset as usize + node_data_section_bytes;
        let mut buf = Vec::with_capacity(total);

        // Write header fields as little-endian u32s.
        buf.extend_from_slice(&BUFFER_MAGIC);
        buf.extend_from_slice(&(K::KIND_TAG as u32).to_ne_bytes());
        buf.extend_from_slice(&(NODE_STRUCT_SIZE as u32).to_ne_bytes());
        buf.extend_from_slice(&(self.nodes.len() as u32).to_ne_bytes());
        buf.extend_from_slice(&nodes_offset.to_ne_bytes());
        buf.extend_from_slice(&(self.children.len() as u32).to_ne_bytes());
        buf.extend_from_slice(&children_offset.to_ne_bytes());
        buf.extend_from_slice(&(self.type_data.len() as u32).to_ne_bytes());
        buf.extend_from_slice(&type_data_offset.to_ne_bytes());
        buf.extend_from_slice(&(self.source.len() as u32).to_ne_bytes());
        buf.extend_from_slice(&source_offset.to_ne_bytes());
        buf.extend_from_slice(&node_data_count.to_ne_bytes());
        buf.extend_from_slice(&node_data_offset.to_ne_bytes());

        // The arena tracks `start_offset`/`end_offset` as **byte** offsets
        // (the parser works in bytes). remark/micromark report code-point
        // offsets in `position`, so to match the reference shape we
        // convert here at serialization time. Columns and lines are
        // already in code-point units (set via `LineIndexCursor` which
        // counts code points). Convert by cloning the node vec and
        // patching the two offset fields.
        let cp_nodes;
        let nodes_slice: &[u8] = if self.source.is_ascii() {
            // ASCII fast path: byte offsets == code-point offsets, no work.
            unsafe { std::slice::from_raw_parts(self.nodes.as_ptr() as *const u8, nodes_bytes) }
        } else {
            cp_nodes = self.nodes_with_cp_offsets();
            unsafe { std::slice::from_raw_parts(cp_nodes.as_ptr() as *const u8, nodes_bytes) }
        };
        // SAFETY: ArenaNode is #[repr(C)] with all fields explicitly defined
        // (no implicit padding, _pad is explicit). The buffer is only read back
        // on the same platform via the JS DataView, never deserialized into Rust.
        buf.extend_from_slice(nodes_slice);

        // SAFETY: u32 has no padding or alignment concerns for ne bytes.
        let children_slice: &[u8] = unsafe {
            std::slice::from_raw_parts(self.children.as_ptr() as *const u8, children_bytes)
        };
        buf.extend_from_slice(children_slice);

        buf.extend_from_slice(&self.type_data);
        buf.extend_from_slice(self.source.as_bytes());

        // node_data entries: [id:u32][len:u32][bytes...]
        for (id, data) in node_data_entries {
            buf.extend_from_slice(&id.to_ne_bytes());
            buf.extend_from_slice(&(data.len() as u32).to_ne_bytes());
            buf.extend_from_slice(data);
        }

        buf
    }
}
