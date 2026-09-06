//! Render a HAST arena to an HTML string.

use satteri_arena::{Arena, Hast};

use crate::hast::HastNodeType;
use crate::hast::codec::{
    decode_element_prop, decode_element_prop_count, decode_element_tag, decode_text_data,
};
use crate::hast::escape::{escape_html_attr_value, escape_html_body_text};
use crate::hast::properties::property_to_attribute;
use crate::shared::{
    PROP_BOOL_FALSE, PROP_BOOL_TRUE, PROP_COMMA_SEP, PROP_COMMA_SEP_NUM, PROP_INT, PROP_SPACE_SEP,
    PROP_STRING,
};

/// Render HTML from an arena.
pub fn hast_arena_to_html(arena: &Arena<Hast>) -> String {
    let mut out = String::with_capacity(arena.string_pool().len());
    render_node(0, arena, &mut out, false, false);
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Render a HAST node subtree to HTML.
///
/// `in_raw_text` indicates the node is being rendered inside a raw-text element
/// (`<script>` / `<style>`). Per the HTML spec, descendant text of these elements
/// is not entity-escaped.
///
/// `in_svg` selects the SVG attribute schema. Set on entry to `<svg>` and
/// sticky for all descendants — `<foreignObject>` does NOT switch back, matching
/// `hast-util-to-html`.
pub fn render_node(
    node_id: u32,
    view: &Arena<Hast>,
    out: &mut String,
    in_raw_text: bool,
    in_svg: bool,
) {
    render_node_inner(node_id, view, out, in_raw_text, in_svg, 0);
}

fn render_node_inner(
    node_id: u32,
    view: &Arena<Hast>,
    out: &mut String,
    in_raw_text: bool,
    in_svg: bool,
    depth: u32,
) {
    crate::stack::with_headroom(depth, || {
        render_node_at(node_id, view, out, in_raw_text, in_svg, depth);
    });
}

fn render_node_at(
    node_id: u32,
    view: &Arena<Hast>,
    out: &mut String,
    in_raw_text: bool,
    in_svg: bool,
    depth: u32,
) {
    let node = view.get_node(node_id);

    let Some(node_type) = HastNodeType::from_u8(node.node_type) else {
        for &child_id in view.get_children(node_id) {
            render_node_inner(child_id, view, out, in_raw_text, in_svg, depth + 1);
        }
        return;
    };

    match node_type {
        HastNodeType::Root => {
            for &child_id in view.get_children(node_id) {
                render_node_inner(child_id, view, out, in_raw_text, in_svg, depth + 1);
            }
        }

        HastNodeType::Element => {
            let data = view.get_type_data(node_id);
            if data.len() < 16 {
                return;
            }
            let tag_ref = decode_element_tag(data);
            let tag = view.get_str(tag_ref);

            // The schema switch covers the <svg> element's own attributes too,
            // not just its descendants.
            let element_in_svg = in_svg || tag == "svg";

            out.push('<');
            out.push_str(tag);

            let prop_count = decode_element_prop_count(data);
            for i in 0..prop_count {
                let (name_ref, value_kind, value_ref) = decode_element_prop(data, i);
                let name = view.get_str(name_ref);
                let attr_name = property_to_attribute(name, element_in_svg);
                match value_kind {
                    PROP_BOOL_TRUE => {
                        out.push(' ');
                        out.push_str(&attr_name);
                    }
                    PROP_BOOL_FALSE => {}
                    PROP_STRING | PROP_INT | PROP_SPACE_SEP | PROP_COMMA_SEP
                    | PROP_COMMA_SEP_NUM => {
                        let value = view.get_str(value_ref);
                        out.push(' ');
                        out.push_str(&attr_name);
                        out.push_str("=\"");
                        escape_html_attr_value(out, value);
                        out.push('"');
                    }
                    _ => {}
                }
            }

            if is_void_element(tag) {
                out.push('>');
            } else {
                out.push('>');
                let child_in_raw_text = in_raw_text || is_raw_text_element(tag);
                for &child_id in view.get_children(node_id) {
                    render_node_inner(
                        child_id,
                        view,
                        out,
                        child_in_raw_text,
                        element_in_svg,
                        depth + 1,
                    );
                }
                out.push_str("</");
                out.push_str(tag);
                out.push('>');
            }
        }

        HastNodeType::Text => {
            let data = view.get_type_data(node_id);
            if data.len() >= 8 {
                let sr = decode_text_data(data);
                let text = view.get_str(sr);
                if in_raw_text {
                    out.push_str(text);
                } else {
                    escape_html_body_text(out, text);
                }
            }
        }

        HastNodeType::Comment => {
            let data = view.get_type_data(node_id);
            if data.len() >= 8 {
                let sr = decode_text_data(data);
                let text = view.get_str(sr);
                out.push_str("<!--");
                out.push_str(text);
                out.push_str("-->");
            }
        }

        HastNodeType::Doctype => {
            out.push_str("<!doctype html>");
        }

        HastNodeType::Raw => {
            let data = view.get_type_data(node_id);
            if data.len() >= 8 {
                let sr = decode_text_data(data);
                let html = view.get_str(sr);
                out.push_str(html);
            }
        }

        // MDX nodes have no HTML form.
        HastNodeType::MdxJsxElement
        | HastNodeType::MdxJsxTextElement
        | HastNodeType::MdxFlowExpression
        | HastNodeType::MdxTextExpression
        | HastNodeType::MdxEsm => {}
    }
}

/// Void elements render as a single tag; any children never reach the output.
pub fn is_void_element(tag: &str) -> bool {
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

/// Raw-text elements whose children are not entity-escaped on output, per the
/// WHATWG HTML serialization algorithm.
fn is_raw_text_element(tag: &str) -> bool {
    matches!(
        tag,
        "script" | "style" | "xmp" | "iframe" | "noembed" | "noframes" | "plaintext"
    )
}
