//! Integration tests for MDAST → HAST conversion.

use tryckeri_hast::{mdast_to_hast, HastNodeType};
use tryckeri_mdast::{
    encode_code_data, encode_heading_data, encode_image_data, encode_link_data, encode_list_data,
    encode_string_ref_data, encode_table_data, ColumnAlign, MdastBuilder, MdastNodeType, StringRef,
};

fn build_heading_paragraph_arena() -> tryckeri_mdast::MdastArena {
    let source = "# Hello\n\nWorld".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(MdastNodeType::Root);

    let heading = b.open_node(MdastNodeType::Heading);
    b.set_data_current(&encode_heading_data(1));
    let text_hello = b.open_node(MdastNodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(2, 5)));
    b.close_node(); // text
    let _ = (heading, text_hello);
    b.close_node(); // heading

    b.open_node(MdastNodeType::Paragraph);
    b.open_node(MdastNodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(9, 5)));
    b.close_node(); // text
    b.close_node(); // paragraph

    b.close_node(); // root
    b.finish()
}

#[test]
fn arena1_root_is_root() {
    let mdast = build_heading_paragraph_arena();
    let hast = mdast_to_hast(&mdast);
    let root = hast.get_node(0);
    assert_eq!(root.node_type, HastNodeType::Root);
}

#[test]
fn arena1_two_children_of_root() {
    let mdast = build_heading_paragraph_arena();
    let hast = mdast_to_hast(&mdast);
    // Root has: h1, \n text, p (3 children with inter-block newline)
    assert_eq!(hast.get_children(0).len(), 3);
}

#[test]
fn arena1_first_child_is_h1() {
    let mdast = build_heading_paragraph_arena();
    let hast = mdast_to_hast(&mdast);
    let children = hast.get_children(0);
    let h1_id = children[0];
    let h1 = hast.get_node(h1_id);
    assert_eq!(h1.node_type, HastNodeType::Element);
    assert_eq!(hast.get_str(h1.tag_name), "h1");
}

#[test]
fn arena1_h1_has_text_hello() {
    let mdast = build_heading_paragraph_arena();
    let hast = mdast_to_hast(&mdast);
    let h1_id = hast.get_children(0)[0];
    let text_children = hast.get_children(h1_id);
    assert_eq!(text_children.len(), 1);
    let text_node = hast.get_node(text_children[0]);
    assert_eq!(text_node.node_type, HastNodeType::Text);
    assert_eq!(hast.get_str(text_node.value), "Hello");
}

#[test]
fn arena1_second_child_is_p_with_world() {
    let mdast = build_heading_paragraph_arena();
    let hast = mdast_to_hast(&mdast);
    // p is now at index 2 (after h1 and \n text node)
    let p_id = hast.get_children(0)[2];
    let p = hast.get_node(p_id);
    assert_eq!(p.node_type, HastNodeType::Element);
    assert_eq!(hast.get_str(p.tag_name), "p");

    let text_children = hast.get_children(p_id);
    assert_eq!(text_children.len(), 1);
    let text_node = hast.get_node(text_children[0]);
    assert_eq!(hast.get_str(text_node.value), "World");
}

fn build_link_arena() -> tryckeri_mdast::MdastArena {
    let source = "[click](https://example.com)".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(MdastNodeType::Root);
    b.open_node(MdastNodeType::Paragraph);

    let link = b.open_node(MdastNodeType::Link);
    b.set_data_current(&encode_link_data(
        StringRef::new(8, 19), // "https://example.com"
        StringRef::empty(),
    ));
    b.open_node(MdastNodeType::Text);
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
    let hast = mdast_to_hast(&mdast);
    let p_id = hast.get_children(0)[0];
    let a_id = hast.get_children(p_id)[0];
    let a = hast.get_node(a_id);
    assert_eq!(a.node_type, HastNodeType::Element);
    assert_eq!(hast.get_str(a.tag_name), "a");
}

