//! Integration tests for MDAST → HAST conversion.

use mdast_arena::{
    encode_code_data, encode_heading_data, encode_image_data, encode_link_data, encode_list_data,
    encode_string_ref_data, encode_table_data, ColumnAlign, MdastBuilder, NodeType, StringRef,
};
use tryckeri_hast::{mdast_to_hast, HastNodeType};

// ---------------------------------------------------------------------------
// Arena 1: Heading + Paragraph
// ---------------------------------------------------------------------------

fn build_heading_paragraph_arena() -> mdast_arena::MdastArena {
    // source: "# Hello\n\nWorld"
    let source = "# Hello\n\nWorld".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(NodeType::Root);

    // Heading depth=1
    let heading = b.open_node(NodeType::Heading);
    b.set_data_current(&encode_heading_data(1));
    // Text "Hello" — offset 2..7
    let text_hello = b.open_node(NodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(2, 5)));
    b.close_node(); // text
    let _ = (heading, text_hello);
    b.close_node(); // heading

    // Paragraph
    b.open_node(NodeType::Paragraph);
    // Text "World" — offset 9..14
    b.open_node(NodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(9, 5)));
    b.close_node(); // text
    b.close_node(); // paragraph

    b.close_node(); // root
    b.finish()
}

#[test]
fn arena1_root_is_root() {
    let mdast = build_heading_paragraph_arena();
    let hast = mdast_to_hast(&mdast, None);
    let root = hast.get_node(0);
    assert_eq!(root.node_type, HastNodeType::Root);
}

#[test]
fn arena1_two_children_of_root() {
    let mdast = build_heading_paragraph_arena();
    let hast = mdast_to_hast(&mdast, None);
    assert_eq!(hast.get_children(0).len(), 2);
}

#[test]
fn arena1_first_child_is_h1() {
    let mdast = build_heading_paragraph_arena();
    let hast = mdast_to_hast(&mdast, None);
    let children = hast.get_children(0);
    let h1_id = children[0];
    let h1 = hast.get_node(h1_id);
    assert_eq!(h1.node_type, HastNodeType::Element);
    assert_eq!(h1.tag_name.as_deref(), Some("h1"));
}

#[test]
fn arena1_h1_has_text_hello() {
    let mdast = build_heading_paragraph_arena();
    let hast = mdast_to_hast(&mdast, None);
    let h1_id = hast.get_children(0)[0];
    let text_children = hast.get_children(h1_id);
    assert_eq!(text_children.len(), 1);
    let text_node = hast.get_node(text_children[0]);
    assert_eq!(text_node.node_type, HastNodeType::Text);
    assert_eq!(text_node.value.as_deref(), Some("Hello"));
}

#[test]
fn arena1_second_child_is_p_with_world() {
    let mdast = build_heading_paragraph_arena();
    let hast = mdast_to_hast(&mdast, None);
    let p_id = hast.get_children(0)[1];
    let p = hast.get_node(p_id);
    assert_eq!(p.node_type, HastNodeType::Element);
    assert_eq!(p.tag_name.as_deref(), Some("p"));

    let text_children = hast.get_children(p_id);
    assert_eq!(text_children.len(), 1);
    let text_node = hast.get_node(text_children[0]);
    assert_eq!(text_node.value.as_deref(), Some("World"));
}

// ---------------------------------------------------------------------------
// Arena 2: Link
// ---------------------------------------------------------------------------

fn build_link_arena() -> mdast_arena::MdastArena {
    // source: "[click](https://example.com)"
    // url: "https://example.com" at offset 8, len 19
    // text: "click" at offset 1, len 5
    let source = "[click](https://example.com)".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(NodeType::Root);
    b.open_node(NodeType::Paragraph);

    let link = b.open_node(NodeType::Link);
    b.set_data_current(&encode_link_data(
        StringRef::new(8, 19), // "https://example.com"
        StringRef::empty(),
    ));
    b.open_node(NodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(1, 5))); // "click"
    b.close_node(); // text
    let _ = link;
    b.close_node(); // link

    b.close_node(); // paragraph
    b.close_node(); // root
    b.finish()
}

