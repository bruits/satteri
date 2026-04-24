//! Verify that mdast→hast conversion preserves source positions on every
//! node it emits. Each HAST node we produce must carry the same
//! start/end offsets, lines, and columns as the MDAST node it came from
//! (or, for synthesized leaves like the checkbox `<input>`, the parent
//! MDAST node's position).

use satteri_arena::{decode_string_ref_data, Arena, ArenaBuilder};
use satteri_ast::hast::{
    codec::{decode_element_prop, decode_element_prop_count, decode_element_tag},
    mdast_arena_to_hast_arena, HastNodeType,
};
use satteri_ast::mdast::{
    codec::{encode_definition_data, encode_reference_data},
    MdastNodeType,
};

fn parse(md: &str) -> Arena {
    satteri_pulldown_cmark::parse(md, satteri_pulldown_cmark::DEFAULT_OPTIONS).0
}

fn find_mdast(arena: &Arena, target: MdastNodeType) -> u32 {
    (0..arena.len() as u32)
        .find(|&id| MdastNodeType::from_u8(arena.get_node(id).node_type) == Some(target))
        .unwrap_or_else(|| panic!("no mdast node of type {target:?}"))
}

/// Returns the value of `prop` on element `id`, or `None` if missing.
fn element_prop<'a>(arena: &'a Arena, id: u32, prop: &str) -> Option<&'a str> {
    let data = arena.get_type_data(id);
    let count = decode_element_prop_count(data);
    (0..count).find_map(|i| {
        let (name, _kind, value) = decode_element_prop(data, i);
        if arena.get_str(name) == prop {
            Some(arena.get_str(value))
        } else {
            None
        }
    })
}

fn is_element_with_tag(arena: &Arena, id: u32, tag: &str) -> bool {
    let node = arena.get_node(id);
    if HastNodeType::from_u8(node.node_type) != Some(HastNodeType::Element) {
        return false;
    }
    arena.get_str(decode_element_tag(arena.get_type_data(id))) == tag
}

/// Find the first element with the given tag in document order. Fine for
/// documents where only one element with that tag exists; use
/// `find_hast_element_where` when several share the tag and you need to
/// pick a specific one.
fn find_hast_element(arena: &Arena, tag: &str) -> u32 {
    (0..arena.len() as u32)
        .find(|&id| is_element_with_tag(arena, id, tag))
        .unwrap_or_else(|| panic!("no hast <{tag}> element"))
}

/// Find an element whose tag matches and that also satisfies `pred`.
fn find_hast_element_where<F>(arena: &Arena, tag: &str, pred: F) -> u32
where
    F: Fn(&Arena, u32) -> bool,
{
    (0..arena.len() as u32)
        .find(|&id| is_element_with_tag(arena, id, tag) && pred(arena, id))
        .unwrap_or_else(|| panic!("no hast <{tag}> element matching predicate"))
}

fn find_hast_by_type(arena: &Arena, target: HastNodeType) -> u32 {
    (0..arena.len() as u32)
        .find(|&id| HastNodeType::from_u8(arena.get_node(id).node_type) == Some(target))
        .unwrap_or_else(|| panic!("no hast node of type {target:?}"))
}

fn assert_position_matches(hast: &Arena, hast_id: u32, mdast: &Arena, mdast_id: u32, label: &str) {
    let h = hast.get_node(hast_id);
    let m = mdast.get_node(mdast_id);
    assert!(
        m.start_line > 0,
        "{label}: source mdast node has no position — test setup is broken"
    );
    assert_eq!(h.start_offset, m.start_offset, "{label}: start_offset");
    assert_eq!(h.end_offset, m.end_offset, "{label}: end_offset");
    assert_eq!(h.start_line, m.start_line, "{label}: start_line");
    assert_eq!(h.start_column, m.start_column, "{label}: start_column");
    assert_eq!(h.end_line, m.end_line, "{label}: end_line");
    assert_eq!(h.end_column, m.end_column, "{label}: end_column");
}

