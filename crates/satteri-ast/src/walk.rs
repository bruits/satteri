//! Subscription-based tree walk.
//!
//! Walks the arena depth-first and collects nodes that match a set of
//! subscriptions into a single flat binary buffer. JS reads this with
//! DataView, no per-node object allocation.
//!
//! ## Result buffer format
//!
//! All integers are little-endian.
//!
//! ```text
//! [match_count: u32]
//! [match_index: match_count × 12 bytes]
//!   per entry: [node_id: u32][subscription_index: u8][pad: u8][data_offset: u32][data_len: u16]
//! [data section: variable length]
//!   per matched node: inline resolved data (format depends on node type)
//! ```
//!
//! Every HAST node payload shares the same prelude (matching `serialize_mdast_node_inline`):
//! ```text
//! [node_data: u32 len + utf8 JSON bytes]   ; 0-length when the node has no `data`
//! [position: 6×u32 = 24B]                  ; start_offset, end_offset, start_line, start_col, end_line, end_col
//! [child_count: u16][child_ids: child_count × u32]
//! [type-specific resolved data]
//! ```
//! Synthesized nodes (no source range) store all-zero position; JS surfaces those as `position: undefined`.
//!
//! Type-specific tails (after the shared prelude):
//!
//! ### Element (node_type=1)
//! ```text
//! [tag_name_len: u16][tag_name: utf8...]
//! [prop_count: u16]
//! per prop: [name_len: u16][name: utf8...][value_kind: u8][value_len: u16][value: utf8...]
//! ```
//!
//! ### MDX JSX flow/text element (node_type=10, 11)
//! ```text
//! [name_len: u16][name: utf8...]
//! [attr_count: u16]
//! per attr: [kind: u8][name_len: u16][name: utf8...][value_len: u32][value: utf8...]
//! ```
//!
//! ### Text/comment/raw/MDX expression/MDX ESM (node_type=2, 3, 5, 12, 13, 14)
//! ```text
//! [value_len: u32][value: utf8...]
//! ```
//!
//! ### Code (node_type=8)
//! ```text
//! [lang_len: u16][lang: utf8...][meta_len: u16][meta: utf8...][value_len: u32][value: utf8...]
//! ```

use satteri_arena::{Arena, ArenaKind, Hast, Mdast, StringRef};

/// A single subscription: match nodes of a given type, optionally filtered
/// by tag name (for HAST element nodes).
#[derive(Debug)]
pub struct Subscription {
    pub node_type: u8,
    pub tag_filter: Vec<String>,
}

use crate::hast::HastNodeType;

const HAST_ELEMENT_TYPE: u8 = HastNodeType::Element as u8;
const HAST_MDX_JSX_FLOW_TYPE: u8 = HastNodeType::MdxJsxElement as u8;
const HAST_MDX_JSX_TEXT_TYPE: u8 = HastNodeType::MdxJsxTextElement as u8;

/// Node types whose `type_data[0..8]` is a `StringRef` to a name that
/// `tag_filter` should compare against. HAST elements use the HTML tag name;
/// MDX JSX flow/text elements use the component name (`<Box/>` → `"Box"`).
fn hast_node_has_name(node_type: u8) -> bool {
    matches!(
        node_type,
        HAST_ELEMENT_TYPE | HAST_MDX_JSX_FLOW_TYPE | HAST_MDX_JSX_TEXT_TYPE
    )
}

/// Walk an MDAST arena and return matched nodes as a flat binary buffer.
pub fn walk_mdast(arena: &Arena<Mdast>, subscriptions: &[Subscription]) -> Vec<u8> {
    // MDAST has no tag-filtering: any non-empty tag_filter on an MDAST
    // subscription is a no-op (matches nothing in the current API).
    walk_and_collect_inner(arena, subscriptions, serialize_mdast_node_inline, |_| false)
}

/// Walk a HAST arena and return matched nodes as a flat binary buffer.
pub fn walk_hast(arena: &Arena<Hast>, subscriptions: &[Subscription]) -> Vec<u8> {
    walk_and_collect_inner(
        arena,
        subscriptions,
        serialize_hast_node_inline,
        hast_node_has_name,
    )
}