#[test]
fn arena2_link_becomes_a_element() {
    let mdast = build_link_arena();
    let hast = mdast_to_hast(&mdast, None);
    // root → p → a
    let p_id = hast.get_children(0)[0];
    let a_id = hast.get_children(p_id)[0];
    let a = hast.get_node(a_id);
    assert_eq!(a.node_type, HastNodeType::Element);
    assert_eq!(a.tag_name.as_deref(), Some("a"));
}

#[test]
fn arena2_a_has_href_property() {
    let mdast = build_link_arena();
    let hast = mdast_to_hast(&mdast, None);
    let p_id = hast.get_children(0)[0];
    let a_id = hast.get_children(p_id)[0];
    let props = hast.get_properties(a_id);
    assert_eq!(props.len(), 1);
    assert_eq!(props[0].name, "href");
    assert_eq!(
        props[0].value,
        tryckeri_hast::PropertyValue::String("https://example.com".to_string())
    );
}

#[test]
fn arena2_a_has_text_child_click() {
    let mdast = build_link_arena();
    let hast = mdast_to_hast(&mdast, None);
    let p_id = hast.get_children(0)[0];
    let a_id = hast.get_children(p_id)[0];
    let text_children = hast.get_children(a_id);
    assert_eq!(text_children.len(), 1);
    let text = hast.get_node(text_children[0]);
    assert_eq!(text.value.as_deref(), Some("click"));
}

// ---------------------------------------------------------------------------
// Arena 3: Image
// ---------------------------------------------------------------------------

fn build_image_arena() -> mdast_arena::MdastArena {
    // source: "![alt text](img.png)"
    // url: "img.png" at offset 12, len 7
    // alt: "alt text" at offset 2, len 8
    let source = "![alt text](img.png)".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(NodeType::Root);
    b.open_node(NodeType::Paragraph);
    b.open_node(NodeType::Image);
    b.set_data_current(&encode_image_data(
        StringRef::new(12, 7), // "img.png"
        StringRef::new(2, 8),  // "alt text"
        StringRef::empty(),
    ));
    b.close_node(); // image
    b.close_node(); // paragraph
    b.close_node(); // root
    b.finish()
}

#[test]
fn arena3_image_becomes_img_element() {
    let mdast = build_image_arena();
    let hast = mdast_to_hast(&mdast, None);
    let p_id = hast.get_children(0)[0];
    let img_id = hast.get_children(p_id)[0];
    let img = hast.get_node(img_id);
    assert_eq!(img.node_type, HastNodeType::Element);
    assert_eq!(img.tag_name.as_deref(), Some("img"));
}

#[test]
fn arena3_img_has_src_and_alt() {
    let mdast = build_image_arena();
    let hast = mdast_to_hast(&mdast, None);
    let p_id = hast.get_children(0)[0];
    let img_id = hast.get_children(p_id)[0];
    let props = hast.get_properties(img_id);
    assert_eq!(props.len(), 2);
    assert_eq!(props[0].name, "src");
    assert_eq!(
        props[0].value,
        tryckeri_hast::PropertyValue::String("img.png".to_string())
    );
    assert_eq!(props[1].name, "alt");
    assert_eq!(
        props[1].value,
        tryckeri_hast::PropertyValue::String("alt text".to_string())
    );
}

// ---------------------------------------------------------------------------
// Arena 4: Emphasis + Strong
// ---------------------------------------------------------------------------

fn build_emphasis_strong_arena() -> mdast_arena::MdastArena {
    let source = "emstrongtext".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(NodeType::Root);
    b.open_node(NodeType::Paragraph);

    // Emphasis → Text("em")
    b.open_node(NodeType::Emphasis);
    b.open_node(NodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(0, 2)));
    b.close_node();
    b.close_node(); // emphasis

    // Strong → Text("strong")
    b.open_node(NodeType::Strong);
    b.open_node(NodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(2, 6)));
    b.close_node();
    b.close_node(); // strong

    b.close_node(); // paragraph
    b.close_node(); // root
    b.finish()
}