#[test]
fn heading_position_preserved() {
    let mdast = parse("# Hello\n\nworld");
    let hast = mdast_arena_to_hast_arena(&mdast);
    let m_id = find_mdast(&mdast, MdastNodeType::Heading);
    let h_id = find_hast_element(&hast, "h1");
    assert_position_matches(&hast, h_id, &mdast, m_id, "heading");
}

#[test]
fn paragraph_position_preserved() {
    let mdast = parse("Hello world");
    let hast = mdast_arena_to_hast_arena(&mdast);
    let m_id = find_mdast(&mdast, MdastNodeType::Paragraph);
    let h_id = find_hast_element(&hast, "p");
    assert_position_matches(&hast, h_id, &mdast, m_id, "paragraph");
}

#[test]
fn blockquote_position_preserved() {
    let mdast = parse("> quoted");
    let hast = mdast_arena_to_hast_arena(&mdast);
    let m_id = find_mdast(&mdast, MdastNodeType::Blockquote);
    let h_id = find_hast_element(&hast, "blockquote");
    assert_position_matches(&hast, h_id, &mdast, m_id, "blockquote");
}

#[test]
fn list_and_list_item_positions_preserved() {
    let mdast = parse("- first\n- second");
    let hast = mdast_arena_to_hast_arena(&mdast);

    let list_m = find_mdast(&mdast, MdastNodeType::List);
    let list_h = find_hast_element(&hast, "ul");
    assert_position_matches(&hast, list_h, &mdast, list_m, "list");

    let item_m = find_mdast(&mdast, MdastNodeType::ListItem);
    let item_h = find_hast_element(&hast, "li");
    assert_position_matches(&hast, item_h, &mdast, item_m, "list item");
}

#[test]
fn code_block_position_preserved() {
    let mdast = parse("```js\nconsole.log(1)\n```");
    let hast = mdast_arena_to_hast_arena(&mdast);
    let m_id = find_mdast(&mdast, MdastNodeType::Code);
    // The code block's position is attached to the outer <pre> element.
    let h_id = find_hast_element(&hast, "pre");
    assert_position_matches(&hast, h_id, &mdast, m_id, "code block");
}

#[test]
fn inline_code_position_preserved() {
    let mdast = parse("use `foo` here");
    let hast = mdast_arena_to_hast_arena(&mdast);
    let m_id = find_mdast(&mdast, MdastNodeType::InlineCode);
    // Inline code emits a bare <code> (no class). Fenced code uses
    // class="language-*" and inline math uses class="language-math *",
    // so filtering on class absence locks onto inline code specifically.
    let h_id = find_hast_element_where(&hast, "code", |arena, id| {
        element_prop(arena, id, "className").is_none()
    });
    assert_position_matches(&hast, h_id, &mdast, m_id, "inline code");
}

#[test]
fn emphasis_and_strong_positions_preserved() {
    let mdast = parse("a *em* and **strong**");
    let hast = mdast_arena_to_hast_arena(&mdast);

    let em_m = find_mdast(&mdast, MdastNodeType::Emphasis);
    let em_h = find_hast_element(&hast, "em");
    assert_position_matches(&hast, em_h, &mdast, em_m, "emphasis");

    let strong_m = find_mdast(&mdast, MdastNodeType::Strong);
    let strong_h = find_hast_element(&hast, "strong");
    assert_position_matches(&hast, strong_h, &mdast, strong_m, "strong");
}

#[test]
fn link_position_preserved() {
    let mdast = parse("[text](https://example.com)");
    let hast = mdast_arena_to_hast_arena(&mdast);
    let m_id = find_mdast(&mdast, MdastNodeType::Link);
    let h_id = find_hast_element(&hast, "a");
    assert_position_matches(&hast, h_id, &mdast, m_id, "link");
}

#[test]
fn thematic_break_position_preserved() {
    // Leaf void element — exercises the copy_position_to path.
    let mdast = parse("before\n\n---\n\nafter");
    let hast = mdast_arena_to_hast_arena(&mdast);
    let m_id = find_mdast(&mdast, MdastNodeType::ThematicBreak);
    let h_id = find_hast_element(&hast, "hr");
    assert_position_matches(&hast, h_id, &mdast, m_id, "thematic break");
}

