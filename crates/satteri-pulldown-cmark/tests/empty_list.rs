use satteri_ast::mdast::MdastNodeType;
use satteri_pulldown_cmark::arena_build::{parse, DEFAULT_OPTIONS, MDX_OPTIONS};

#[test]
fn empty_list_item_keeps_list() {
    let input = "- a\n-  \n- b";
    let (arena, _) = parse(input, DEFAULT_OPTIONS);
    let root_children = arena.get_children(0);
    assert_eq!(root_children.len(), 1, "expected single list");
}

#[test]
fn empty_sublist_cant_interrupt_paragraph() {
    let input = "x\n+ -";
    let (arena, _) = parse(input, DEFAULT_OPTIONS);
    let root_children = arena.get_children(0);
    let types: Vec<u8> = root_children
        .iter()
        .map(|&id| arena.get_node(id).node_type)
        .collect();
    assert_eq!(
        root_children.len(),
        2,
        "expected paragraph + list, got types {:?}",
        types
    );
    let list_id = root_children[1];
    let list_items = arena.get_children(list_id);
    assert_eq!(list_items.len(), 1, "list should have 1 item");
    let item_children = arena.get_children(list_items[0]);
    assert_eq!(
        item_children.len(),
        1,
        "item should have 1 child (paragraph, not sublist)"
    );
}

#[test]
fn mdx_expression_then_jsx_is_flow() {
    let input = "{-83} <Box/>";
    let (arena, _) = parse(input, MDX_OPTIONS);
    let root_children = arena.get_children(0);
    let types: Vec<u8> = root_children
        .iter()
        .map(|&id| arena.get_node(id).node_type)
        .collect();
    eprintln!("types: {:?}", types);
    eprintln!(
        "MdxFlowExpression = {}",
        MdastNodeType::MdxFlowExpression as u8
    );
    eprintln!(
        "MdxJsxFlowElement = {}",
        MdastNodeType::MdxJsxFlowElement as u8
    );
    assert!(
        root_children.len() == 2,
        "expected 2 flow nodes, got {} with types {:?}",
        root_children.len(),
        types
    );
}

#[test]
fn mdx_fragment_with_content_is_flow() {
    let input = "<>{998}</>";
    let (arena, _) = parse(input, MDX_OPTIONS);
    let root_children = arena.get_children(0);
    let types: Vec<u8> = root_children
        .iter()
        .map(|&id| arena.get_node(id).node_type)
        .collect();
    eprintln!("types: {:?}", types);
    assert_eq!(
        root_children.len(),
        1,
        "expected 1 flow element, got {} with types {:?}",
        root_children.len(),
        types
    );
}

#[test]
fn blockquote_then_dash_after_para() {
    let input = "x\n>}\n-";
    let (arena, _) = parse(input, DEFAULT_OPTIONS);
    let root_children = arena.get_children(0);
    let types: Vec<u8> = root_children
        .iter()
        .map(|&id| arena.get_node(id).node_type)
        .collect();
    // paragraph, blockquote, list — the `-` on a new line outside the blockquote IS a list
    assert_eq!(
        root_children.len(),
        3,
        "expected 3 children, got {:?}",
        types
    );
    assert_eq!(
        arena.get_node(root_children[2]).node_type,
        MdastNodeType::List as u8
    );
}

#[test]
fn empty_list_in_blockquote_after_para() {
    let input = "x\n>*";
    let (arena, _) = parse(input, DEFAULT_OPTIONS);
    let root_children = arena.get_children(0);
    assert_eq!(root_children.len(), 2, "expected paragraph + blockquote");
    let bq_children = arena.get_children(root_children[1]);
    let bq_child_type = arena.get_node(bq_children[0]).node_type;
    assert_eq!(
        bq_child_type,
        MdastNodeType::Paragraph as u8,
        "blockquote content should be paragraph, not list"
    );
}
