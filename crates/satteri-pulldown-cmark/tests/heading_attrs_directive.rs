//! Heading attributes interacting with text directives (issues #134, #135).
//! Both features share `{...}` syntax: a heading's trailing attribute block
//! must not swallow a directive's own attributes, and must still be found when
//! a directive trails it.

use satteri_arena::{Arena, Mdast, StringRef};
use satteri_ast::mdast::codec::{decode_directive_attr, decode_directive_attr_count};
use satteri_ast::mdast::MdastNodeType;
use satteri_pulldown_cmark::arena_build::{parse, DEFAULT_OPTIONS};
use satteri_pulldown_cmark::Options;

fn options() -> Options {
    DEFAULT_OPTIONS | Options::ENABLE_DIRECTIVE | Options::ENABLE_HEADING_ATTRIBUTES
}

fn heading(arena: &Arena<Mdast>) -> u32 {
    let roots = arena.get_children(0);
    assert_eq!(roots.len(), 1, "expected a single heading");
    let h = roots[0];
    assert_eq!(
        arena.get_node(h).node_type,
        MdastNodeType::Heading as u8,
        "root child should be a heading"
    );
    h
}

fn heading_data(arena: &Arena<Mdast>, heading: u32) -> String {
    arena
        .get_node_data(heading)
        .map(|d| String::from_utf8_lossy(d).into_owned())
        .unwrap_or_default()
}

fn find_directive(arena: &Arena<Mdast>, parent: u32) -> u32 {
    arena
        .get_children(parent)
        .iter()
        .copied()
        .find(|&id| arena.get_node(id).node_type == MdastNodeType::TextDirective as u8)
        .expect("expected a text directive child")
}

fn directive_name(arena: &Arena<Mdast>, id: u32) -> String {
    let data = arena.get_type_data(id);
    arena.get_str(StringRef::from_bytes(&data[..8])).to_string()
}

fn directive_attrs(arena: &Arena<Mdast>, id: u32) -> Vec<(String, String)> {
    let data = arena.get_type_data(id);
    (0..decode_directive_attr_count(data))
        .map(|i| {
            let (k, v) = decode_directive_attr(data, i);
            (arena.get_str(k).to_string(), arena.get_str(v).to_string())
        })
        .collect()
}

// #134: `{variant=caution}` is the directive's attribute block, not the heading's.
#[test]
fn directive_attributes_are_not_stolen_by_heading() {
    let (arena, _) = parse("## Heading :badge[New]{variant=caution}\n", options());
    let h = heading(&arena);

    assert!(
        !heading_data(&arena, h).contains("variant"),
        "heading must not absorb the directive's attribute block"
    );

    let dir = find_directive(&arena, h);
    assert_eq!(directive_name(&arena, dir), "badge");
    assert_eq!(
        directive_attrs(&arena, dir),
        vec![("variant".to_string(), "caution".to_string())]
    );
}

// #135: the attribute block is recognized whether it comes before or after a
// trailing directive.
#[test]
fn heading_attribute_is_recognized_around_a_trailing_directive() {
    let before = "## Heading with a badge {#custom} :badge[Custom]\n";
    let after = "## Heading with a badge :badge[Custom] {#custom}\n";
    for input in [before, after] {
        let (arena, _) = parse(input, options());
        let h = heading(&arena);
        assert!(
            heading_data(&arena, h).contains(r#""id":"custom""#),
            "expected id=custom for input {input:?}, got {:?}",
            heading_data(&arena, h)
        );
        assert_eq!(directive_name(&arena, find_directive(&arena, h)), "badge");
    }
}

// A directive that owns its block AND is preceded by a real heading block:
// the heading keeps `#custom`, the directive keeps `variant=caution`.
#[test]
fn heading_block_and_directive_block_coexist() {
    let (arena, _) = parse(
        "## Heading {#custom} :badge[New]{variant=caution}\n",
        options(),
    );
    let h = heading(&arena);
    assert!(heading_data(&arena, h).contains(r#""id":"custom""#));

    let dir = find_directive(&arena, h);
    assert_eq!(
        directive_attrs(&arena, dir),
        vec![("variant".to_string(), "caution".to_string())]
    );
}

// The tricky setext case: a block sitting *before* a trailing directive, which
// the directive must survive with its own attributes intact.
#[test]
fn setext_heading_attribute_is_recognized_around_a_trailing_directive() {
    let before = "Heading with a badge {#custom} :badge[Custom]\n===\n";
    let after = "Heading with a badge :badge[Custom] {#custom}\n===\n";
    for input in [before, after] {
        let (arena, _) = parse(input, options());
        let h = heading(&arena);
        assert!(
            heading_data(&arena, h).contains(r#""id":"custom""#),
            "expected id=custom for input {input:?}, got {:?}",
            heading_data(&arena, h)
        );
        assert_eq!(directive_name(&arena, find_directive(&arena, h)), "badge");
    }
}

// Setext counterpart of `heading_block_and_directive_block_coexist`: the block
// before the directive must not swallow the directive's own `{variant=...}`.
#[test]
fn setext_heading_block_and_directive_block_coexist() {
    let (arena, _) = parse(
        "Heading {#custom} :badge[New]{variant=caution}\n===\n",
        options(),
    );
    let h = heading(&arena);
    assert!(heading_data(&arena, h).contains(r#""id":"custom""#));

    let dir = find_directive(&arena, h);
    assert_eq!(
        directive_attrs(&arena, dir),
        vec![("variant".to_string(), "caution".to_string())]
    );
}
