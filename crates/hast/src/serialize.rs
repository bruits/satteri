//! HAST → HTML serialization.

use crate::node::{HastArena, HastNodeType, PropertyValue};

pub fn hast_to_html(hast: &HastArena) -> String {
    let mut out = String::with_capacity(hast.strings.len());
    serialize_node(0, hast, &mut out);
    out
}

fn serialize_node(node_id: u32, hast: &HastArena, out: &mut String) {
    let node = hast.get_node(node_id);

    match node.node_type {
        HastNodeType::Root => {
            for &child_id in hast.get_children(node_id) {
                serialize_node(child_id, hast, out);
            }
        }

        HastNodeType::Element => {
            let tag = if node.tag_name.is_empty() {
                "div"
            } else {
                hast.get_str(node.tag_name)
            };
            let is_void = is_void_element(tag);

            out.push('<');
            out.push_str(tag);

            let props = hast.get_properties(node_id);
            for prop in props {
                if prop.value.is_bool_false() {
                    continue;
                }
                out.push(' ');
                out.push_str(hast.get_str(prop.name));
                match &prop.value {
                    PropertyValue::Bool(true) => {
                        // boolean attribute: just the name, no value
                    }
                    _ => {
                        out.push_str("=\"");
                        let val = hast.get_str(prop.value.as_string_ref());
                        pulldown_cmark_escape::escape_html(&mut *out, val).unwrap();
                        out.push('"');
                    }
                }
            }

            if is_void {
                out.push_str(" />");
            } else {
                out.push('>');
                for &child_id in hast.get_children(node_id) {
                    serialize_node(child_id, hast, out);
                }
                out.push_str("</");
                out.push_str(tag);
                out.push('>');
            }
        }

        HastNodeType::Text => {
            if !node.value.is_empty() {
                let text = hast.get_str(node.value);
                pulldown_cmark_escape::escape_html_body_text(&mut *out, text).unwrap();
            }
        }

        HastNodeType::Raw => {
            if !node.value.is_empty() {
                let html = hast.get_str(node.value);
                out.push_str(html);
            }
        }

        HastNodeType::Comment => {
            if !node.value.is_empty() {
                let text = hast.get_str(node.value);
                out.push_str("<!--");
                out.push_str(text);
                out.push_str("-->");
            }
        }

        HastNodeType::Doctype => {
            out.push_str("<!doctype html>");
        }

        // MDX nodes have no HTML representation.
        HastNodeType::MdxJsxElement
        | HastNodeType::MdxJsxTextElement
        | HastNodeType::MdxExpression
        | HastNodeType::MdxEsm => {}
    }
}

fn is_void_element(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}
