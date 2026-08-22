//! Raw buffer export for zero-copy transfer.
//!
//! Wire format: `[Header][nodes...][children u32s][type_data bytes][string_pool UTF-8][node_data entries]`
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

use std::mem::offset_of;

use crate::arena::Arena;
use crate::generated::layout::header;
use crate::kind::ArenaKind;
use crate::node::{ArenaNode, NODE_STRUCT_SIZE};

pub(crate) const BUFFER_MAGIC: [u8; 4] = *b"MDAR";

impl<K: ArenaKind> Arena<K> {
    /// Serialize to a flat byte buffer:
    /// `[Header][nodes][children u32s][type_data][source][node_data]`
    pub fn to_raw_buffer(&self) -> Vec<u8> {
        let nodes_bytes = self.nodes.len() * NODE_STRUCT_SIZE;
        let children_bytes = self.children.len() * 4;
        let type_data_bytes = self.type_data.len();
        let string_pool_bytes = self.string_pool.len();

        // Sort node_data entries by node_id for deterministic output.
        let mut node_data_entries: Vec<(u32, &Vec<u8>)> =
            self.node_data.iter().map(|(k, v)| (*k, v)).collect();
        node_data_entries.sort_by_key(|(id, _)| *id);
        let node_data_count = node_data_entries.len() as u32;
        let node_data_section_bytes: usize = node_data_entries
            .iter()
            .map(|(_, v)| 4 /* id */ + 4 /* len */ + v.len())
            .sum();

        let pool_is_ascii = self.string_pool.is_ascii();

        // Backs every byte-to-UTF-16 conversion below without a `LineIndex` rebuild.
        let mut multibyte_starts: Vec<u32> = Vec::new();
        let mut multibyte_shifts: Vec<u32> = Vec::new();
        if !pool_is_ascii {
            let bytes = self.string_pool.as_bytes();
            let mut shift = 0u32;
            let mut i = 0;
            while i < bytes.len() {
                // Pools are overwhelmingly ASCII; striding beats decoding each char.
                while i + 8 <= bytes.len() {
                    let chunk = u64::from_le_bytes(bytes[i..i + 8].try_into().unwrap());
                    if chunk & 0x8080_8080_8080_8080 != 0 {
                        break;
                    }
                    i += 8;
                }
                if i >= bytes.len() {
                    break;
                }
                let lead = bytes[i];
                if lead < 0x80 {
                    i += 1;
                    continue;
                }
                let (utf8_len, utf16_len) = if lead < 0xe0 {
                    (2, 1)
                } else if lead < 0xf0 {
                    (3, 1)
                } else {
                    (4, 2)
                };
                multibyte_starts.push(i as u32);
                shift += utf8_len - utf16_len;
                multibyte_shifts.push(shift);
                i += utf8_len as usize;
            }
        }
        // Equal adjacent block shifts prove the 256-byte block holds no multibyte boundary, so a lookup there skips the search.
        let mut block_shifts: Vec<u32> = Vec::new();
        if !pool_is_ascii {
            let nblocks = (string_pool_bytes >> 8) + 2;
            block_shifts = vec![0u32; nblocks];
            let mut mi = 0usize;
            let mut cur = 0u32;
            for (b, slot) in block_shifts.iter_mut().enumerate() {
                let block_start = (b as u32) << 8;
                while mi < multibyte_starts.len() && multibyte_starts[mi] < block_start {
                    cur = multibyte_shifts[mi];
                    mi += 1;
                }
                *slot = cur;
            }
        }

        let nodes_offset = header::SIZE as u32;
        let children_offset = nodes_offset + nodes_bytes as u32;
        let type_data_offset = children_offset + children_bytes as u32;
        let string_pool_offset = type_data_offset + type_data_bytes as u32;
        let node_data_offset = string_pool_offset + string_pool_bytes as u32;

        let total = node_data_offset as usize + node_data_section_bytes;
        let mut buf = Vec::with_capacity(total);

        // The wire carries only UTF-16 units, so JS never needs a byte remap of its own.
        let to_utf16 = |byte_offset: u32| -> u32 {
            let b = (byte_offset >> 8) as usize;
            let shift = block_shifts[b];
            if shift == block_shifts[b + 1] {
                return byte_offset - shift;
            }
            let seen = multibyte_starts.partition_point(|&start| start < byte_offset);
            byte_offset
                - if seen == 0 {
                    0
                } else {
                    multibyte_shifts[seen - 1]
                }
        };

        // Header fields (little-endian u32s) at the generated layout offsets,
        // so the JS readers' generated `HEADER` table reads the same bytes.
        let mut hdr = [0u8; header::SIZE];
        let mut put = |off: usize, v: u32| hdr[off..off + 4].copy_from_slice(&v.to_le_bytes());
        put(header::MAGIC, u32::from_le_bytes(BUFFER_MAGIC));
        put(header::KIND, K::KIND_TAG as u32);
        put(header::NODE_STRUCT_SIZE, NODE_STRUCT_SIZE as u32);
        put(header::NODE_COUNT, self.nodes.len() as u32);
        put(header::NODES_OFFSET, nodes_offset);
        put(header::CHILDREN_COUNT, self.children.len() as u32);
        put(header::CHILDREN_OFFSET, children_offset);
        put(header::TYPE_DATA_LEN, self.type_data.len() as u32);
        put(header::TYPE_DATA_OFFSET, type_data_offset);
        put(header::STRING_POOL_LEN, self.string_pool.len() as u32);
        put(header::STRING_POOL_OFFSET, string_pool_offset);
        put(header::NODE_DATA_COUNT, node_data_count);
        put(header::NODE_DATA_OFFSET, node_data_offset);
        buf.extend_from_slice(&hdr);

        // The arena tracks `start_offset`/`end_offset` as **byte** offsets
        // (the parser works in bytes); `position` carries JS string indices
        // (UTF-16 code units), so convert here at serialization time.
        // Columns and lines are already in UTF-16 units.
        // SAFETY: ArenaNode is #[repr(C)] with explicit padding; same-process
        // serialization, never deserialized back into Rust.
        let nodes_slice: &[u8] =
            unsafe { std::slice::from_raw_parts(self.nodes.as_ptr() as *const u8, nodes_bytes) };
        let nodes_buf_start = buf.len();
        buf.extend_from_slice(nodes_slice);
        if !pool_is_ascii {
            const START_OFF_FIELD: usize = offset_of!(ArenaNode, start_offset);
            const END_OFF_FIELD: usize = offset_of!(ArenaNode, end_offset);
            let cached = self.utf16_offsets.len() == self.nodes.len();
            let put4 = |buf: &mut [u8], off: usize, v: u32| {
                let dst: &mut [u8; 4] = (&mut buf[off..off + 4]).try_into().unwrap();
                *dst = v.to_le_bytes();
            };
            if cached {
                for (i, &(utf16_start, utf16_end)) in self.utf16_offsets.iter().enumerate() {
                    let node = &self.nodes[i];
                    // A zero start line marks a synthesized node with no source
                    // range (lines are 1-based), even when a patch splice left it a
                    // non-zero spliced offset — nothing to convert.
                    if node.start_line == 0 {
                        continue;
                    }
                    let off = nodes_buf_start + i * NODE_STRUCT_SIZE;
                    put4(&mut buf, off + START_OFF_FIELD, utf16_start);
                    put4(&mut buf, off + END_OFF_FIELD, utf16_end);
                }
            } else {
                for (i, node) in self.nodes.iter().enumerate() {
                    if node.start_line == 0 {
                        continue;
                    }
                    let off = nodes_buf_start + i * NODE_STRUCT_SIZE;
                    put4(&mut buf, off + START_OFF_FIELD, to_utf16(node.start_offset));
                    put4(&mut buf, off + END_OFF_FIELD, to_utf16(node.end_offset));
                }
            }
        }

        // SAFETY: u32 has no padding. Note: this is a native-endian raw
        // dump of the children array; on big-endian targets it'd need
        // per-element to_le_bytes to match the wire format. Same caveat
        // applies to the nodes_slice dump above. Acceptable today since
        // all supported targets are little-endian.
        let children_slice: &[u8] = unsafe {
            std::slice::from_raw_parts(self.children.as_ptr() as *const u8, children_bytes)
        };
        buf.extend_from_slice(children_slice);

        if !pool_is_ascii {
            let type_data_start = buf.len();
            buf.extend_from_slice(&self.type_data);
            let type_data = &mut buf[type_data_start..];
            let mut remap = to_utf16;
            // A patch rebuild can leave several nodes sharing one blob; blob starts are 4-aligned, so a bit per slot dedups without hashing.
            let mut seen_blobs = vec![0u64; self.type_data.len() / 4 / 64 + 1];
            for node in &self.nodes {
                if node.data_len == 0 {
                    continue;
                }
                debug_assert_eq!(node.data_offset % 4, 0);
                let slot = (node.data_offset / 4) as usize;
                let (word, bit) = (slot / 64, 1u64 << (slot % 64));
                if seen_blobs[word] & bit != 0 {
                    continue;
                }
                seen_blobs[word] |= bit;
                let start = node.data_offset as usize;
                let end = start + node.data_len as usize;
                K::remap_string_refs(node.node_type, &mut type_data[start..end], &mut remap);
            }
        } else {
            buf.extend_from_slice(&self.type_data);
        }
        buf.extend_from_slice(self.string_pool.as_bytes());

        // node_data entries: [id:u32][len:u32][bytes...]
        for (id, data) in node_data_entries {
            buf.extend_from_slice(&id.to_le_bytes());
            buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
            buf.extend_from_slice(data);
        }

        buf
    }
}