fn walk_and_collect_inner<K: ArenaKind>(
    arena: &Arena<K>,
    subscriptions: &[Subscription],
    serialize: fn(&Arena<K>, u32, u8, &[u8], &mut Vec<u8>),
    has_name: fn(u8) -> bool,
) -> Vec<u8> {
    if subscriptions.is_empty() {
        return 0u32.to_le_bytes().to_vec();
    }

    // Build fast lookup: node_type → list of (subscription_index, tag_filter)
    let mut type_subs: [Vec<(u8, &[String])>; 256] = std::array::from_fn(|_| Vec::new());
    for (i, sub) in subscriptions.iter().enumerate() {
        type_subs[sub.node_type as usize].push((i as u8, &sub.tag_filter));
    }

    // First pass: collect matches (node_id, sub_index) and serialize data
    let mut matches: Vec<(u32, u8)> = Vec::new();
    let mut data_section: Vec<u8> = Vec::new();
    let mut data_offsets: Vec<(u32, u16)> = Vec::new(); // (offset, len) per match

    let mut stack: Vec<u32> = vec![0];

    while let Some(node_id) = stack.pop() {
        let node = arena.get_node(node_id);
        let node_type = node.node_type;

        let subs = &type_subs[node_type as usize];
        if !subs.is_empty() {
            let type_data = arena.get_type_data(node_id);

            // For named nodes (HAST elements + MDX JSX flow/text), resolve the
            // name once so tag_filter comparisons can short-circuit.
            let tag_name = if has_name(node_type) && type_data.len() >= 8 {
                let tag_ref = read_string_ref(type_data, 0);
                Some(arena.get_str(tag_ref))
            } else {
                None
            };

            for &(sub_idx, tag_filter) in subs {
                let matched = if tag_filter.is_empty() {
                    true
                } else if let Some(tag) = tag_name {
                    tag_filter.iter().any(|f| f == tag)
                } else {
                    false
                };

                if matched {
                    let data_start = data_section.len() as u32;
                    serialize(arena, node_id, node_type, type_data, &mut data_section);
                    let data_len = (data_section.len() - data_start as usize) as u16;
                    matches.push((node_id, sub_idx));
                    data_offsets.push((data_start, data_len));
                }
            }
        }

        // Push children in reverse for depth-first order
        let children = arena.get_children(node_id);
        for &child_id in children.iter().rev() {
            stack.push(child_id);
        }
    }

    // Build output buffer: [count][index entries][data section]
    let match_count = matches.len() as u32;
    let index_size = match_count as usize * 12;
    let header_size = 4; // match_count
    let total = header_size + index_size + data_section.len();

    let mut out = Vec::with_capacity(total);

    // Header
    out.extend_from_slice(&match_count.to_le_bytes());

    // Index entries, adjust data_offset to account for header + index
    let data_base = (header_size + index_size) as u32;
    for i in 0..matches.len() {
        let (node_id, sub_idx) = matches[i];
        let (offset, len) = data_offsets[i];
        out.extend_from_slice(&node_id.to_le_bytes());
        out.push(sub_idx);
        out.push(0); // pad
        out.extend_from_slice(&(data_base + offset).to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
    }

    // Data section
    out.extend_from_slice(&data_section);

    out
}

/// MDAST node inline serialization.
///
/// Format per matched node:
/// ```text
/// [position: 6×u32 (24 bytes)]: start_offset, end_offset, start_line, start_col, end_line, end_col
/// [child_count: u16][child_ids: child_count × u32]: for parent nodes
/// [type-specific resolved data]
/// ```
fn serialize_mdast_node_inline(
    arena: &Arena<Mdast>,
    node_id: u32,
    node_type: u8,
    type_data: &[u8],
    out: &mut Vec<u8>,
) {
    let node = arena.get_node(node_id);

    // Node data (JSON bytes), length-prefixed, always first so JS can read it at a known offset
    if let Some(data) = arena.get_node_data(node_id) {
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
    } else {
        out.extend_from_slice(&0u32.to_le_bytes());
    }

    // Position (always present)
    out.extend_from_slice(&node.start_offset.to_le_bytes());
    out.extend_from_slice(&node.end_offset.to_le_bytes());
    out.extend_from_slice(&node.start_line.to_le_bytes());
    out.extend_from_slice(&node.start_column.to_le_bytes());
    out.extend_from_slice(&node.end_line.to_le_bytes());
    out.extend_from_slice(&node.end_column.to_le_bytes());

    // Children (for parent nodes)
    let children = arena.get_children(node_id);
    out.extend_from_slice(&(children.len() as u16).to_le_bytes());
    for &child_id in children {
        out.extend_from_slice(&child_id.to_le_bytes());
    }

    // Helper: write a resolved string ref as [len: u16][data]
    let write_str16 = |out: &mut Vec<u8>, data: &[u8], offset: usize| {
        if data.len() >= offset + 8 {
            let sr = read_string_ref(data, offset);
            let s = arena.get_str(sr);
            out.extend_from_slice(&(s.len() as u16).to_le_bytes());
            out.extend_from_slice(s.as_bytes());
        } else {
            out.extend_from_slice(&0u16.to_le_bytes());
        }
    };

    let write_str32 = |out: &mut Vec<u8>, data: &[u8], offset: usize| {
        if data.len() >= offset + 8 {
            let sr = read_string_ref(data, offset);
            let s = arena.get_str(sr);
            out.extend_from_slice(&(s.len() as u32).to_le_bytes());
            out.extend_from_slice(s.as_bytes());
        } else {
            out.extend_from_slice(&0u32.to_le_bytes());
        }
    };

    match node_type {
        // Heading: depth u8
        2 => {
            out.push(if !type_data.is_empty() {
                type_data[0]
            } else {
                1
            });
        }

        // Text(10), InlineCode(13), Html(7), Yaml(25), Toml(26): single StringRef value
        10 | 13 | 7 | 25 | 26 => write_str32(out, type_data, 0),

        // Code(8): lang(0) + meta(8) + value(16)
        8 => {
            write_str16(out, type_data, 0);
            write_str16(out, type_data, 8);
            write_str32(out, type_data, 16);
        }

        // Math(27), InlineMath(28): meta(0) + value(8)
        27 | 28 => {
            write_str16(out, type_data, 0);
            write_str32(out, type_data, 8);
        }

        // Link(15): url(0) + title(8)
        15 => {
            write_str16(out, type_data, 0);
            write_str16(out, type_data, 8);
        }

        // Image(16): url(0) + alt(8) + title(16)
        16 => {
            write_str16(out, type_data, 0);
            write_str16(out, type_data, 8);
            write_str16(out, type_data, 16);
        }

        // Definition(9): url(0) + title(8) + identifier(16) + label(24)
        9 => {
            write_str16(out, type_data, 0);
            write_str16(out, type_data, 8);
            write_str16(out, type_data, 16);
            write_str16(out, type_data, 24);
        }

        // List(5): start(0..4), ordered(4), spread(5)
        5 => {
            if type_data.len() >= 6 {
                out.extend_from_slice(&type_data[0..6]);
            } else {
                out.extend_from_slice(&[0u8; 6]);
            }
        }

        // ListItem(6): checked(0), spread(1)
        6 => {
            if type_data.len() >= 2 {
                out.extend_from_slice(&type_data[0..2]);
            } else {
                out.extend_from_slice(&[0u8; 2]);
            }
        }

        // LinkReference(17), ImageReference(18), FootnoteReference(20):
        // identifier(0) + label(8) + kind(16)
        17 | 18 | 20 => {
            write_str16(out, type_data, 0);
            write_str16(out, type_data, 8);
            out.push(if type_data.len() > 16 {
                type_data[16]
            } else {
                0
            });
        }

        // FootnoteDefinition(19): identifier(0) + label(8)
        19 => {
            write_str16(out, type_data, 0);
            write_str16(out, type_data, 8);
        }

        // Table(21): align_count(0..4) + align bytes
        21 => {
            if type_data.len() >= 4 {
                let count = u32::from_le_bytes(type_data[0..4].try_into().unwrap()) as usize;
                out.extend_from_slice(&(count as u16).to_le_bytes());
                let end = (4 + count).min(type_data.len());
                out.extend_from_slice(&type_data[4..end]);
            } else {
                out.extend_from_slice(&0u16.to_le_bytes());
            }
        }

        // MdxJsxFlowElement(100), MdxJsxTextElement(101): name(0) + attributes
        100 | 101 => {
            write_str16(out, type_data, 0);
            if type_data.len() >= 16 {
                let attr_count = u32::from_le_bytes(type_data[8..12].try_into().unwrap()) as usize;
                out.extend_from_slice(&(attr_count as u16).to_le_bytes());
                for i in 0..attr_count {
                    let base = 16 + i * 20;
                    let kind = type_data[base];
                    let attr_name = arena.get_str(read_string_ref(type_data, base + 4));
                    let attr_val = arena.get_str(read_string_ref(type_data, base + 12));
                    out.push(kind);
                    out.extend_from_slice(&(attr_name.len() as u16).to_le_bytes());
                    out.extend_from_slice(attr_name.as_bytes());
                    out.extend_from_slice(&(attr_val.len() as u32).to_le_bytes());
                    out.extend_from_slice(attr_val.as_bytes());
                }
            } else {
                out.extend_from_slice(&0u16.to_le_bytes());
            }
        }

        // ContainerDirective(30), LeafDirective(31), TextDirective(32): name + attributes
        30..=32 => {
            write_str16(out, type_data, 0);
            if type_data.len() >= 16 {
                let attr_count = u32::from_le_bytes(type_data[8..12].try_into().unwrap()) as usize;
                out.extend_from_slice(&(attr_count as u16).to_le_bytes());
                for i in 0..attr_count {
                    let base = 16 + i * 16;
                    let key = arena.get_str(read_string_ref(type_data, base));
                    let val = arena.get_str(read_string_ref(type_data, base + 8));
                    out.extend_from_slice(&(key.len() as u16).to_le_bytes());
                    out.extend_from_slice(key.as_bytes());
                    out.extend_from_slice(&(val.len() as u16).to_le_bytes());
                    out.extend_from_slice(val.as_bytes());
                }
            } else {
                out.extend_from_slice(&0u16.to_le_bytes());
            }
        }

        // MdxFlowExpression(102), MdxTextExpression(103), MdxjsEsm(104): value StringRef
        102..=104 => write_str32(out, type_data, 0),

        // Root(0), Paragraph(1), ThematicBreak(3), Blockquote(4), Emphasis(11),
        // Strong(12), Break(14), TableRow(22), TableCell(23), Delete(24): no type data
        _ => {}
    }
}

fn read_string_ref(data: &[u8], offset: usize) -> StringRef {
    StringRef::new(
        u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()),
        u32::from_le_bytes(data[offset + 4..offset + 8].try_into().unwrap()),
    )
}

