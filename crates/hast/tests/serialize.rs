//! Integration tests for HAST → HTML serialization and end-to-end conversion.

use tryckeri_hast::{hast_to_html, mdast_to_html, HastBuilder, Property, PropertyValue};
use tryckeri_mdast::{
    encode_code_data, encode_heading_data, encode_image_data, encode_link_data,
    encode_string_ref_data, MdastBuilder, MdastNodeType, StringRef,
};

#[test]
fn serialize_simple_paragraph() {
    let mut b = HastBuilder::new();
    b.open_root();
    b.open_element("p");
    b.add_text("Hello");
    b.close();
    let hast = b.finish();
    assert_eq!(hast_to_html(&hast), "<p>Hello</p>");
}

#[test]
fn serialize_element_with_attribute() {
    let mut b = HastBuilder::new();
    b.open_root();
    let a_id = b.open_element("a");
    let name = b.alloc_string("href");
    let val = b.alloc_string("url");
    b.set_properties(
        a_id,
        &[Property {
            name,
            value: PropertyValue::String(val),
        }],
    );
    b.add_text("text");
    b.close();
    let hast = b.finish();
    assert_eq!(hast_to_html(&hast), "<a href=\"url\">text</a>");
}

#[test]
fn serialize_boolean_attribute_true() {
    let mut b = HastBuilder::new();
    b.open_root();
    let input_id = b.open_element("input");
    let name = b.alloc_string("disabled");
    b.set_properties(
        input_id,
        &[Property {
            name,
            value: PropertyValue::Bool(true),
        }],
    );
    b.close();
    let hast = b.finish();
    assert_eq!(hast_to_html(&hast), "<input disabled />");
}

#[test]
fn serialize_boolean_attribute_false_omitted() {
    let mut b = HastBuilder::new();
    b.open_root();
    let input_id = b.open_element("input");
    let name = b.alloc_string("checked");
    b.set_properties(
        input_id,
        &[Property {
            name,
            value: PropertyValue::Bool(false),
        }],
    );
    b.close();
    let hast = b.finish();
    // false boolean attribute should be omitted entirely
    assert_eq!(hast_to_html(&hast), "<input />");
}

#[test]
fn serialize_void_element_br() {
    let mut b = HastBuilder::new();
    b.open_root();
    b.open_element("br");
    b.close();
    let hast = b.finish();
    assert_eq!(hast_to_html(&hast), "<br />");
}

#[test]
fn serialize_void_element_hr() {
    let mut b = HastBuilder::new();
    b.open_root();
    b.open_element("hr");
    b.close();
    let hast = b.finish();
    assert_eq!(hast_to_html(&hast), "<hr />");
}

#[test]
fn serialize_text_escaping() {
    let mut b = HastBuilder::new();
    b.open_root();
    b.open_element("p");
    b.add_text("<foo> & \"bar\"");
    b.close();
    let hast = b.finish();
    // Double quotes don't need escaping in text content (only in attributes)
    assert_eq!(hast_to_html(&hast), "<p>&lt;foo&gt; &amp; \"bar\"</p>");
}

#[test]
fn serialize_attribute_escaping() {
    let mut b = HastBuilder::new();
    b.open_root();
    let a_id = b.open_element("a");
    let name = b.alloc_string("href");
    let val = b.alloc_string("url?a=1&b=\"2\"");
    b.set_properties(
        a_id,
        &[Property {
            name,
            value: PropertyValue::String(val),
        }],
    );
    b.add_text("link");
    b.close();
    let hast = b.finish();
    assert_eq!(
        hast_to_html(&hast),
        "<a href=\"url?a=1&amp;b=&quot;2&quot;\">link</a>"
    );
}

#[test]
fn serialize_nested_elements() {
    let mut b = HastBuilder::new();
    b.open_root();
    b.open_element("p");
    b.open_element("strong");
    b.add_text("bold");
    b.close(); // strong
    b.close(); // p
    let hast = b.finish();
    assert_eq!(hast_to_html(&hast), "<p><strong>bold</strong></p>");
}

#[test]
fn serialize_multiple_children_in_list() {
    let mut b = HastBuilder::new();
    b.open_root();
    b.open_element("ul");
    b.open_element("li");
    b.add_text("a");
    b.close();
    b.open_element("li");
    b.add_text("b");
    b.close();
    b.close(); // ul
    let hast = b.finish();
    assert_eq!(hast_to_html(&hast), "<ul><li>a</li><li>b</li></ul>");
}