#[test]
fn image_position_preserved() {
    // Leaf void element emitted from the Image arm.
    let mdast = parse("![alt](/u)");
    let hast = mdast_arena_to_hast_arena(&mdast);
    let m_id = find_mdast(&mdast, MdastNodeType::Image);
    let h_id = find_hast_element(&hast, "img");
    assert_position_matches(&hast, h_id, &mdast, m_id, "image");
}

#[test]
fn break_position_preserved() {
    let mdast = parse("line one  \nline two");
    let hast = mdast_arena_to_hast_arena(&mdast);
    let m_id = find_mdast(&mdast, MdastNodeType::Break);
    let h_id = find_hast_element(&hast, "br");
    assert_position_matches(&hast, h_id, &mdast, m_id, "hard break");
}

#[test]
fn text_position_preserved() {
    // Text is a leaf; the copy_position_to path runs inside the Text arm.
    let mdast = parse("hello");
    let hast = mdast_arena_to_hast_arena(&mdast);
    let m_id = find_mdast(&mdast, MdastNodeType::Text);
    let h_id = find_hast_by_type(&hast, HastNodeType::Text);
    // Sanity: the HAST leaf really is the text we expect.
    let text_ref = decode_string_ref_data(hast.get_type_data(h_id));
    assert_eq!(hast.get_str(text_ref), "hello");
    assert_position_matches(&hast, h_id, &mdast, m_id, "text");
}

#[test]
fn table_position_preserved() {
    let mdast = parse("| a | b |\n|---|---|\n| 1 | 2 |\n");
    let hast = mdast_arena_to_hast_arena(&mdast);
    let m_id = find_mdast(&mdast, MdastNodeType::Table);
    let h_id = find_hast_element(&hast, "table");
    assert_position_matches(&hast, h_id, &mdast, m_id, "table");
}

#[test]
fn footnote_reference_position_preserved() {
    // In GFM footnote emission, only the reference `<sup>` copies its
    // position from the source mdast node — the definition is moved into a
    // synthesised `<section>` whose `<li id="user-content-fn-N">` wrapper is
    // synthetic and deliberately has no source position.
    let mdast = parse("See[^1].\n\n[^1]: note");
    let hast = mdast_arena_to_hast_arena(&mdast);
    let ref_m = find_mdast(&mdast, MdastNodeType::FootnoteReference);
    let ref_h = find_hast_element(&hast, "sup");
    assert_position_matches(&hast, ref_h, &mdast, ref_m, "footnote reference");
}

#[test]
fn delete_position_preserved() {
    let mdast = parse("a ~~strike~~ b");
    let hast = mdast_arena_to_hast_arena(&mdast);
    let m_id = find_mdast(&mdast, MdastNodeType::Delete);
    let h_id = find_hast_element(&hast, "del");
    assert_position_matches(&hast, h_id, &mdast, m_id, "delete");
}

#[test]
fn math_block_position_preserved() {
    // Block math emits <pre><code class="language-math math-display">…</code></pre>;
    // position lands on the outer <pre>. Fenced code also emits <pre>, so
    // filter on the inner <code>'s class to lock onto math specifically.
    let mdast = parse("$$\nx = 1\n$$");
    let hast = mdast_arena_to_hast_arena(&mdast);
    let m_id = find_mdast(&mdast, MdastNodeType::Math);
    let h_id = find_hast_element_where(&hast, "pre", |arena, id| {
        arena.get_children(id).first().is_some_and(|&child_id| {
            is_element_with_tag(arena, child_id, "code")
                && matches!(
                    element_prop(arena, child_id, "className"),
                    Some(c) if c.contains("math-display")
                )
        })
    });
    assert_position_matches(&hast, h_id, &mdast, m_id, "math block");
}

#[test]
fn inline_math_position_preserved() {
    // InlineMath emits <code class="language-math math-inline">. Filter by
    // class so fenced code's <code> (class="language-*") can't match.
    let mdast = parse("an $x$ sym");
    let hast = mdast_arena_to_hast_arena(&mdast);
    let m_id = find_mdast(&mdast, MdastNodeType::InlineMath);
    let h_id = find_hast_element_where(&hast, "code", |arena, id| {
        matches!(
            element_prop(arena, id, "className"),
            Some(c) if c.contains("math-inline")
        )
    });
    assert_position_matches(&hast, h_id, &mdast, m_id, "inline math");
}

