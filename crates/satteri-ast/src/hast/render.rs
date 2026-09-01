//! Render a HAST arena to an HTML string.

use std::borrow::Cow;

use satteri_arena::{Arena, Hast};
use satteri_property_info::{PropKind, find_property};

use crate::hast::codec::{
    decode_element_prop, decode_element_prop_count, decode_element_tag, decode_text_data,
};
use crate::hast::escape::{escape_html_attr_value, escape_html_body_text};
use crate::hast::properties::property_to_attribute;
use crate::hast::{HastNodeType, is_svg_html_integration_point};
use crate::shared::{
    PROP_BOOL_FALSE, PROP_BOOL_TRUE, PROP_COMMA_SEP, PROP_COMMA_SEP_NUM, PROP_INT, PROP_SPACE_SEP,
    PROP_STRING, PROP_TOKEN_LIST,
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
/// (HTML `<script>` / `<style>`). Descendant text of these elements is not
/// entity-escaped; SVG script/style text is escaped.
///
/// `in_svg` selects the SVG attribute schema. Set on entry to `<svg>` and
/// sticky for all descendants — `<foreignObject>` does NOT switch back, matching
/// `hast-util-to-html`. It also sets the initial content namespace, which does
/// switch back to HTML at SVG integration points for text and void-element rules.
pub fn render_node(
    node_id: u32,
    view: &Arena<Hast>,
    out: &mut String,
    in_raw_text: bool,
    in_svg: bool,
) {
    render_node_with_options(node_id, view, out, RenderOptions::new(in_raw_text, in_svg));
}

/// Attribute casing stays in the SVG schema through integration points, but
/// text escaping and void tags follow the content namespace instead.
#[derive(Clone, Copy, Debug, Default)]
pub struct RenderOptions {
    /// Whether text is inside an HTML raw-text element and must remain unescaped.
    pub in_raw_text: bool,
    /// Whether attributes use the SVG schema, including below integration points.
    pub svg_schema: bool,
    /// Whether elements are in SVG content, controlling escaping and void tags.
    pub svg_content: bool,
}

impl RenderOptions {
    pub(crate) fn new(in_raw_text: bool, in_svg: bool) -> Self {
        Self {
            in_raw_text,
            svg_schema: in_svg,
            svg_content: in_svg,
        }
    }

    /// Derive the rendering options for the children of an element named `tag`.
    pub fn for_children(self, tag: &str) -> Self {
        let element_in_svg = self.svg_content || tag == "svg";
        Self {
            in_raw_text: self.in_raw_text || (!element_in_svg && is_raw_text_element(tag)),
            svg_schema: self.svg_schema || tag == "svg",
            svg_content: element_in_svg && !is_svg_html_integration_point(tag),
        }
    }
}

/// Render a subtree with separate attribute-schema and content-namespace options.
pub fn render_node_with_options(
    node_id: u32,
    view: &Arena<Hast>,
    out: &mut String,
    options: RenderOptions,
) {
    render_node_inner(node_id, view, out, options, None, 0);
}

/// Raw-HTML reparse hook: receives the output buffer and the MDX node's id.
pub(crate) type OnMdx<'a> = dyn FnMut(&mut String, u32) + 'a;

/// MDX nodes have no HTML representation: `on_mdx` decides what to emit for
/// them; `None` skips them.
pub(crate) fn render_node_inner<'cb>(
    node_id: u32,
    view: &Arena<Hast>,
    out: &mut String,
    context: RenderOptions,
    on_mdx: Option<&mut OnMdx<'cb>>,
    depth: u32,
) {
    crate::stack::with_headroom(depth, || {
        render_node_at(node_id, view, out, context, on_mdx, depth);
    });
}

fn render_node_at<'cb>(
    node_id: u32,
    view: &Arena<Hast>,
    out: &mut String,
    context: RenderOptions,
    mut on_mdx: Option<&mut OnMdx<'cb>>,
    depth: u32,
) {
    let node = view.get_node(node_id);

    let Some(node_type) = HastNodeType::from_u8(node.node_type) else {
        for &child_id in view.get_children(node_id) {
            render_node_inner(
                child_id,
                view,
                out,
                context,
                on_mdx.as_deref_mut(),
                depth + 1,
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
                    context,
                    on_mdx.as_deref_mut(),
                    depth + 1,
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
            let svg_schema = context.svg_schema || tag == "svg";
            let element_in_svg = context.svg_content || tag == "svg";

            out.push('<');
            out.push_str(tag);

            let prop_count = decode_element_prop_count(data);
            for i in 0..prop_count {
                let (name_ref, value_kind, value_ref) = decode_element_prop(data, i);
                let name = view.get_str(name_ref);
                let attr_name = property_to_attribute(name, svg_schema);
                match value_kind {
                    PROP_BOOL_TRUE => {
                        out.push(' ');
                        out.push_str(&attr_name);
                    }
                    PROP_BOOL_FALSE => {}
                    PROP_STRING | PROP_INT | PROP_SPACE_SEP | PROP_COMMA_SEP
                    | PROP_COMMA_SEP_NUM | PROP_TOKEN_LIST => {
                        let stored = view.get_str(value_ref);
                        let value = if value_kind == PROP_TOKEN_LIST {
                            Cow::Owned(join_token_list(name, element_in_svg, stored))
                        } else {
                            Cow::Borrowed(stored)
                        };
                        out.push(' ');
                        out.push_str(&attr_name);
                        out.push_str("=\"");
                        escape_html_attr_value(out, &value);
                        out.push('"');
                    }
                    _ => {}
                }
            }

            if !element_in_svg && is_void_element(tag) {
                out.push('>');
            } else {
                out.push('>');
                let child_context = context.for_children(tag);
                for &child_id in view.get_children(node_id) {
                    render_node_inner(
                        child_id,
                        view,
                        out,
                        child_context,
                        on_mdx.as_deref_mut(),
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
                if context.in_raw_text {
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

        HastNodeType::MdxJsxElement
        | HastNodeType::MdxJsxTextElement
        | HastNodeType::MdxFlowExpression
        | HastNodeType::MdxTextExpression
        | HastNodeType::MdxEsm => {
            if let Some(cb) = on_mdx.as_mut() {
                cb(out, node_id);
            }
        }
    }
}

/// Split a `PROP_TOKEN_LIST` value: every token is NUL-terminated, so an empty
/// value is an empty list and a lone NUL is a list holding one empty token.
fn token_list_items(tokens: &str) -> Vec<&str> {
    if tokens.is_empty() {
        return Vec::new();
    }
    tokens
        .strip_suffix('\0')
        .unwrap_or(tokens)
        .split('\0')
        .collect()
}

/// Join a JS-built list property, whose tokens ride the wire unjoined because
/// only the render knows the element's schema: `coords` is comma-separated in
/// HTML and plain in SVG, `glyphName` the reverse. Mirrors
/// `comma-separated-tokens` and `space-separated-tokens`; the trailing empty
/// item each pads with is already in the tokens (see `encodeTokenList`).
pub fn join_token_list(name: &str, in_svg: bool, tokens: &str) -> String {
    let items = token_list_items(tokens);
    let comma_separated = matches!(
        find_property(name, in_svg).1,
        PropKind::CommaSeparated | PropKind::NumberCommaSeparated
    );
    let joined = items.join(if comma_separated { ", " } else { " " });
    // `String.prototype.trim` counts the BOM as whitespace and `str::trim` does not.
    joined
        .trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
        .to_string()
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
