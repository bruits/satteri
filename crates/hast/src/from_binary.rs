//! Convert a HAST binary buffer to an HTML string.

use tryckeri_mdast::{BufferError, MdastArena, MdastView};

use crate::codec::{
    decode_element_prop, decode_element_prop_count, decode_element_tag, decode_text_data,
};
use crate::node_types::*;

fn is_void(tag: &str) -> bool {
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

pub fn hast_buffer_to_html(buf: &[u8]) -> Result<String, BufferError> {
    let view = MdastArena::from_raw_buffer(buf)?;
    // Pre-allocate: source len is a reasonable lower bound for HTML output size
    let mut out = String::with_capacity(view.get_source().len());
    render_node(0, &view, &mut out);
    Ok(out)
}

/// Also used by the MDX compiler's static optimization pass to
/// serialize static subtrees into raw HTML.
pub fn render_node(node_id: u32, view: &MdastView, out: &mut String) {
    let node = view.get_node(node_id);
    let raw_type = node.node_type;

    match raw_type {
        HAST_ROOT => {
            for &child_id in view.get_children(node_id) {
                render_node(child_id, view, out);
            }
        }

        HAST_ELEMENT => {
            let data = view.get_type_data(node_id);
            if data.len() < 16 {
                // malformed — skip
                return;
            }
            let tag_ref = decode_element_tag(data);
            let tag = view.get_str(tag_ref);

            out.push('<');
            out.push_str(tag);

            let prop_count = decode_element_prop_count(data);
            for i in 0..prop_count {
                let (name_ref, value_kind, value_ref) = decode_element_prop(data, i);
                let name = view.get_str(name_ref);
                match value_kind {
                    PROP_BOOL_TRUE => {
                        out.push(' ');
                        out.push_str(name);
                    }
                    PROP_BOOL_FALSE => {}
                    PROP_STRING | PROP_SPACE_SEP | PROP_COMMA_SEP => {
                        let value = view.get_str(value_ref);
                        out.push(' ');
                        out.push_str(name);
                        out.push_str("=\"");
                        pulldown_cmark_escape::escape_html(&mut *out, value).unwrap();
                        out.push('"');
                    }
                    _ => {}
                }
            }

            if is_void(tag) {
                out.push('>');
            } else {
                out.push('>');
                for &child_id in view.get_children(node_id) {
                    render_node(child_id, view, out);
                }
                out.push_str("</");
                out.push_str(tag);
                out.push('>');
            }
        }

        HAST_TEXT => {
            let data = view.get_type_data(node_id);
            if data.len() >= 8 {
                let sr = decode_text_data(data);
                let text = view.get_str(sr);
                pulldown_cmark_escape::escape_html_body_text(&mut *out, text).unwrap();
            }
        }

        HAST_COMMENT => {
            let data = view.get_type_data(node_id);
            if data.len() >= 8 {
                let sr = decode_text_data(data);
                let text = view.get_str(sr);
                out.push_str("<!--");
                out.push_str(text);
                out.push_str("-->");
            }
        }

        HAST_DOCTYPE => {
            out.push_str("<!doctype html>");
        }

        HAST_RAW => {
            let data = view.get_type_data(node_id);
            if data.len() >= 8 {
                let sr = decode_text_data(data);
                let html = view.get_str(sr);
                out.push_str(html);
            }
        }

        HAST_MDX_JSX_ELEMENT | HAST_MDX_JSX_TEXT_ELEMENT | HAST_MDX_EXPRESSION | HAST_MDX_ESM => {
            // MDX nodes have no HTML representation — they're only used
            // in the MDX→JS compilation path.
        }

        _ => {
            // Unknown node type — recurse into children if any
            for &child_id in view.get_children(node_id) {
                render_node(child_id, view, out);
            }
        }
    }
}