#[test]
fn arena2_a_has_href_property() {
    let mdast = build_link_arena();
    let hast = mdast_to_hast(&mdast);
    let p_id = hast.get_children(0)[0];
    let a_id = hast.get_children(p_id)[0];
    let props = hast.get_properties(a_id);
    assert_eq!(props.len(), 1);
    assert_eq!(hast.get_str(props[0].name), "href");
    assert_eq!(
        hast.get_str(props[0].value.as_string_ref()),
        "https://example.com"
    );
}

#[test]
fn arena2_a_has_text_child_click() {
    let mdast = build_link_arena();
    let hast = mdast_to_hast(&mdast);
    let p_id = hast.get_children(0)[0];
    let a_id = hast.get_children(p_id)[0];
    let text_children = hast.get_children(a_id);
    assert_eq!(text_children.len(), 1);
    let text = hast.get_node(text_children[0]);
    assert_eq!(hast.get_str(text.value), "click");
}

fn build_image_arena() -> tryckeri_mdast::MdastArena {
    let source = "![alt text](img.png)".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(MdastNodeType::Root);
    b.open_node(MdastNodeType::Paragraph);
    b.open_node(MdastNodeType::Image);
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
    let hast = mdast_to_hast(&mdast);
    let p_id = hast.get_children(0)[0];
    let img_id = hast.get_children(p_id)[0];
    let img = hast.get_node(img_id);
    assert_eq!(img.node_type, HastNodeType::Element);
    assert_eq!(hast.get_str(img.tag_name), "img");
}

#[test]
fn arena3_img_has_src_and_alt() {
    let mdast = build_image_arena();
    let hast = mdast_to_hast(&mdast);
    let p_id = hast.get_children(0)[0];
    let img_id = hast.get_children(p_id)[0];
    let props = hast.get_properties(img_id);
    assert_eq!(props.len(), 2);
    assert_eq!(hast.get_str(props[0].name), "src");
    assert_eq!(hast.get_str(props[0].value.as_string_ref()), "img.png");
    assert_eq!(hast.get_str(props[1].name), "alt");
    assert_eq!(hast.get_str(props[1].value.as_string_ref()), "alt text");
}

fn build_emphasis_strong_arena() -> tryckeri_mdast::MdastArena {
    let source = "emstrongtext".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(MdastNodeType::Root);
    b.open_node(MdastNodeType::Paragraph);

    b.open_node(MdastNodeType::Emphasis);
    b.open_node(MdastNodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(0, 2)));
    b.close_node();
    b.close_node(); // emphasis

    b.open_node(MdastNodeType::Strong);
    b.open_node(MdastNodeType::Text);
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
    let hast = mdast_to_hast(&mdast);
    let p_id = hast.get_children(0)[0];
    let em_id = hast.get_children(p_id)[0];
    let em = hast.get_node(em_id);
    assert_eq!(em.node_type, HastNodeType::Element);
    assert_eq!(hast.get_str(em.tag_name), "em");
}

#[test]
fn arena4_strong_becomes_strong() {
    let mdast = build_emphasis_strong_arena();
    let hast = mdast_to_hast(&mdast);
    let p_id = hast.get_children(0)[0];
    let strong_id = hast.get_children(p_id)[1];
    let strong = hast.get_node(strong_id);
    assert_eq!(strong.node_type, HastNodeType::Element);
    assert_eq!(hast.get_str(strong.tag_name), "strong");
}

fn build_unordered_list_arena() -> tryckeri_mdast::MdastArena {
    let source = "- item 1\n- item 2".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(MdastNodeType::Root);

    b.open_node(MdastNodeType::List);
    b.set_data_current(&encode_list_data(false, 1, false));

    b.open_node(MdastNodeType::ListItem);
    b.open_node(MdastNodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(2, 6)));
    b.close_node();
    b.close_node();

    b.open_node(MdastNodeType::ListItem);
    b.open_node(MdastNodeType::Text);
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
    let hast = mdast_to_hast(&mdast);
    let ul_id = hast.get_children(0)[0];
    let ul = hast.get_node(ul_id);
    assert_eq!(ul.node_type, HastNodeType::Element);
    assert_eq!(hast.get_str(ul.tag_name), "ul");
}