// LinkReference / ImageReference: pulldown-cmark resolves `[text][id]` into
// inline Link/Image at parse time, so these MDAST node types never appear
// in parser output — only when an mdast plugin constructs them. To test
// the corresponding convert.rs arms we build a minimal arena by hand.

/// Build a synthetic MDAST arena containing one paragraph with a single
/// `LinkReference` or `ImageReference` child, plus a matching `Definition`
/// sibling paragraph-level. Returns (arena, ref_node_id).
fn synth_reference_arena(ref_type: MdastNodeType) -> (Arena, u32) {
    let source = "see the page\n".to_string();
    let mut b = ArenaBuilder::new(source);

    b.open_node(MdastNodeType::Root as u8);
    b.set_position_current(0, 13, 1, 1, 2, 1);

    b.open_node(MdastNodeType::Paragraph as u8);
    b.set_position_current(0, 12, 1, 1, 1, 13);

    let ref_id = b.open_node(ref_type as u8);
    // Non-trivial position so assert_position_matches can actually catch
    // a bug: start and end differ from the paragraph's.
    b.set_position_current(4, 12, 1, 5, 1, 13);
    let identifier = b.alloc_string("ref");
    let label = b.alloc_string("ref");
    b.set_data_current(&encode_reference_data(identifier, label, 2 /* Full */));

    b.close_node(); // LinkReference / ImageReference
    b.close_node(); // Paragraph

    // Definition resolves the reference so the "resolved" branch of the
    // arm runs (that's the branch that calls copy_position).
    b.open_node(MdastNodeType::Definition as u8);
    let url = b.alloc_string("/u");
    let title = b.alloc_string("");
    let def_ident = b.alloc_string("ref");
    let def_label = b.alloc_string("ref");
    b.set_data_current(&encode_definition_data(url, title, def_ident, def_label));
    b.close_node();

    b.close_node(); // Root

    (b.finish(), ref_id)
}

#[test]
fn link_reference_position_preserved() {
    let (mdast, ref_id) = synth_reference_arena(MdastNodeType::LinkReference);
    let hast = mdast_arena_to_hast_arena(&mdast);
    // With the definition resolved, the LinkReference emits an <a>; the
    // Definition itself emits no HAST, so <a> is unique here.
    let h_id = find_hast_element(&hast, "a");
    assert_position_matches(&hast, h_id, &mdast, ref_id, "link reference");
}

#[test]
fn image_reference_position_preserved() {
    let (mdast, ref_id) = synth_reference_arena(MdastNodeType::ImageReference);
    let hast = mdast_arena_to_hast_arena(&mdast);
    let h_id = find_hast_element(&hast, "img");
    assert_position_matches(&hast, h_id, &mdast, ref_id, "image reference");
}

#[test]
fn html_block_position_preserved() {
    // Html blocks become Raw HAST leaves, not Elements — finding by
    // HastNodeType is the correct path.
    let mdast = parse("<div>raw</div>\n\npara");
    let hast = mdast_arena_to_hast_arena(&mdast);
    let m_id = find_mdast(&mdast, MdastNodeType::Html);
    let h_id = find_hast_by_type(&hast, HastNodeType::Raw);
    assert_position_matches(&hast, h_id, &mdast, m_id, "html block");
}

#[test]
fn multiline_positions_propagate_correctly() {
    // Keep the source stable and non-trivial: positions across lines are
    // where bugs tend to show up.
    let src = "# Title\n\nA paragraph with **bold** and `code`.\n\n- item\n";
    let mdast = parse(src);
    let hast = mdast_arena_to_hast_arena(&mdast);

    // The list starts on line 5 in the source.
    let list_m = find_mdast(&mdast, MdastNodeType::List);
    let list_node = mdast.get_node(list_m);
    assert_eq!(list_node.start_line, 5, "sanity: list on line 5 in source");

    let list_h = find_hast_element(&hast, "ul");
    assert_position_matches(&hast, list_h, &mdast, list_m, "multiline list");
}
