//! `text_content`: collect the concatenated text of all descendant text nodes.
//!
//! Works on any arena (MDAST or HAST) by taking a predicate that identifies
//! which node types are text-bearing and where their StringRef lives in type_data.

use satteri_arena::{Arena, ArenaKind};

/// Collect the concatenated text content of `node_id` and all its descendants.
///
/// `text_offset` is called with each node's type byte. It should return
/// `Some(offset)` if the node's `type_data` contains a StringRef at that byte
/// offset that should contribute to the output. Return `None` to recurse into
/// children instead.
///
/// Generic over `K`; the public wrappers `mdast::text_content` and
/// `hast::text_content` pin the kind for callers.
pub fn text_content<K: ArenaKind>(
    arena: &Arena<K>,
    node_id: u32,
    text_offset: impl Fn(u8) -> Option<usize>,
) -> String {
    let mut out = String::new();
    collect(arena, node_id, &text_offset, &mut out);
    out
}

fn collect<K: ArenaKind>(
    arena: &Arena<K>,
    node_id: u32,
    text_offset: &dyn Fn(u8) -> Option<usize>,
    out: &mut String,
) {
    let node = arena.get_node(node_id);

    if let Some(offset) = text_offset(node.node_type) {
        let td = arena.get_type_data(node_id);
        if td.len() >= offset + 8 {
            let str_offset =
                u32::from_le_bytes(td[offset..offset + 4].try_into().unwrap()) as usize;
            let str_len =
                u32::from_le_bytes(td[offset + 4..offset + 8].try_into().unwrap()) as usize;
            out.push_str(&arena.string_pool()[str_offset..str_offset + str_len]);
        }
        return;
    }

    for &child_id in arena.get_children(node_id) {
        collect(arena, child_id, text_offset, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use satteri_arena::{ArenaBuilder, Mdast};

    fn make_text_type_data(builder: &mut ArenaBuilder<Mdast>, text: &str) -> Vec<u8> {
        let sr = builder.alloc_string(text);
        let mut td = [0u8; 8];
        td[0..4].copy_from_slice(&sr.offset.to_le_bytes());
        td[4..8].copy_from_slice(&sr.len.to_le_bytes());
        td.to_vec()
    }

    // For tests: type 2 = text at offset 0, type 12/14 = expressions at offset 0
    fn text_offset(nt: u8) -> Option<usize> {
        match nt {
            2 | 12 | 14 => Some(0),
            _ => None,
        }
    }

    #[test]
    fn single_text_node() {
        let mut b = ArenaBuilder::<Mdast>::new(String::new());
        b.open_node_raw(0);
        b.open_node_raw(2);
        let td = make_text_type_data(&mut b, "hello");
        b.set_data_current(&td);
        b.close_node();
        b.close_node();
        let arena = b.finish();
        assert_eq!(text_content(&arena, 0, text_offset), "hello");
        assert_eq!(text_content(&arena, 1, text_offset), "hello");
    }

    #[test]
    fn nested_elements_with_text() {
        let mut b = ArenaBuilder::<Mdast>::new(String::new());
        b.open_node_raw(0);
        b.open_node_raw(1);
        {
            b.open_node_raw(2);
            let td = make_text_type_data(&mut b, "Hello ");
            b.set_data_current(&td);
            b.close_node();
            b.open_node_raw(1);
            {
                b.open_node_raw(2);
                let td = make_text_type_data(&mut b, "world");
                b.set_data_current(&td);
                b.close_node();
            }
            b.close_node();
        }
        b.close_node();
        b.close_node();
        let arena = b.finish();
        assert_eq!(text_content(&arena, 0, text_offset), "Hello world");
    }

    #[test]
    fn skips_non_text_nodes() {
        let mut b = ArenaBuilder::<Mdast>::new(String::new());
        b.open_node_raw(0);
        b.open_node_raw(2);
        let td = make_text_type_data(&mut b, "a");
        b.set_data_current(&td);
        b.close_node();
        b.open_node_raw(3);
        let td = make_text_type_data(&mut b, "COMMENT");
        b.set_data_current(&td);
        b.close_node();
        b.open_node_raw(5);
        let td = make_text_type_data(&mut b, "RAW");
        b.set_data_current(&td);
        b.close_node();
        b.open_node_raw(2);
        let td = make_text_type_data(&mut b, "b");
        b.set_data_current(&td);
        b.close_node();
        b.close_node();
        let arena = b.finish();
        assert_eq!(text_content(&arena, 0, text_offset), "ab");
    }

    #[test]
    fn includes_expression_nodes() {
        let mut b = ArenaBuilder::<Mdast>::new(String::new());
        b.open_node_raw(0);
        b.open_node_raw(2);
        let td = make_text_type_data(&mut b, "Hello ");
        b.set_data_current(&td);
        b.close_node();
        b.open_node_raw(14);
        let td = make_text_type_data(&mut b, "frontmatter.name");
        b.set_data_current(&td);
        b.close_node();
        b.close_node();
        let arena = b.finish();
        assert_eq!(
            text_content(&arena, 0, text_offset),
            "Hello frontmatter.name"
        );
    }

    #[test]
    fn value_at_nonzero_offset() {
        // Simulate a node where the StringRef is at offset 8 (like mdast Image alt)
        let mut b = ArenaBuilder::<Mdast>::new(String::new());
        b.open_node_raw(0);
        b.open_node_raw(42); // fake node type
        let sr = b.alloc_string("alt text");
        let mut td = vec![0u8; 16]; // 8 bytes padding + 8 bytes StringRef
        td[8..12].copy_from_slice(&sr.offset.to_le_bytes());
        td[12..16].copy_from_slice(&sr.len.to_le_bytes());
        b.set_data_current(&td);
        b.close_node();
        b.close_node();
        let arena = b.finish();
        assert_eq!(
            text_content(&arena, 0, |nt| if nt == 42 { Some(8) } else { None }),
            "alt text"
        );
    }
}