#[test]
fn arena4_emphasis_becomes_em() {
    let mdast = build_emphasis_strong_arena();
    let hast = mdast_to_hast(&mdast, None);
    let p_id = hast.get_children(0)[0];
    let em_id = hast.get_children(p_id)[0];
    let em = hast.get_node(em_id);
    assert_eq!(em.node_type, HastNodeType::Element);
    assert_eq!(em.tag_name.as_deref(), Some("em"));
}

#[test]
fn arena4_strong_becomes_strong() {
    let mdast = build_emphasis_strong_arena();
    let hast = mdast_to_hast(&mdast, None);
    let p_id = hast.get_children(0)[0];
    let strong_id = hast.get_children(p_id)[1];
    let strong = hast.get_node(strong_id);
    assert_eq!(strong.node_type, HastNodeType::Element);
    assert_eq!(strong.tag_name.as_deref(), Some("strong"));
}

// ---------------------------------------------------------------------------
// Arena 5: Unordered list
// ---------------------------------------------------------------------------

fn build_unordered_list_arena() -> mdast_arena::MdastArena {
    let source = "- item 1\n- item 2".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(NodeType::Root);

    b.open_node(NodeType::List);
    b.set_data_current(&encode_list_data(false, 1, false));

    // ListItem 1 → Text("item 1")
    b.open_node(NodeType::ListItem);
    b.open_node(NodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(2, 6)));
    b.close_node();
    b.close_node();

    // ListItem 2 → Text("item 2")
    b.open_node(NodeType::ListItem);
    b.open_node(NodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(11, 6)));
    b.close_node();
    b.close_node();

    b.close_node(); // list
    b.close_node(); // root
    b.finish()
}

#[test]
fn arena5_list_becomes_ul() {
    let mdast = build_unordered_list_arena();
    let hast = mdast_to_hast(&mdast, None);
    let ul_id = hast.get_children(0)[0];
    let ul = hast.get_node(ul_id);
    assert_eq!(ul.node_type, HastNodeType::Element);
    assert_eq!(ul.tag_name.as_deref(), Some("ul"));
}

#[test]
fn arena5_list_items_become_li() {
    let mdast = build_unordered_list_arena();
    let hast = mdast_to_hast(&mdast, None);
    let ul_id = hast.get_children(0)[0];
    let li_children = hast.get_children(ul_id);
    for &li_id in li_children {
        let li = hast.get_node(li_id);
        assert_eq!(li.tag_name.as_deref(), Some("li"));
    }
}

#[test]
fn arena5_two_li_children() {
    let mdast = build_unordered_list_arena();
    let hast = mdast_to_hast(&mdast, None);
    let ul_id = hast.get_children(0)[0];
    assert_eq!(hast.get_children(ul_id).len(), 2);
}

// ---------------------------------------------------------------------------
// Arena 6: Ordered list with start=3
// ---------------------------------------------------------------------------

fn build_ordered_list_arena() -> mdast_arena::MdastArena {
    let source = "3. item".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(NodeType::Root);

    b.open_node(NodeType::List);
    b.set_data_current(&encode_list_data(true, 3, false));

    b.open_node(NodeType::ListItem);
    b.open_node(NodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(3, 4)));
    b.close_node();
    b.close_node();

    b.close_node(); // list
    b.close_node(); // root
    b.finish()
}

#[test]
fn arena6_ordered_list_becomes_ol() {
    let mdast = build_ordered_list_arena();
    let hast = mdast_to_hast(&mdast, None);
    let ol_id = hast.get_children(0)[0];
    let ol = hast.get_node(ol_id);
    assert_eq!(ol.tag_name.as_deref(), Some("ol"));
}

#[test]
fn arena6_ol_has_start_3_property() {
    let mdast = build_ordered_list_arena();
    let hast = mdast_to_hast(&mdast, None);
    let ol_id = hast.get_children(0)[0];
    let props = hast.get_properties(ol_id);
    assert_eq!(props.len(), 1);
    assert_eq!(props[0].name, "start");
    assert_eq!(
        props[0].value,
        tryckeri_hast::PropertyValue::String("3".to_string())
    );
}

