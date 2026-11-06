//! Raw buffer export and import for zero-copy transfer.
//!
//! Wire format: `[Header][nodes...][children u32s][type_data bytes][string_pool UTF-8][node_data entries]`
//!
//! The header carries a `kind` u32 right after `magic` so JS readers can
//! assert the buffer matches the kind they expect (`MdastReader` vs
//! `HastReader`). Mismatch is loud rather than silent — without the tag,
//! materialising an MDAST buffer through `HastReader` would decode garbage
//! `node_type` bytes into the wrong variants because the two kinds share
//! overlapping numeric values. `format_version` follows, so a reader refuses
//! a buffer from another wire format version instead of misreading it.
//!
//! `offset_unit` says whether offsets and string refs count UTF-16 units (for the JS readers) or UTF-8 bytes.
//!
//! `node_data` is the per-node JSON blob set via `Arena::set_node_data`
//! (used for `data.meta` on code elements, plugin-set custom data, etc.).
//! Each entry is `[node_id: u32 LE][data_len: u32 LE][bytes...]` and
//! entries are written in ascending node_id order.

use std::fmt;
use std::mem::offset_of;

use rustc_hash::FxHashMap;

use crate::arena::{Arena, Document};
pub use crate::generated::layout::FORMAT_VERSION;
use crate::generated::layout::header;
use crate::kind::ArenaKind;
use crate::node::{ArenaNode, NODE_STRUCT_SIZE};

pub(crate) const BUFFER_MAGIC: [u8; 4] = *b"MDAR";

/// What node offsets and string refs in a raw buffer count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OffsetUnit {
    /// UTF-16 code units, which JS strings index by. The only unit the JS readers accept.
    #[default]
    Utf16,
    /// UTF-8 bytes, as an arena stores them.
    Byte,
}

/// Why [`Document::from_raw_buffer`] refused a buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawBufferError {
    /// The magic bytes are wrong, or the buffer ends before the format version.
    NotARawBuffer,
    /// The buffer holds the other kind of tree.
    WrongKind { expected: u8, found: u32 },
    /// Written with another wire format version than [`FORMAT_VERSION`].
    UnsupportedVersion { found: u32 },
    /// The named part of the buffer is out of bounds or invalid.
    Malformed(&'static str),
}

impl fmt::Display for RawBufferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotARawBuffer => f.write_str("not a raw buffer"),
            Self::WrongKind { expected, found } => {
                write!(f, "buffer of kind {found}, expected kind {expected}")
            }
            Self::UnsupportedVersion { found } => {
                write!(f, "wire format version {found}, expected {FORMAT_VERSION}")
            }
            Self::Malformed(part) => write!(f, "malformed raw buffer: {part}"),
        }
    }
}

impl std::error::Error for RawBufferError {}