#[test]
fn arena5_list_items_become_li() {
    let mdast = build_unordered_list_arena();
    let hast = mdast_to_hast(&mdast);
    let ul_id = hast.get_children(0)[0];
    let li_children = hast.get_children(ul_id);
    for &li_id in li_children {
        let li = hast.get_node(li_id);
        assert_eq!(hast.get_str(li.tag_name), "li");
    }
}

#[test]
fn arena5_two_li_children() {
    let mdast = build_unordered_list_arena();
    let hast = mdast_to_hast(&mdast);
    let ul_id = hast.get_children(0)[0];
    assert_eq!(hast.get_children(ul_id).len(), 2);
}

fn build_ordered_list_arena() -> tryckeri_mdast::MdastArena {
    let source = "3. item".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(MdastNodeType::Root);

    b.open_node(MdastNodeType::List);
    b.set_data_current(&encode_list_data(true, 3, false));

    b.open_node(MdastNodeType::ListItem);
    b.open_node(MdastNodeType::Text);
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
    let hast = mdast_to_hast(&mdast);
    let ol_id = hast.get_children(0)[0];
    let ol = hast.get_node(ol_id);
    assert_eq!(hast.get_str(ol.tag_name), "ol");
}

#[test]
fn arena6_ol_has_start_3_property() {
    let mdast = build_ordered_list_arena();
    let hast = mdast_to_hast(&mdast);
    let ol_id = hast.get_children(0)[0];
    let props = hast.get_properties(ol_id);
    assert_eq!(props.len(), 1);
    assert_eq!(hast.get_str(props[0].name), "start");
    assert_eq!(hast.get_str(props[0].value.as_string_ref()), "3");
}

fn build_inline_and_block_code_arena() -> tryckeri_mdast::MdastArena {
    let source = "foo\nrust\nfn main() {}".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(MdastNodeType::Root);

    b.open_node(MdastNodeType::Paragraph);
    b.open_node(MdastNodeType::InlineCode);
    b.set_data_current(&encode_string_ref_data(StringRef::new(0, 3))); // "foo"
    b.close_node();
    b.close_node();

    b.open_node(MdastNodeType::Code);
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
    let hast = mdast_to_hast(&mdast);
    let p_id = hast.get_children(0)[0];
    let code_id = hast.get_children(p_id)[0];
    let code = hast.get_node(code_id);
    assert_eq!(code.node_type, HastNodeType::Element);
    assert_eq!(hast.get_str(code.tag_name), "code");

    let text_id = hast.get_children(code_id)[0];
    let text = hast.get_node(text_id);
    assert_eq!(hast.get_str(text.value), "foo");
}

#[test]
fn arena7_code_block_becomes_pre_code_with_language_class() {
    let mdast = build_inline_and_block_code_arena();
    let hast = mdast_to_hast(&mdast);
    // root children: p, \n, pre (with inter-block newline)
    let pre_id = hast.get_children(0)[2];
    let pre = hast.get_node(pre_id);
    assert_eq!(hast.get_str(pre.tag_name), "pre");

    let code_id = hast.get_children(pre_id)[0];
    let code = hast.get_node(code_id);
    assert_eq!(hast.get_str(code.tag_name), "code");

    let props = hast.get_properties(code_id);
    assert_eq!(props.len(), 1);
    assert_eq!(hast.get_str(props[0].name), "class");
    assert_eq!(
        hast.get_str(props[0].value.as_string_ref()),
        "language-rust"
    );

    let text_id = hast.get_children(code_id)[0];
    let text = hast.get_node(text_id);
    assert_eq!(hast.get_str(text.value), "fn main() {}");
}

#[test]
fn arena8_thematic_break_becomes_hr() {
    let source = "---".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(MdastNodeType::Root);
    b.add_leaf(MdastNodeType::ThematicBreak);
    b.close_node();
    let mdast = b.finish();

    let hast = mdast_to_hast(&mdast);
    let hr_id = hast.get_children(0)[0];
    let hr = hast.get_node(hr_id);
    assert_eq!(hr.node_type, HastNodeType::Element);
    assert_eq!(hast.get_str(hr.tag_name), "hr");
}