/// HAST inline serialization: write node data with all strings resolved
/// (no StringRefs). Element data includes child node IDs so plugins can
/// reference them.
fn serialize_hast_node_inline(
    arena: &Arena<Hast>,
    node_id: u32,
    node_type: u8,
    type_data: &[u8],
    out: &mut Vec<u8>,
) {
    let node = arena.get_node(node_id);

    // Shared prelude (mirrors serialize_mdast_node_inline):
    // [data: u32 len + bytes][position: 24B][child_count: u16][child_ids: N×u32]

    // Node data (JSON bytes), length-prefixed, always first so JS can read it at a known offset
    if let Some(data) = arena.get_node_data(node_id) {
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
    } else {
        out.extend_from_slice(&0u32.to_le_bytes());
    }

    out.extend_from_slice(&node.start_offset.to_le_bytes());
    out.extend_from_slice(&node.end_offset.to_le_bytes());
    out.extend_from_slice(&node.start_line.to_le_bytes());
    out.extend_from_slice(&node.start_column.to_le_bytes());
    out.extend_from_slice(&node.end_line.to_le_bytes());
    out.extend_from_slice(&node.end_column.to_le_bytes());

    let children = arena.get_children(node_id);
    out.extend_from_slice(&(children.len() as u16).to_le_bytes());
    for &child_id in children {
        out.extend_from_slice(&child_id.to_le_bytes());
    }

    match node_type {
        // HAST element: tag + properties
        1 => {
            if type_data.len() < 16 {
                out.extend_from_slice(&0u16.to_le_bytes()); // empty tag
                out.extend_from_slice(&0u16.to_le_bytes()); // 0 props
                return;
            }
            let tag_ref = read_string_ref(type_data, 0);
            let tag = arena.get_str(tag_ref);
            out.extend_from_slice(&(tag.len() as u16).to_le_bytes());
            out.extend_from_slice(tag.as_bytes());

            let prop_count = u32::from_le_bytes(type_data[8..12].try_into().unwrap()) as usize;
            out.extend_from_slice(&(prop_count as u16).to_le_bytes());
            for i in 0..prop_count {
                let base = 16 + i * 20;
                let name_ref = read_string_ref(type_data, base);
                let kind = type_data[base + 8];
                let val_ref = read_string_ref(type_data, base + 12);
                let name = arena.get_str(name_ref);
                out.extend_from_slice(&(name.len() as u16).to_le_bytes());
                out.extend_from_slice(name.as_bytes());
                out.push(kind);
                let val = arena.get_str(val_ref);
                out.extend_from_slice(&(val.len() as u16).to_le_bytes());
                out.extend_from_slice(val.as_bytes());
            }
        }

        // MDX JSX elements (flow=10, text=11): name + attributes
        10 | 11 => {
            if type_data.len() < 16 {
                out.extend_from_slice(&0u16.to_le_bytes()); // empty name
                out.extend_from_slice(&0u16.to_le_bytes()); // 0 attrs
                return;
            }
            let name_ref = read_string_ref(type_data, 0);
            let name = arena.get_str(name_ref);
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(name.as_bytes());

            let attr_count = u32::from_le_bytes(type_data[8..12].try_into().unwrap()) as usize;
            out.extend_from_slice(&(attr_count as u16).to_le_bytes());
            for i in 0..attr_count {
                let base = 16 + i * 20;
                let kind = type_data[base];
                let attr_name_ref = read_string_ref(type_data, base + 4);
                let attr_val_ref = read_string_ref(type_data, base + 12);
                let attr_name = arena.get_str(attr_name_ref);
                let attr_val = arena.get_str(attr_val_ref);
                out.push(kind);
                out.extend_from_slice(&(attr_name.len() as u16).to_le_bytes());
                out.extend_from_slice(attr_name.as_bytes());
                out.extend_from_slice(&(attr_val.len() as u32).to_le_bytes());
                out.extend_from_slice(attr_val.as_bytes());
            }
        }

        // HAST text / comment / raw / MDX expressions / MDX ESM: single value
        2 | 3 | 5 | 12 | 13 | 14 => {
            if type_data.len() >= 8 {
                let val_ref = read_string_ref(type_data, 0);
                let val = arena.get_str(val_ref);
                out.extend_from_slice(&(val.len() as u32).to_le_bytes());
                out.extend_from_slice(val.as_bytes());
            } else {
                out.extend_from_slice(&0u32.to_le_bytes());
            }
        }

        // Code (type 8): lang + meta + value
        8 => {
            if type_data.len() >= 24 {
                let lang_ref = read_string_ref(type_data, 0);
                let meta_ref = read_string_ref(type_data, 8);
                let val_ref = read_string_ref(type_data, 16);
                let lang = arena.get_str(lang_ref);
                let meta = arena.get_str(meta_ref);
                let val = arena.get_str(val_ref);
                out.extend_from_slice(&(lang.len() as u16).to_le_bytes());
                out.extend_from_slice(lang.as_bytes());
                out.extend_from_slice(&(meta.len() as u16).to_le_bytes());
                out.extend_from_slice(meta.as_bytes());
                out.extend_from_slice(&(val.len() as u32).to_le_bytes());
                out.extend_from_slice(val.as_bytes());
            } else {
                out.extend_from_slice(&[0u8; 8]); // empty lang, meta, value
            }
        }

        _ => {
            // Generic: copy raw type_data as length-prefixed blob
            out.extend_from_slice(&(type_data.len() as u16).to_le_bytes());
            out.extend_from_slice(type_data);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use satteri_arena::ArenaBuilder;

    fn build_hast_with_elements(tags: &[&str]) -> Arena<Hast> {
        let mut b = ArenaBuilder::<Hast>::new(String::new());
        b.open_node_raw(0); // HAST root
        for tag in tags {
            b.open_node_raw(1); // HAST element
            let tag_ref = b.alloc_string(tag);
            let mut type_data = Vec::with_capacity(16);
            type_data.extend_from_slice(&tag_ref.offset.to_le_bytes());
            type_data.extend_from_slice(&tag_ref.len.to_le_bytes());
            type_data.extend_from_slice(&0u32.to_le_bytes()); // prop_count
            type_data.extend_from_slice(&0u32.to_le_bytes()); // pad
            b.set_data_current(&type_data);
            // text child
            b.open_node_raw(2);
            let val_ref = b.alloc_string("hello");
            let mut td = [0u8; 8];
            td[0..4].copy_from_slice(&val_ref.offset.to_le_bytes());
            td[4..8].copy_from_slice(&val_ref.len.to_le_bytes());
            b.set_data_current(&td);
            b.close_node();
            b.close_node();
        }
        b.close_node();
        b.finish()
    }

    fn read_match_count(buf: &[u8]) -> u32 {
        u32::from_le_bytes(buf[0..4].try_into().unwrap())
    }

    fn read_match_sub_index(buf: &[u8], index: usize) -> u8 {
        buf[4 + index * 12 + 4]
    }

    #[test]
    fn walk_no_subscriptions() {
        let arena = build_hast_with_elements(&["div", "a", "p"]);
        let buf = walk_hast(&arena, &[]);
        assert_eq!(read_match_count(&buf), 0);
    }

    #[test]
    fn walk_match_all_elements() {
        let arena = build_hast_with_elements(&["div", "a", "p"]);
        let subs = vec![Subscription {
            node_type: 1,
            tag_filter: vec![],
        }];
        let buf = walk_hast(&arena, &subs);
        assert_eq!(read_match_count(&buf), 3);
    }

    #[test]
    fn walk_filter_by_tag() {
        let arena = build_hast_with_elements(&["div", "a", "p", "a", "img"]);
        let subs = vec![Subscription {
            node_type: 1,
            tag_filter: vec!["a".into(), "img".into()],
        }];
        let buf = walk_hast(&arena, &subs);
        assert_eq!(read_match_count(&buf), 3); // two <a> + one <img>
    }

    #[test]
    fn walk_multiple_subscriptions() {
        let arena = build_hast_with_elements(&["div", "a", "p"]);
        let subs = vec![
            Subscription {
                node_type: 1,
                tag_filter: vec!["a".into()],
            },
            Subscription {
                node_type: 2, // HAST_TEXT
                tag_filter: vec![],
            },
        ];
        let buf = walk_hast(&arena, &subs);
        // 1 <a> element + 3 text nodes = 4
        assert_eq!(read_match_count(&buf), 4);
        // First match: text inside div (sub_index=1)
        assert_eq!(read_match_sub_index(&buf, 0), 1);
        // Second match: <a> element (sub_index=0)
        assert_eq!(read_match_sub_index(&buf, 1), 0);
    }

    #[test]
    fn element_data_contains_tag_name() {
        let arena = build_hast_with_elements(&["a"]);
        let subs = vec![Subscription {
            node_type: 1,
            tag_filter: vec![],
        }];
        let buf = walk_hast(&arena, &subs);
        assert_eq!(read_match_count(&buf), 1);
        // Read data offset and len from index
        let data_offset = u32::from_le_bytes(buf[4 + 6..4 + 10].try_into().unwrap()) as usize;
        let data_len = u16::from_le_bytes(buf[4 + 10..4 + 12].try_into().unwrap()) as usize;
        assert!(data_len > 0);
        // Skip prelude: [data_len=0: 4B][position: 24B][child_count=1: 2B][child_id: 4B] = 34B
        let tag_off = data_offset + 4 + 24 + 2 + 4;
        let tag_len = u16::from_le_bytes(buf[tag_off..tag_off + 2].try_into().unwrap()) as usize;
        assert_eq!(tag_len, 1); // "a"
        let tag = std::str::from_utf8(&buf[tag_off + 2..tag_off + 2 + tag_len]).unwrap();
        assert_eq!(tag, "a");
    }
}