/// Byte offset of each multibyte char, with the running UTF-8 minus UTF-16 length through it.
// Outlined, the pushes write through the returned tuple and the scan measures ~1.2x slower.
#[inline(always)]
fn multibyte_shifts(bytes: &[u8]) -> (Vec<u32>, Vec<u32>) {
    let mut multibyte_starts: Vec<u32> = Vec::new();
    let mut multibyte_shifts: Vec<u32> = Vec::new();
    let mut shift = 0u32;
    let mut i = 0;
    while i < bytes.len() {
        // Pools are overwhelmingly ASCII; striding beats decoding each char.
        while let Some(chunk) = bytes[i..].first_chunk::<8>() {
            let flagged = u64::from_le_bytes(*chunk) & 0x8080_8080_8080_8080;
            if flagged != 0 {
                i += (flagged.trailing_zeros() >> 3) as usize;
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
    (multibyte_starts, multibyte_shifts)
}

impl<K: ArenaKind> Document<'_, K> {
    /// Serialize to a flat byte buffer for the JS readers, with offsets in UTF-16 units:
    /// `[Header][nodes][children u32s][type_data][source][node_data]`
    pub fn to_raw_buffer(&self) -> Vec<u8> {
        self.to_raw_buffer_in(OffsetUnit::Utf16)
    }

    /// Serialize like [`Self::to_raw_buffer`], with offsets counted in `unit`.
    pub fn to_raw_buffer_in(&self, unit: OffsetUnit) -> Vec<u8> {
        let pool = self.contiguous_pool();
        let nodes_bytes = self.nodes.len() * NODE_STRUCT_SIZE;
        let children_bytes = self.children.len() * 4;
        let type_data_bytes = self.type_data.len();
        let string_pool_bytes = pool.len();

        // Sort node_data entries by node_id for deterministic output.
        let mut node_data_entries: Vec<(u32, &Vec<u8>)> =
            self.node_data.iter().map(|(k, v)| (*k, v)).collect();
        node_data_entries.sort_by_key(|(id, _)| *id);
        let node_data_count = node_data_entries.len() as u32;
        let node_data_section_bytes: usize = node_data_entries
            .iter()
            .map(|(_, v)| 4 /* id */ + 4 /* len */ + v.len())
            .sum();

        // An ASCII pool has equal byte and UTF-16 offsets, so it needs no remap for either unit.
        let to_utf16_units = unit == OffsetUnit::Utf16 && !pool.is_ascii();

        // Backs every byte-to-UTF-16 conversion below without a `LineIndex` rebuild.
        let (multibyte_starts, multibyte_shifts) = if to_utf16_units {
            multibyte_shifts(pool.as_bytes())
        } else {
            (Vec::new(), Vec::new())
        };
        // Equal adjacent block shifts prove the 256-byte block holds no multibyte boundary, so a lookup there skips the search.
        let mut block_shifts: Vec<u32> = Vec::new();
        if to_utf16_units {
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
        let source_len = if to_utf16_units {
            to_utf16(self.source_len)
        } else {
            self.source_len
        };

        // Header fields (little-endian u32s) at the generated layout offsets,
        // so the JS readers' generated `HEADER` table reads the same bytes.
        let mut hdr = [0u8; header::SIZE];
        let mut put = |off: usize, v: u32| hdr[off..off + 4].copy_from_slice(&v.to_le_bytes());
        put(header::MAGIC, u32::from_le_bytes(BUFFER_MAGIC));
        put(header::KIND, K::KIND_TAG as u32);
        put(header::FORMAT_VERSION, FORMAT_VERSION);
        put(header::OFFSET_UNIT, unit as u32);
        put(header::NODE_STRUCT_SIZE, NODE_STRUCT_SIZE as u32);
        put(header::NODE_COUNT, self.nodes.len() as u32);
        put(header::NODES_OFFSET, nodes_offset);
        put(header::CHILDREN_COUNT, self.children.len() as u32);
        put(header::CHILDREN_OFFSET, children_offset);
        put(header::TYPE_DATA_LEN, self.type_data.len() as u32);
        put(header::TYPE_DATA_OFFSET, type_data_offset);
        put(header::STRING_POOL_LEN, pool.len() as u32);
        put(header::STRING_POOL_OFFSET, string_pool_offset);
        put(header::SOURCE_LEN, source_len);
        put(header::NODE_DATA_COUNT, node_data_count);
        put(header::NODE_DATA_OFFSET, node_data_offset);
        buf.extend_from_slice(&hdr);

        // The arena tracks `start_offset`/`end_offset` as **byte** offsets
        // (the parser works in bytes); `position` carries JS string indices
        // (UTF-16 code units), so convert here at serialization time.
        // Columns and lines are already in UTF-16 units.
        // SAFETY: ArenaNode is #[repr(C)] with explicit padding.
        let nodes_slice: &[u8] =
            unsafe { std::slice::from_raw_parts(self.nodes.as_ptr() as *const u8, nodes_bytes) };
        let nodes_buf_start = buf.len();
        buf.extend_from_slice(nodes_slice);
        if to_utf16_units {
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

        if to_utf16_units {
            let type_data_start = buf.len();
            buf.extend_from_slice(&self.type_data);
            let mut remap = to_utf16;
            remap_type_data::<K>(&self.nodes, &mut buf[type_data_start..], &mut remap);
        } else {
            buf.extend_from_slice(&self.type_data);
        }
        buf.extend_from_slice(pool.as_bytes());

        // node_data entries: [id:u32][len:u32][bytes...]
        for (id, data) in node_data_entries {
            buf.extend_from_slice(&id.to_le_bytes());
            buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
            buf.extend_from_slice(data);
        }

        buf
    }
}

impl<K: ArenaKind> Arena<K> {
    /// Rebuild an arena from a buffer [`Document::to_raw_buffer_in`] wrote, in either offset unit.
    ///
    /// `mdx` and `parse_options` are not on the wire and come back unset, so
    /// set `mdx` before compiling an MDX tree. The header, sections, and each
    /// node's child and data ranges are bounds-checked; the tree's shape is not.
    pub fn from_raw_buffer(buf: &[u8]) -> Result<Self, RawBufferError> {
        if read_u32(buf, header::MAGIC) != Some(u32::from_le_bytes(BUFFER_MAGIC)) {
            return Err(RawBufferError::NotARawBuffer);
        }
        let kind = read_u32(buf, header::KIND).ok_or(RawBufferError::NotARawBuffer)?;
        if kind != K::KIND_TAG as u32 {
            return Err(RawBufferError::WrongKind {
                expected: K::KIND_TAG,
                found: kind,
            });
        }
        let version = read_u32(buf, header::FORMAT_VERSION).ok_or(RawBufferError::NotARawBuffer)?;
        if version != FORMAT_VERSION {
            return Err(RawBufferError::UnsupportedVersion { found: version });
        }
        if buf.len() < header::SIZE {
            return Err(RawBufferError::Malformed("header"));
        }
        let field = |offset: usize| {
            u32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap()) as usize
        };
        if field(header::NODE_STRUCT_SIZE) != NODE_STRUCT_SIZE {
            return Err(RawBufferError::Malformed("node size"));
        }
        let unit = match field(header::OFFSET_UNIT) {
            u if u == OffsetUnit::Utf16 as usize => OffsetUnit::Utf16,
            u if u == OffsetUnit::Byte as usize => OffsetUnit::Byte,
            _ => return Err(RawBufferError::Malformed("offset unit")),
        };
        let section = |offset_field: usize, len: Option<usize>, part: &'static str| {
            let start = field(offset_field);
            len.and_then(|len| buf.get(start..start.checked_add(len)?))
                .ok_or(RawBufferError::Malformed(part))
        };
        let node_bytes = section(
            header::NODES_OFFSET,
            field(header::NODE_COUNT).checked_mul(NODE_STRUCT_SIZE),
            "nodes",
        )?;
        let children_bytes = section(
            header::CHILDREN_OFFSET,
            field(header::CHILDREN_COUNT).checked_mul(4),
            "children",
        )?;
        let type_data = section(
            header::TYPE_DATA_OFFSET,
            Some(field(header::TYPE_DATA_LEN)),
            "type data",
        )?;
        let pool = section(
            header::STRING_POOL_OFFSET,
            Some(field(header::STRING_POOL_LEN)),
            "string pool",
        )?;
        let pool =
            std::str::from_utf8(pool).map_err(|_| RawBufferError::Malformed("string pool"))?;
        let node_data = read_node_data(
            buf,
            field(header::NODE_DATA_OFFSET),
            field(header::NODE_DATA_COUNT),
        )
        .ok_or(RawBufferError::Malformed("node data"))?;

        let mut nodes: Vec<ArenaNode> = node_bytes
            .chunks_exact(NODE_STRUCT_SIZE)
            .map(|raw| {
                let at =
                    |offset: usize| u32::from_le_bytes(raw[offset..offset + 4].try_into().unwrap());
                let pad = offset_of!(ArenaNode, _pad);
                ArenaNode {
                    id: at(offset_of!(ArenaNode, id)),
                    node_type: raw[offset_of!(ArenaNode, node_type)],
                    _pad: raw[pad..pad + 3].try_into().unwrap(),
                    parent: at(offset_of!(ArenaNode, parent)),
                    start_offset: at(offset_of!(ArenaNode, start_offset)),
                    end_offset: at(offset_of!(ArenaNode, end_offset)),
                    start_line: at(offset_of!(ArenaNode, start_line)),
                    start_column: at(offset_of!(ArenaNode, start_column)),
                    end_line: at(offset_of!(ArenaNode, end_line)),
                    end_column: at(offset_of!(ArenaNode, end_column)),
                    children_start: at(offset_of!(ArenaNode, children_start)),
                    children_count: at(offset_of!(ArenaNode, children_count)),
                    data_offset: at(offset_of!(ArenaNode, data_offset)),
                    data_len: at(offset_of!(ArenaNode, data_len)),
                }
            })
            .collect();
        let children: Vec<u32> = children_bytes
            .chunks_exact(4)
            .map(|raw| u32::from_le_bytes(raw.try_into().unwrap()))
            .collect();
        let within =
            |start: u32, len: u32, bound: usize| u64::from(start) + u64::from(len) <= bound as u64;
        for node in &nodes {
            if !within(node.children_start, node.children_count, children.len()) {
                return Err(RawBufferError::Malformed("node children"));
            }
            if node.data_len != 0
                && (node.data_offset % 4 != 0
                    || !within(node.data_offset, node.data_len, type_data.len()))
            {
                return Err(RawBufferError::Malformed("node type data"));
            }
        }

        let mut type_data = type_data.to_vec();
        let mut source_len = field(header::SOURCE_LEN) as u32;
        if unit == OffsetUnit::Utf16 && !pool.is_ascii() {
            let (multibyte_starts, multibyte_shifts) = multibyte_shifts(pool.as_bytes());
            let unit_starts: Vec<u32> = multibyte_starts
                .iter()
                .zip(std::iter::once(0).chain(multibyte_shifts.iter().copied()))
                .map(|(&start, shift_before)| start - shift_before)
                .collect();
            let mut on_char_boundaries = true;
            let mut to_byte = |utf16_offset: u32| -> u32 {
                let seen = unit_starts.partition_point(|&start| start < utf16_offset);
                let shift = if seen == 0 {
                    0
                } else {
                    multibyte_shifts[seen - 1]
                };
                let byte_offset = utf16_offset.saturating_add(shift);
                on_char_boundaries &= pool.is_char_boundary(byte_offset as usize);
                byte_offset
            };
            for node in &mut nodes {
                // The writer leaves synthesized nodes' offsets unconverted.
                if node.start_line != 0 {
                    node.start_offset = to_byte(node.start_offset);
                    node.end_offset = to_byte(node.end_offset);
                }
            }
            source_len = to_byte(source_len);
            remap_type_data::<K>(&nodes, &mut type_data, &mut to_byte);
            if !on_char_boundaries {
                return Err(RawBufferError::Malformed("offsets"));
            }
        }
        if !pool.is_char_boundary(source_len as usize) {
            return Err(RawBufferError::Malformed("source length"));
        }

        let mut arena = Arena::<K>::new(pool.to_owned());
        arena.source_len = source_len;
        arena.nodes = nodes;
        arena.children = children;
        arena.type_data = type_data;
        arena.node_data = node_data;
        Ok(arena)
    }
}

fn remap_type_data<K: ArenaKind>(
    nodes: &[ArenaNode],
    type_data: &mut [u8],
    remap: &mut dyn FnMut(u32) -> u32,
) {
    // A patch rebuild can leave several nodes sharing one blob; blob starts are 4-aligned, so a bit per slot dedups without hashing.
    let mut seen_blobs = vec![0u64; type_data.len() / 4 / 64 + 1];
    #[cfg(debug_assertions)]
    let mut blob_claims = std::collections::HashMap::new();
    for node in nodes {
        if node.data_len == 0 {
            continue;
        }
        debug_assert_eq!(node.data_offset % 4, 0);
        let slot = (node.data_offset / 4) as usize;
        let (word, bit) = (slot / 64, 1u64 << (slot % 64));
        if seen_blobs[word] & bit != 0 {
            #[cfg(debug_assertions)]
            assert_eq!(
                blob_claims.get(&node.data_offset),
                Some(&(node.node_type, node.data_len)),
                "nodes sharing a type_data blob must share its layout"
            );
            continue;
        }
        seen_blobs[word] |= bit;
        #[cfg(debug_assertions)]
        blob_claims.insert(node.data_offset, (node.node_type, node.data_len));
        let start = node.data_offset as usize;
        let end = start + node.data_len as usize;
        K::remap_string_refs(node.node_type, &mut type_data[start..end], remap);
    }
}

fn read_u32(buf: &[u8], offset: usize) -> Option<u32> {
    let bytes = buf.get(offset..offset.checked_add(4)?)?;
    Some(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_node_data(buf: &[u8], mut cursor: usize, count: usize) -> Option<FxHashMap<u32, Vec<u8>>> {
    let mut node_data = FxHashMap::default();
    for _ in 0..count {
        let id = read_u32(buf, cursor)?;
        let len = read_u32(buf, cursor.checked_add(4)?)? as usize;
        let start = cursor.checked_add(8)?;
        let end = start.checked_add(len)?;
        node_data.insert(id, buf.get(start..end)?.to_vec());
        cursor = end;
    }
    Some(node_data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::{Hast, Mdast};
    use crate::{Arena, NodePosition};

    /// Text tag: a single StringRef at offset 0 in the generated remap table.
    const TEXT: u8 = 7;
    const START: usize = offset_of!(ArenaNode, start_offset);
    const END: usize = offset_of!(ArenaNode, end_offset);
    const DATA_OFF: usize = offset_of!(ArenaNode, data_offset);

    fn u32_at(buf: &[u8], off: usize) -> u32 {
        u32::from_le_bytes(buf[off..off + 4].try_into().unwrap())
    }

    fn wire_ref(buf: &[u8], type_data_at: u32, blob_offset: u32) -> (u32, u32) {
        let at = (type_data_at + blob_offset) as usize;
        (u32_at(buf, at), u32_at(buf, at + 4))
    }

    fn mixed_width_arena() -> Arena<Mdast> {
        // "aé😀b" is 8 bytes / 5 UTF-16 units; 300 'x's push the tail refs past the first 256-byte block onto the equal-shifts fast path.
        let mut pool = String::from("aé😀b");
        pool.push_str(&"x".repeat(300));
        let mut arena: Arena<Mdast> = Arena::new(pool);
        let root = arena.alloc_node(0);
        arena.set_position(
            root,
            NodePosition {
                start_offset: 0,
                end_offset: 308,
                start_line: 1,
                start_column: 1,
                end_line: 2,
                end_column: 1,
            },
        );
        let text = arena.alloc_node(TEXT);
        arena.set_position(
            text,
            NodePosition {
                start_offset: 1,
                end_offset: 8,
                start_line: 1,
                start_column: 2,
                end_line: 1,
                end_column: 6,
            },
        );
        arena.set_type_data(text, &[1, 0, 0, 0, 7, 0, 0, 0]);
        let tail = arena.alloc_node(TEXT);
        arena.set_position(
            tail,
            NodePosition {
                start_offset: 260,
                end_offset: 300,
                start_line: 1,
                start_column: 258,
                end_line: 1,
                end_column: 298,
            },
        );
        arena.set_type_data(tail, &[4, 1, 0, 0, 40, 0, 0, 0]);
        let synthesized = arena.alloc_node(TEXT);
        arena.set_position(
            synthesized,
            NodePosition {
                start_offset: 999,
                end_offset: 999,
                start_line: 0,
                start_column: 0,
                end_line: 0,
                end_column: 0,
            },
        );
        arena.set_children(root, &[text, tail, synthesized]);
        arena
    }

    fn assert_restored(arena: &Arena<Mdast>, unit: OffsetUnit) {
        let buf = arena.to_raw_buffer_in(unit);
        let read = Arena::<Mdast>::from_raw_buffer(&buf).unwrap();
        assert_eq!(read.nodes, arena.nodes);
        assert_eq!(read.children, arena.children);
        assert_eq!(read.type_data, arena.type_data);
        assert_eq!(read.string_pool, arena.string_pool);
        assert_eq!(read.source_len, arena.source_len);
        assert_eq!(read.node_data, arena.node_data);
        assert_eq!(read.to_raw_buffer_in(unit), buf);
    }

    #[test]
    fn wire_remaps_refs_and_node_offsets_to_utf16_units() {
        let buf = mixed_width_arena().to_raw_buffer();

        let nodes_at = u32_at(&buf, header::NODES_OFFSET) as usize;
        let type_data_at = u32_at(&buf, header::TYPE_DATA_OFFSET);
        let node = |id: usize| nodes_at + id * NODE_STRUCT_SIZE;

        assert_eq!(u32_at(&buf, header::OFFSET_UNIT), OffsetUnit::Utf16 as u32);
        assert_eq!(u32_at(&buf, header::SOURCE_LEN), 305);
        assert_eq!(u32_at(&buf, node(0) + START), 0);
        assert_eq!(u32_at(&buf, node(0) + END), 305);
        assert_eq!(u32_at(&buf, node(1) + START), 1);
        assert_eq!(u32_at(&buf, node(1) + END), 5);
        assert_eq!(u32_at(&buf, node(2) + START), 257);
        assert_eq!(u32_at(&buf, node(2) + END), 297);
        // Synthesized nodes (start_line 0) keep their spliced byte offsets.
        assert_eq!(u32_at(&buf, node(3) + START), 999);

        let text_blob = u32_at(&buf, node(1) + DATA_OFF);
        assert_eq!(wire_ref(&buf, type_data_at, text_blob), (1, 4));
        let tail_blob = u32_at(&buf, node(2) + DATA_OFF);
        assert_eq!(wire_ref(&buf, type_data_at, tail_blob), (257, 40));
    }

    #[test]
    fn byte_unit_wire_keeps_arena_offsets() {
        let arena = mixed_width_arena();
        let buf = arena.to_raw_buffer_in(OffsetUnit::Byte);

        let nodes_at = u32_at(&buf, header::NODES_OFFSET) as usize;
        let type_data_at = u32_at(&buf, header::TYPE_DATA_OFFSET) as usize;
        assert_eq!(u32_at(&buf, header::OFFSET_UNIT), OffsetUnit::Byte as u32);
        assert_eq!(u32_at(&buf, header::SOURCE_LEN), 308);
        assert_eq!(u32_at(&buf, nodes_at + 2 * NODE_STRUCT_SIZE + START), 260);
        assert_eq!(
            &buf[type_data_at..type_data_at + arena.type_data.len()],
            arena.type_data.as_slice()
        );
    }

    #[test]
    fn from_raw_buffer_restores_the_arena_in_either_unit() {
        let mut arena = mixed_width_arena();
        let computed = arena.alloc_string("ünï");
        let extra = arena.alloc_node(TEXT);
        arena.set_type_data(extra, &computed.as_bytes());
        arena.set_node_data(1, b"{\"meta\":\"\xc3\xa9\"}".to_vec());
        for unit in [OffsetUnit::Utf16, OffsetUnit::Byte] {
            assert_restored(&arena, unit);
        }
    }

    #[test]
    fn from_raw_buffer_refuses_other_buffers() {
        let buf = mixed_width_arena().to_raw_buffer();
        let read = |buf: &[u8]| Arena::<Mdast>::from_raw_buffer(buf).err();

        assert_eq!(read(&[]), Some(RawBufferError::NotARawBuffer));
        assert_eq!(read(&buf[..8]), Some(RawBufferError::NotARawBuffer));
        assert_eq!(
            Arena::<Hast>::from_raw_buffer(&buf).err(),
            Some(RawBufferError::WrongKind {
                expected: Hast::KIND_TAG,
                found: Mdast::KIND_TAG as u32
            })
        );

        let mut other_version = buf.clone();
        other_version[header::FORMAT_VERSION..header::FORMAT_VERSION + 4]
            .copy_from_slice(&(FORMAT_VERSION + 1).to_le_bytes());
        assert_eq!(
            read(&other_version),
            Some(RawBufferError::UnsupportedVersion {
                found: FORMAT_VERSION + 1
            })
        );

        assert_eq!(
            read(&buf[..buf.len() - 1]),
            Some(RawBufferError::Malformed("string pool"))
        );

        let mut past_pool = buf.clone();
        let end_at = u32_at(&buf, header::NODES_OFFSET) as usize + NODE_STRUCT_SIZE + END;
        past_pool[end_at..end_at + 4].copy_from_slice(&10_000u32.to_le_bytes());
        assert_eq!(read(&past_pool), Some(RawBufferError::Malformed("offsets")));
    }

    #[test]
    fn shared_blobs_remap_once() {
        let mut arena: Arena<Mdast> = Arena::new(String::from("aébb"));
        let root = arena.alloc_node(0);
        let a = arena.alloc_node(TEXT);
        arena.set_type_data(a, &[3, 0, 0, 0, 2, 0, 0, 0]);
        let b = arena.alloc_node(TEXT);
        let blob = arena.get_node(a).data_offset;
        arena.nodes[b as usize].data_offset = blob;
        arena.nodes[b as usize].data_len = 8;
        arena.set_children(root, &[a, b]);

        let buf = arena.to_raw_buffer();

        // A second remap of the shared blob would shift the start again (2 → 1).
        let type_data_at = u32_at(&buf, header::TYPE_DATA_OFFSET);
        assert_eq!(wire_ref(&buf, type_data_at, blob), (2, 2));
        assert_restored(&arena, OffsetUnit::Utf16);
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "must share its layout")]
    fn shared_blob_with_a_different_layout_asserts() {
        let mut arena: Arena<Mdast> = Arena::new(String::from("aébb"));
        let root = arena.alloc_node(0);
        let a = arena.alloc_node(TEXT);
        arena.set_type_data(a, &[3, 0, 0, 0, 2, 0, 0, 0]);
        let b = arena.alloc_node(8);
        let blob = arena.get_node(a).data_offset;
        arena.nodes[b as usize].data_offset = blob;
        arena.nodes[b as usize].data_len = 8;
        arena.set_children(root, &[a, b]);
        arena.to_raw_buffer();
    }
}
