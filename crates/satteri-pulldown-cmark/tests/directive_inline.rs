//! Directive label inline-parsing (Issue 3) and HTML-block closing-fence
//! handling (Issue 4) at the MDAST level.

use satteri_arena::{Arena, Mdast, StringRef};
use satteri_ast::mdast::MdastNodeType;
use satteri_pulldown_cmark::arena_build::{parse, DEFAULT_OPTIONS};
use satteri_pulldown_cmark::Options;

fn dir_options() -> Options {
    DEFAULT_OPTIONS | Options::ENABLE_DIRECTIVE
}

fn node_value(arena: &Arena<Mdast>, id: u32) -> String {
    let data = arena.get_type_data(id);
    if data.len() >= 8 {
        arena.get_str(StringRef::from_bytes(data)).to_string()
    } else {
        String::new()
    }
}

fn types_of(arena: &Arena<Mdast>, parent: u32) -> Vec<u8> {
    arena
        .get_children(parent)
        .iter()
        .map(|&id| arena.get_node(id).node_type)
        .collect()
}

// Issue 4: a closing `:::` fence must end the directive even when the body's
// last block is an HTML block, rather than being swallowed as HTML content.
#[test]
fn closing_fence_after_html_block_does_not_leak() {
    let input =
        ":::note\nParagraph.\n\n<details>\n<summary>See more</summary>\n\nMore.\n\n</details>\n:::";
    let (arena, _) = parse(input, dir_options());

    let root_children = arena.get_children(0);
    assert_eq!(
        root_children.len(),
        1,
        "expected a single container directive"
    );
    assert_eq!(
        arena.get_node(root_children[0]).node_type,
        MdastNodeType::ContainerDirective as u8
    );

    for id in 0..arena.len() as u32 {
        let nt = arena.get_node(id).node_type;
        if nt == MdastNodeType::Text as u8 || nt == MdastNodeType::Html as u8 {
            assert!(
                !node_value(&arena, id).contains(":::"),
                "closing fence leaked into a node value"
            );
        }
    }
}

// Issue 4: an unterminated raw-text (type 1) HTML block is still closed by the
// directive fence.
#[test]
fn closing_fence_ends_unterminated_pre_block() {
    let input = ":::note\n<pre>\ncontent\n:::";
    let (arena, _) = parse(input, dir_options());

    let root_children = arena.get_children(0);
    assert_eq!(root_children.len(), 1);
    assert_eq!(
        arena.get_node(root_children[0]).node_type,
        MdastNodeType::ContainerDirective as u8
    );
    for id in 0..arena.len() as u32 {
        if arena.get_node(id).node_type == MdastNodeType::Html as u8 {
            assert!(!node_value(&arena, id).contains(":::"));
        }
    }
}

// Issue 3: a directive label is inline-parsed in full — emphasis and strong, not
// just inline code.
#[test]
fn container_directive_label_parses_strong_and_emphasis() {
    let input = ":::note[Custom **strong with _emphasis_** Label]\nx\n:::";
    let (arena, _) = parse(input, dir_options());

    let directive = arena.get_children(0)[0];
    let label = arena.get_children(directive)[0];
    assert_eq!(
        arena.get_node(label).node_type,
        MdastNodeType::Paragraph as u8,
        "first directive child is the label paragraph"
    );

    assert_eq!(
        types_of(&arena, label),
        vec![
            MdastNodeType::Text as u8,   // "Custom "
            MdastNodeType::Strong as u8, // **strong with _emphasis_**
            MdastNodeType::Text as u8,   // " Label"
        ]
    );

    let strong = arena.get_children(label)[1];
    assert_eq!(
        types_of(&arena, strong),
        vec![
            MdastNodeType::Text as u8,     // "strong with "
            MdastNodeType::Emphasis as u8, // _emphasis_
        ]
    );
}

// Issue 3: a plain-text label is left as a single Text node.
#[test]
fn plain_directive_label_stays_single_text() {
    let input = ":::note[Just words]\nx\n:::";
    let (arena, _) = parse(input, dir_options());

    let directive = arena.get_children(0)[0];
    let label = arena.get_children(directive)[0];
    assert_eq!(types_of(&arena, label), vec![MdastNodeType::Text as u8]);
}

// Issue 3: a leaf directive's label is inline-parsed too, as its direct
// children (no label paragraph).
#[test]
fn leaf_directive_label_is_inline_parsed() {
    let input = "::video[A *great* clip]{src=x}";
    let (arena, _) = parse(input, dir_options());

    let directive = arena.get_children(0)[0];
    assert_eq!(
        arena.get_node(directive).node_type,
        MdastNodeType::LeafDirective as u8
    );
    assert_eq!(
        types_of(&arena, directive),
        vec![
            MdastNodeType::Text as u8,     // "A "
            MdastNodeType::Emphasis as u8, // *great*
            MdastNodeType::Text as u8,     // " clip"
        ]
    );
}

// Issue 3: a text directive's label is inline-parsed by re-entering the inline
// scanner over the bracketed span.
#[test]
fn text_directive_label_is_inline_parsed() {
    let input = "see :abbr[be **bold** now] end";
    let (arena, _) = parse(input, dir_options());

    let para = arena.get_children(0)[0];
    let directive = arena.get_children(para)[1];
    assert_eq!(
        arena.get_node(directive).node_type,
        MdastNodeType::TextDirective as u8
    );
    assert_eq!(
        types_of(&arena, directive),
        vec![
            MdastNodeType::Text as u8,   // "be "
            MdastNodeType::Strong as u8, // **bold**
            MdastNodeType::Text as u8,   // " now"
        ]
    );
}