#[test]
fn serialize_comment_node() {
    let mut b = HastBuilder::new();
    b.open_root();
    b.add_comment("comment");
    let hast = b.finish();
    assert_eq!(hast_to_html(&hast), "<!--comment-->");
}

#[test]
fn serialize_doctype() {
    let mut b = HastBuilder::new();
    b.open_root();
    b.add_doctype();
    let hast = b.finish();
    assert_eq!(hast_to_html(&hast), "<!doctype html>");
}

#[test]
fn serialize_raw_passthrough_not_escaped() {
    let mut b = HastBuilder::new();
    b.open_root();
    b.add_raw("<div class=\"foo\">raw &amp; content</div>");
    let hast = b.finish();
    // Raw nodes must not be escaped
    assert_eq!(
        hast_to_html(&hast),
        "<div class=\"foo\">raw &amp; content</div>"
    );
}

#[test]
fn serialize_space_separated_class_property() {
    let mut b = HastBuilder::new();
    b.open_root();
    let span_id = b.open_element("span");
    let name = b.alloc_string("class");
    let val = b.alloc_string("foo bar");
    b.set_properties(
        span_id,
        &[Property {
            name,
            value: PropertyValue::SpaceSeparated(val),
        }],
    );
    b.add_text("text");
    b.close();
    let hast = b.finish();
    assert_eq!(hast_to_html(&hast), "<span class=\"foo bar\">text</span>");
}

fn build_h1_paragraph_mdast() -> tryckeri_mdast::MdastArena {
    let source = "# Hello\n\nWorld".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(MdastNodeType::Root);
    b.open_node(MdastNodeType::Heading);
    b.set_data_current(&encode_heading_data(1));
    b.open_node(MdastNodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(2, 5))); // "Hello"
    b.close_node();
    b.close_node(); // heading
    b.open_node(MdastNodeType::Paragraph);
    b.open_node(MdastNodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(9, 5))); // "World"
    b.close_node();
    b.close_node(); // paragraph
    b.close_node(); // root
    b.finish()
}

#[test]
fn e2e_heading_and_paragraph() {
    let mdast = build_h1_paragraph_mdast();
    let html = mdast_to_html(&mdast);
    assert_eq!(html, "<h1>Hello</h1>\n<p>World</p>");
}

#[test]
fn e2e_link() {
    // "[click](https://example.com)"
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
    b.close_node();
    let _ = link;
    b.close_node(); // link
    b.close_node(); // paragraph
    b.close_node(); // root
    let mdast = b.finish();

    let html = mdast_to_html(&mdast);
    assert_eq!(html, "<p><a href=\"https://example.com\">click</a></p>");
}

#[test]
fn e2e_emphasis() {
    let source = "em".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(MdastNodeType::Root);
    b.open_node(MdastNodeType::Paragraph);
    b.open_node(MdastNodeType::Emphasis);
    b.open_node(MdastNodeType::Text);
    b.set_data_current(&encode_string_ref_data(StringRef::new(0, 2))); // "em"
    b.close_node();
    b.close_node(); // emphasis
    b.close_node(); // paragraph
    b.close_node(); // root
    let mdast = b.finish();

    let html = mdast_to_html(&mdast);
    assert_eq!(html, "<p><em>em</em></p>");
}

#[test]
fn e2e_code_block_with_language() {
    // Code block: lang="rust", value="fn main() {}"
    let source = "rust\nfn main() {}".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(MdastNodeType::Root);
    b.open_node(MdastNodeType::Code);
    b.set_data_current(&encode_code_data(
        StringRef::new(0, 4),  // lang: "rust"
        StringRef::empty(),    // meta: empty
        StringRef::new(5, 12), // value: "fn main() {}"
        b'`',
    ));
    b.close_node(); // code
    b.close_node(); // root
    let mdast = b.finish();

    let html = mdast_to_html(&mdast);
    assert_eq!(
        html,
        "<pre><code class=\"language-rust\">fn main() {}</code></pre>"
    );
}

#[test]
fn e2e_image() {
    let source = "![alt](img.png)".to_string();
    let mut b = MdastBuilder::new(source);
    b.open_node(MdastNodeType::Root);
    b.open_node(MdastNodeType::Paragraph);
    b.open_node(MdastNodeType::Image);
    b.set_data_current(&encode_image_data(
        StringRef::new(7, 7), // "img.png"
        StringRef::new(2, 3), // "alt"
        StringRef::empty(),
    ));
    b.close_node(); // image
    b.close_node(); // paragraph
    b.close_node(); // root
    let mdast = b.finish();

    let html = mdast_to_html(&mdast);
    assert_eq!(html, "<p><img src=\"img.png\" alt=\"alt\" /></p>");
}