fn build_table_arena() -> tryckeri_mdast::MdastArena {
    let source = "Name|Age\nAlice|30".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(MdastNodeType::Root);
    b.open_node(MdastNodeType::Table);
    b.set_data_current(&encode_table_data(&[ColumnAlign::None, ColumnAlign::None]));

    b.open_node(MdastNodeType::TableRow);
    b.open_node(MdastNodeType::TableCell);
    b.open_node(MdastNodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(0, 4)));
    b.close_node();
    b.close_node();
    b.open_node(MdastNodeType::TableCell);
    b.open_node(MdastNodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(5, 3)));
    b.close_node();
    b.close_node();
    b.close_node(); // header row

    b.open_node(MdastNodeType::TableRow);
    b.open_node(MdastNodeType::TableCell);
    b.open_node(MdastNodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(9, 5)));
    b.close_node();
    b.close_node();
    b.open_node(MdastNodeType::TableCell);
    b.open_node(MdastNodeType::Text);
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
    let hast = mdast_to_hast(&mdast);
    let table_id = hast.get_children(0)[0];
    let table = hast.get_node(table_id);
    assert_eq!(hast.get_str(table.tag_name), "table");

    let table_children = hast.get_children(table_id);
    assert_eq!(table_children.len(), 2);
    let thead = hast.get_node(table_children[0]);
    assert_eq!(hast.get_str(thead.tag_name), "thead");
    let tbody = hast.get_node(table_children[1]);
    assert_eq!(hast.get_str(tbody.tag_name), "tbody");
}

#[test]
fn arena9_header_row_uses_th_cells() {
    let mdast = build_table_arena();
    let hast = mdast_to_hast(&mdast);
    let table_id = hast.get_children(0)[0];
    let thead_id = hast.get_children(table_id)[0];
    let tr_id = hast.get_children(thead_id)[0];
    let cells = hast.get_children(tr_id);
    assert_eq!(cells.len(), 2);
    for &cell_id in cells {
        let cell = hast.get_node(cell_id);
        assert_eq!(hast.get_str(cell.tag_name), "th");
    }
}

#[test]
fn arena9_body_row_uses_td_cells() {
    let mdast = build_table_arena();
    let hast = mdast_to_hast(&mdast);
    let table_id = hast.get_children(0)[0];
    let tbody_id = hast.get_children(table_id)[1];
    let tr_id = hast.get_children(tbody_id)[0];
    let cells = hast.get_children(tr_id);
    assert_eq!(cells.len(), 2);
    for &cell_id in cells {
        let cell = hast.get_node(cell_id);
        assert_eq!(hast.get_str(cell.tag_name), "td");
    }
}

#[test]
fn arena10_delete_becomes_del() {
    let source = "deleted".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(MdastNodeType::Root);
    b.open_node(MdastNodeType::Paragraph);
    b.open_node(MdastNodeType::Delete);
    b.open_node(MdastNodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(0, 7)));
    b.close_node();
    b.close_node(); // delete
    b.close_node(); // paragraph
    b.close_node(); // root
    let mdast = b.finish();

    let hast = mdast_to_hast(&mdast);
    let p_id = hast.get_children(0)[0];
    let del_id = hast.get_children(p_id)[0];
    let del = hast.get_node(del_id);
    assert_eq!(del.node_type, HastNodeType::Element);
    assert_eq!(hast.get_str(del.tag_name), "del");
}

#[test]
fn arena11_html_node_becomes_raw() {
    let source = "<div>raw</div>".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(MdastNodeType::Root);
    b.open_node(MdastNodeType::Html);
    b.set_data_current(&encode_string_ref_data(StringRef::new(0, 14)));
    b.close_node();
    b.close_node(); // root
    let mdast = b.finish();

    let hast = mdast_to_hast(&mdast);
    let raw_id = hast.get_children(0)[0];
    let raw = hast.get_node(raw_id);
    assert_eq!(raw.node_type, HastNodeType::Raw);
    assert_eq!(hast.get_str(raw.value), "<div>raw</div>");
}