// ---------------------------------------------------------------------------
// Arena 7: Inline code + code block
// ---------------------------------------------------------------------------

fn build_inline_and_block_code_arena() -> mdast_arena::MdastArena {
    // source: "foo\nrust\nfn main() {}"
    // InlineCode value: "foo" at 0..3
    // Code lang: "rust" at 4..8, value: "fn main() {}" at 9..21
    let source = "foo\nrust\nfn main() {}".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(NodeType::Root);

    // Paragraph with InlineCode
    b.open_node(NodeType::Paragraph);
    b.open_node(NodeType::InlineCode);
    b.set_data_current(&encode_string_ref_data(StringRef::new(0, 3))); // "foo"
    b.close_node();
    b.close_node();

    // Code block with lang="rust"
    b.open_node(NodeType::Code);
    b.set_data_current(&encode_code_data(
        StringRef::new(4, 4),  // lang: "rust"
        StringRef::empty(),    // meta: empty
        StringRef::new(9, 12), // value: "fn main() {}"
        b'`',
    ));
    b.close_node();

    b.close_node(); // root
    b.finish()
}

#[test]
fn arena7_inline_code_becomes_code_element() {
    let mdast = build_inline_and_block_code_arena();
    let hast = mdast_to_hast(&mdast, None);
    let p_id = hast.get_children(0)[0];
    let code_id = hast.get_children(p_id)[0];
    let code = hast.get_node(code_id);
    assert_eq!(code.node_type, HastNodeType::Element);
    assert_eq!(code.tag_name.as_deref(), Some("code"));

    let text_id = hast.get_children(code_id)[0];
    let text = hast.get_node(text_id);
    assert_eq!(text.value.as_deref(), Some("foo"));
}

#[test]
fn arena7_code_block_becomes_pre_code_with_language_class() {
    let mdast = build_inline_and_block_code_arena();
    let hast = mdast_to_hast(&mdast, None);
    // root children: p, pre
    let pre_id = hast.get_children(0)[1];
    let pre = hast.get_node(pre_id);
    assert_eq!(pre.tag_name.as_deref(), Some("pre"));

    let code_id = hast.get_children(pre_id)[0];
    let code = hast.get_node(code_id);
    assert_eq!(code.tag_name.as_deref(), Some("code"));

    // class="language-rust"
    let props = hast.get_properties(code_id);
    assert_eq!(props.len(), 1);
    assert_eq!(props[0].name, "class");
    assert_eq!(
        props[0].value,
        tryckeri_hast::PropertyValue::SpaceSeparated(vec!["language-rust".to_string()])
    );

    // text content
    let text_id = hast.get_children(code_id)[0];
    let text = hast.get_node(text_id);
    assert_eq!(text.value.as_deref(), Some("fn main() {}"));
}

// ---------------------------------------------------------------------------
// Arena 8: ThematicBreak
// ---------------------------------------------------------------------------

#[test]
fn arena8_thematic_break_becomes_hr() {
    let source = "---".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(NodeType::Root);
    b.add_leaf(NodeType::ThematicBreak);
    b.close_node();
    let mdast = b.finish();

    let hast = mdast_to_hast(&mdast, None);
    let hr_id = hast.get_children(0)[0];
    let hr = hast.get_node(hr_id);
    assert_eq!(hr.node_type, HastNodeType::Element);
    assert_eq!(hr.tag_name.as_deref(), Some("hr"));
}

// ---------------------------------------------------------------------------
// Arena 9: Table
// ---------------------------------------------------------------------------

