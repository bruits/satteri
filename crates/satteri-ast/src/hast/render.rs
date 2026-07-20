//! Render a HAST arena to an HTML string.

use satteri_arena::{Arena, Hast};

use crate::hast::codec::{
    decode_element_prop, decode_element_prop_count, decode_element_tag, decode_text_data,
};
use crate::hast::properties::property_to_attribute;
use crate::hast::HastNodeType;
use crate::shared::{
    PROP_BOOL_FALSE, PROP_BOOL_TRUE, PROP_COMMA_SEP, PROP_INT, PROP_SPACE_SEP, PROP_STRING,
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

/// Escape a string to appear inside a double-quoted HTML attribute value,
/// matching hast-util-to-html's default "safe" serialization. Encodes `&`,
/// `"`, `'`, and `` ` `` (backtick is escaped because some legacy browsers
/// treat it as an attribute-value delimiter). Unlike body-text escaping,
/// `<` and `>` are kept as-is since they're valid inside attribute values.
fn escape_html_attr_value(out: &mut String, value: &str) {
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            '`' => out.push_str("&#x60;"),
            _ => out.push(c),
        }
    }
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
    render_node_inner(node_id, view, out, in_raw_text, in_svg, None);
}

/// Comment marker used to carry non-HTML nodes (MDX) through the raw-HTML
/// reparse. [`crate::hast::from_html`] serialises each such node as
/// `<!--{STITCH_COMMENT_PREFIX}{index}-->`, parses that alongside the rest of
/// the tree, then swaps each parsed placeholder comment back for the original
/// subtree. Mirrors `hast-util-raw`'s "stitches".
pub(crate) const STITCH_COMMENT_PREFIX: &str = "satteri:stitch:";

/// Render a HAST node subtree to HTML.
///
/// When `stitches` is `Some`, MDX nodes — which have no HTML representation —
/// are emitted as placeholder comments and their ids pushed onto the
/// accumulator, so the raw-HTML reparse can restore them instead of dropping
/// them. When `None` (normal rendering), MDX nodes are skipped as before.
pub(crate) fn render_node_inner(
    node_id: u32,
    view: &Arena<Hast>,
    out: &mut String,
    in_raw_text: bool,
    in_svg: bool,
    mut stitches: Option<&mut Vec<u32>>,
) {
    let node = view.get_node(node_id);

    let Some(node_type) = HastNodeType::from_u8(node.node_type) else {
        for &child_id in view.get_children(node_id) {
            render_node_inner(
                child_id,
                view,
                out,
                in_raw_text,
                in_svg,
                stitches.as_mut().map(|s| &mut **s),
            );
        }
        return;
    };

    match node_type {
        HastNodeType::Root => {
            for &child_id in view.get_children(node_id) {
                render_node_inner(
                    child_id,
                    view,
                    out,
                    in_raw_text,
                    in_svg,
                    stitches.as_mut().map(|s| &mut **s),
                );
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
                    PROP_STRING | PROP_INT | PROP_SPACE_SEP | PROP_COMMA_SEP => {
                        let value = view.get_str(value_ref);
                        out.push(' ');
                        out.push_str(&attr_name);
                        out.push_str("=\"");
                        escape_html_attr_value(&mut *out, value);
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
                        stitches.as_mut().map(|s| &mut **s),
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
                    pulldown_cmark_escape::escape_html_body_text(&mut *out, text).unwrap();
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

        // MDX nodes have no HTML representation. In a normal render (`stitches`
        // is `None`) they are skipped. During the raw-HTML reparse (`stitches`
        // is `Some`) each is emitted as a placeholder comment and its id
        // recorded, so the reparse can restore the original node afterwards
        // rather than destroying it — mirroring `hast-util-raw`'s passthrough.
        HastNodeType::MdxJsxElement
        | HastNodeType::MdxJsxTextElement
        | HastNodeType::MdxFlowExpression
        | HastNodeType::MdxTextExpression
        | HastNodeType::MdxEsm => {
            if let Some(stitches) = stitches.as_mut() {
                let index = stitches.len();
                stitches.push(node_id);
                out.push_str("<!--");
                out.push_str(STITCH_COMMENT_PREFIX);
                out.push_str(&index.to_string());
                out.push_str("-->");
            }
        }
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

/// Raw-text elements whose children are not entity-escaped on output, per the
/// WHATWG HTML serialization algorithm.
fn is_raw_text_element(tag: &str) -> bool {
    matches!(
        tag,
        "script" | "style" | "xmp" | "iframe" | "noembed" | "noframes" | "plaintext"
    )
}
