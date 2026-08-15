use satteri_ast::mdast::MdastNodeType;
use satteri_pulldown_cmark::{Options, parse};

fn node_end(source: &str, options: Options, node_type: MdastNodeType) -> (u32, u32, u32) {
    let (arena, _) = parse(source, options);
    let node = arena
        .nodes
        .iter()
        .find(|node| node.node_type == node_type as u8)
        .expect("fixture should produce the requested node type");
    (node.end_offset, node.end_line, node.end_column)
}

#[test]
fn list_position_includes_a_trailing_blank_blockquote_marker() {
    let source = "> See the docs:\n> - https://example.com\n> - https://example.org\n>\n";
    assert_eq!(
        node_end(source, Options::ENABLE_GFM, MdastNodeType::List),
        (65, 4, 2)
    );
}

#[test]
fn math_position_excludes_a_trailing_line_ending() {
    let source = "> $$\n> E = mc^2\n";
    assert_eq!(
        node_end(source, Options::ENABLE_MATH, MdastNodeType::Math),
        (15, 2, 11)
    );
}
