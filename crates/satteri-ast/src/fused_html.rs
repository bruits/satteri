//! Emit HTML straight from an MDAST arena, with no intermediate HAST arena.
//!
//! Output is byte-identical to `mdast_arena_to_hast_arena` + `hast_arena_to_html`.

use satteri_arena::{Arena, Mdast, StringRef};

use crate::convert::{ConvertOptions, collect_refs, contains_h_key, trim_lines_for_hast};
use crate::emit::{AttrName, AttrValue, Children, ConvertSink, EmitCtx, Pos, emit_node};
use crate::hast::escape::{escape_html_attr_value, escape_html_body_text};
use crate::mdast::MdastNodeType;

/// Render `view` to HTML, or `None` when the document or options need the two-stage pipeline.
pub(crate) fn mdast_to_html_fused(view: &Arena<Mdast>, options: &ConvertOptions) -> Option<String> {
    if view.is_empty() || view.node_data.values().any(|blob| contains_h_key(blob)) {
        return None;
    }
    #[cfg(feature = "from-html")]
    if options.raw_html {
        return None;
    }
    let refs = collect_refs(view);
    let ctx = EmitCtx {
        view,
        refs: &refs,
        options,
    };
    let mut sink = HtmlSink {
        out: String::with_capacity(view.string_pool().len()),
        view,
        trim: Trim::None,
    };
    emit_node(0, &ctx, &mut sink, 0);
    Some(sink.finish())
}

/// After a `Break`, only the next sibling's own text or its element's first text child is trimmed.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Trim {
    None,
    Node,
    FirstChild,
}

/// The sink that writes HTML bytes, allocating nothing per element.
struct HtmlSink<'a> {
    out: String,
    view: &'a Arena<Mdast>,
    trim: Trim,
}

impl HtmlSink<'_> {
    fn finish(self) -> String {
        let mut out = self.out;
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out
    }

    #[inline]
    fn take_trim(&mut self) -> bool {
        let pending = self.trim != Trim::None;
        self.trim = Trim::None;
        pending
    }

    fn push_text(&mut self, text: &str) {
        let text = if self.take_trim() {
            text.trim_start_matches([' ', '\t'])
        } else {
            text
        };
        escape_html_body_text(&mut self.out, text);
    }

    #[inline]
    fn open_tag(&mut self, tag: &'static str) {
        self.out.push('<');
        self.out.push_str(tag);
    }

    fn write_attr(&mut self, name: &'static str, prefix: &'static str, value: &str) {
        self.out.push(' ');
        self.out.push_str(name);
        self.out.push_str("=\"");
        self.out.push_str(prefix);
        escape_html_attr_value(&mut self.out, value);
        self.out.push('"');
    }
}

impl ConvertSink for HtmlSink<'_> {
    type BreakMark = ();

    #[inline(always)]
    fn open_root(&mut self, _pos: Pos) {}

    #[inline(always)]
    fn close_root(&mut self) {}

    #[inline]
    fn open_element(&mut self, tag: &'static str, _pos: Pos) {
        self.trim = if self.trim == Trim::Node {
            Trim::FirstChild
        } else {
            Trim::None
        };
        self.open_tag(tag);
    }

    #[inline]
    fn open_source_element(&mut self, tag: &'static str, _src_id: u32) {
        self.open_element(tag, Pos::None);
    }

    #[inline]
    fn open_void(&mut self, tag: &'static str, _pos: Pos) {
        self.trim = Trim::None;
        self.open_tag(tag);
    }

    #[inline]
    fn open_source_void(&mut self, tag: &'static str, _src_id: u32) {
        self.open_void(tag, Pos::None);
    }

    #[inline]
    fn attr(&mut self, name: AttrName, value: AttrValue<'_>) {
        match value {
            AttrValue::Pooled(_, sref) => {
                let view = self.view;
                self.write_attr(name.attribute, "", view.get_str(sref));
            }
            AttrValue::Text(_, text) => self.write_attr(name.attribute, "", text),
            AttrValue::Prefixed(_, prefix, tail) => self.write_attr(name.attribute, prefix, tail),
            AttrValue::Flag(true) => {
                self.out.push(' ');
                self.out.push_str(name.attribute);
            }
            AttrValue::Flag(false) => {}
        }
    }

    #[inline]
    fn finish_attrs(&mut self) {
        self.out.push('>');
    }

    #[inline(always)]
    fn finish_source_attrs(&mut self) -> Children {
        self.out.push('>');
        Children::Recurse
    }

    #[inline]
    fn finish_void(&mut self) {
        self.out.push('>');
    }

    #[inline]
    fn finish_source_void(&mut self) {
        self.out.push('>');
    }

    #[inline]
    fn close_element(&mut self, tag: &'static str) {
        if self.trim == Trim::FirstChild {
            self.trim = Trim::None;
        }
        self.out.push_str("</");
        self.out.push_str(tag);
        self.out.push('>');
    }

    fn text(&mut self, value: &str, _pos: Pos) {
        self.push_text(value);
    }

    fn text_pooled(&mut self, value: StringRef, _pos: Pos) {
        let view = self.view;
        self.push_text(view.get_str(value));
    }

    fn text_trimmed(&mut self, value: StringRef, _pos: Pos) {
        let view = self.view;
        self.push_text(&trim_lines_for_hast(view.get_str(value)));
    }

    fn text_with_trailing_space(&mut self, value: StringRef, _pos: Pos) {
        let view = self.view;
        self.push_text(view.get_str(value));
        self.out.push(' ');
    }

    fn code_block_text(&mut self, value: &str, _pos: Pos) {
        self.push_text(value);
        if !value.is_empty() {
            self.out.push('\n');
        }
    }

    fn raw_html(&mut self, value: &str, _pos: Pos) {
        self.trim = Trim::None;
        self.out.push_str(value);
    }

    #[inline]
    fn newline(&mut self) {
        self.trim = Trim::None;
        self.out.push('\n');
    }

    #[inline(always)]
    fn code_info(&mut self, _lang: StringRef, _meta: StringRef) {}

    #[inline]
    fn mark_break_boundary(&mut self, after_break: bool) {
        if after_break {
            self.trim = Trim::Node;
        }
    }

    #[inline]
    fn apply_break_trim(&mut self, after_break: bool, _mark: ()) {
        if after_break {
            self.trim = Trim::None;
        }
    }

    /// Mirrors `convert::produces_hast_output`: without an `hName` a directive never renders.
    fn produces_output(&self, child_id: u32) -> bool {
        !matches!(
            MdastNodeType::from_u8(self.view.get_node(child_id).node_type),
            Some(
                MdastNodeType::Definition
                    | MdastNodeType::Yaml
                    | MdastNodeType::Toml
                    | MdastNodeType::FootnoteDefinition
                    | MdastNodeType::ContainerDirective
                    | MdastNodeType::LeafDirective
                    | MdastNodeType::TextDirective
            )
        )
    }

    #[inline(always)]
    fn has_no_h_data(&self, _node_id: u32) -> bool {
        true
    }

    #[inline(always)]
    fn open_directive(&mut self, _node_id: u32) -> Option<Children> {
        None
    }

    #[inline(always)]
    fn close_dynamic_element(&mut self) {}

    #[cfg(feature = "mdx")]
    #[inline(always)]
    fn open_mdx_jsx_element(&mut self, _node_id: u32, _node_type: MdastNodeType) -> bool {
        false
    }

    #[cfg(feature = "mdx")]
    #[inline(always)]
    fn mdx_leaf(&mut self, _node_id: u32, _node_type: MdastNodeType) {}
}