fn build_table_arena() -> mdast_arena::MdastArena {
    // Table → TableRow(header) → TableCell("Name"), TableCell("Age")
    //       → TableRow(body)   → TableCell("Alice"), TableCell("30")
    let source = "Name|Age\nAlice|30".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(NodeType::Root);
    b.open_node(NodeType::Table);
    b.set_data_current(&encode_table_data(&[ColumnAlign::None, ColumnAlign::None]));

    // Header row
    b.open_node(NodeType::TableRow);
    b.open_node(NodeType::TableCell);
    b.open_node(NodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(0, 4)));
    b.close_node();
    b.close_node();
    b.open_node(NodeType::TableCell);
    b.open_node(NodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(5, 3)));
    b.close_node();
    b.close_node();
    b.close_node(); // header row

    // Body row
    b.open_node(NodeType::TableRow);
    b.open_node(NodeType::TableCell);
    b.open_node(NodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(9, 5)));
    b.close_node();
    b.close_node();
    b.open_node(NodeType::TableCell);
    b.open_node(NodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(15, 2)));
    b.close_node();
    b.close_node();
    b.close_node(); // body row

    b.close_node(); // table
    b.close_node(); // root
    b.finish()
}

#[test]
fn arena9_table_has_thead_and_tbody() {
    let mdast = build_table_arena();
    let hast = mdast_to_hast(&mdast, None);
    let table_id = hast.get_children(0)[0];
    let table = hast.get_node(table_id);
    assert_eq!(table.tag_name.as_deref(), Some("table"));

    let table_children = hast.get_children(table_id);
    assert_eq!(table_children.len(), 2);
    let thead = hast.get_node(table_children[0]);
    assert_eq!(thead.tag_name.as_deref(), Some("thead"));
    let tbody = hast.get_node(table_children[1]);
    assert_eq!(tbody.tag_name.as_deref(), Some("tbody"));
}

#[test]
fn arena9_header_row_uses_th_cells() {
    let mdast = build_table_arena();
    let hast = mdast_to_hast(&mdast, None);
    let table_id = hast.get_children(0)[0];
    let thead_id = hast.get_children(table_id)[0];
    let tr_id = hast.get_children(thead_id)[0];
    let cells = hast.get_children(tr_id);
    assert_eq!(cells.len(), 2);
    for &cell_id in cells {
        let cell = hast.get_node(cell_id);
        assert_eq!(cell.tag_name.as_deref(), Some("th"));
    }
}

#[test]
fn arena9_body_row_uses_td_cells() {
    let mdast = build_table_arena();
    let hast = mdast_to_hast(&mdast, None);
    let table_id = hast.get_children(0)[0];
    let tbody_id = hast.get_children(table_id)[1];
    let tr_id = hast.get_children(tbody_id)[0];
    let cells = hast.get_children(tr_id);
    assert_eq!(cells.len(), 2);
    for &cell_id in cells {
        let cell = hast.get_node(cell_id);
        assert_eq!(cell.tag_name.as_deref(), Some("td"));
    }
}

// ---------------------------------------------------------------------------
// Arena 10: Delete (strikethrough)
// ---------------------------------------------------------------------------

#[test]
fn arena10_delete_becomes_del() {
    let source = "deleted".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(NodeType::Root);
    b.open_node(NodeType::Paragraph);
    b.open_node(NodeType::Delete);
    b.open_node(NodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(0, 7)));
    b.close_node();
    b.close_node(); // delete
    b.close_node(); // paragraph
    b.close_node(); // root
    let mdast = b.finish();

    let hast = mdast_to_hast(&mdast, None);
    let p_id = hast.get_children(0)[0];
    let del_id = hast.get_children(p_id)[0];
    let del = hast.get_node(del_id);
    assert_eq!(del.node_type, HastNodeType::Element);
    assert_eq!(del.tag_name.as_deref(), Some("del"));
}

// ---------------------------------------------------------------------------
// Arena 11: Raw HTML
// ---------------------------------------------------------------------------

#[test]
fn arena11_html_node_becomes_raw() {
    // Html node with value "<div>raw</div>"
    let source = "<div>raw</div>".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(NodeType::Root);
    b.open_node(NodeType::Html);
    b.set_data_current(&encode_string_ref_data(StringRef::new(0, 14)));
    b.close_node();
    b.close_node(); // root
    let mdast = b.finish();

    let hast = mdast_to_hast(&mdast, None);
    let raw_id = hast.get_children(0)[0];
    let raw = hast.get_node(raw_id);
    assert_eq!(raw.node_type, HastNodeType::Raw);
    assert_eq!(raw.value.as_deref(), Some("<div>raw</div>"));
}
