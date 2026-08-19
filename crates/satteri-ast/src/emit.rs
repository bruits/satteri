//! The mdast → element mapping, written once and driven into either a HAST arena or an HTML string.

use core::fmt::Write;

use satteri_arena::{Arena, Mdast, StringRef, decode_string_ref_data};

use crate::convert::{
    Backref, CollectedRefs, ConvertOptions, code_span_line_endings_to_spaces, extract_text_content,
    footnote_fragment_id, list_contains_task_item, normalize_url,
};
use crate::mdast::{
    ColumnAlign, ListData, ListItemData, MdastNodeType, decode_code_data, decode_custom_data,
    decode_description_details_data, decode_footnote_definition_data, decode_heading_data,
    decode_image_data, decode_image_reference_alt, decode_link_data, decode_list_data,
    decode_list_item_data, decode_math_data, decode_reference_data, decode_table_alignments,
};
use crate::shared::{PROP_INT, PROP_SPACE_SEP, PROP_STRING};

#[derive(Clone, Copy)]
pub(crate) enum Pos {
    None,
    Node(u32),
    Span(u32, u32),
}

/// Whether the sink already emitted children of its own in place of the mdast ones.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Children {
    Recurse,
    Replaced,
}

/// Both names an attribute is known by, so one mapping can feed a property list and a byte stream.
#[derive(Clone, Copy)]
pub(crate) struct AttrName {
    pub(crate) property: &'static str,
    pub(crate) attribute: &'static str,
}

pub(crate) const ALT: AttrName = AttrName {
    property: "alt",
    attribute: "alt",
};
pub(crate) const ARIA_DESCRIBED_BY: AttrName = AttrName {
    property: "ariaDescribedBy",
    attribute: "aria-describedby",
};
pub(crate) const ARIA_LABEL: AttrName = AttrName {
    property: "ariaLabel",
    attribute: "aria-label",
};
pub(crate) const CHECKED: AttrName = AttrName {
    property: "checked",
    attribute: "checked",
};
pub(crate) const CLASS_NAME: AttrName = AttrName {
    property: "className",
    attribute: "class",
};
pub(crate) const DATA_FOOTNOTES: AttrName = AttrName {
    property: "dataFootnotes",
    attribute: "data-footnotes",
};
pub(crate) const DATA_FOOTNOTE_BACKREF: AttrName = AttrName {
    property: "dataFootnoteBackref",
    attribute: "data-footnote-backref",
};
pub(crate) const DATA_FOOTNOTE_REF: AttrName = AttrName {
    property: "dataFootnoteRef",
    attribute: "data-footnote-ref",
};
pub(crate) const DISABLED: AttrName = AttrName {
    property: "disabled",
    attribute: "disabled",
};
pub(crate) const HREF: AttrName = AttrName {
    property: "href",
    attribute: "href",
};
pub(crate) const ID: AttrName = AttrName {
    property: "id",
    attribute: "id",
};
pub(crate) const SRC: AttrName = AttrName {
    property: "src",
    attribute: "src",
};
pub(crate) const START: AttrName = AttrName {
    property: "start",
    attribute: "start",
};
pub(crate) const STYLE: AttrName = AttrName {
    property: "style",
    attribute: "style",
};
pub(crate) const TITLE: AttrName = AttrName {
    property: "title",
    attribute: "title",
};
pub(crate) const TYPE: AttrName = AttrName {
    property: "type",
    attribute: "type",
};

#[derive(Clone, Copy)]
pub(crate) enum AttrValue<'a> {
    /// Already interned in the mdast pool, so neither sink copies it.
    Pooled(u8, StringRef),
    Text(u8, &'a str),
    /// A static head and a dynamic tail, so the streaming sink never formats.
    Prefixed(u8, &'static str, &'a str),
    Flag(bool),
}

impl<'a> AttrValue<'a> {
    #[inline]
    pub(crate) fn text(value: &'a str) -> Self {
        AttrValue::Text(PROP_STRING, value)
    }

    #[inline]
    pub(crate) fn class_list(value: &'a str) -> Self {
        AttrValue::Text(PROP_SPACE_SEP, value)
    }

    #[inline]
    pub(crate) fn number(value: &'a str) -> Self {
        AttrValue::Text(PROP_INT, value)
    }

    #[inline]
    pub(crate) fn pooled(value: StringRef) -> Self {
        AttrValue::Pooled(PROP_STRING, value)
    }
}

/// Opening is split into `open` / `attr`* / `finish` so one sink can collect a property list while the other streams bytes.
pub(crate) trait ConvertSink {
    /// What the sink needs to remember across a child to undo a post-`Break` trim.
    type BreakMark: Copy;

    fn open_root(&mut self, pos: Pos);
    fn close_root(&mut self);

    fn open_element(&mut self, tag: &'static str, pos: Pos);
    /// `source` elements are the ones whose mdast node may carry `hName` / `hProperties` / `hChildren`.
    fn open_source_element(&mut self, tag: &'static str, src_id: u32);
    fn open_void(&mut self, tag: &'static str, pos: Pos);
    fn open_source_void(&mut self, tag: &'static str, src_id: u32);

    fn attr(&mut self, name: AttrName, value: AttrValue<'_>);

    fn finish_attrs(&mut self);
    fn finish_source_attrs(&mut self) -> Children;
    fn finish_void(&mut self);
    fn finish_source_void(&mut self);
    fn close_element(&mut self, tag: &'static str);

    fn text(&mut self, value: &str, pos: Pos);
    fn text_pooled(&mut self, value: StringRef, pos: Pos);
    fn text_trimmed(&mut self, value: StringRef, pos: Pos);
    /// One text run, not two, so the space merges into the value.
    fn text_with_trailing_space(&mut self, value: StringRef, pos: Pos);
    /// A code block's body, which gains a trailing newline when non-empty.
    fn code_block_text(&mut self, value: &str, pos: Pos);
    fn raw_html(&mut self, value: &str, pos: Pos);
    fn newline(&mut self);

    fn code_info(&mut self, lang: StringRef, meta: StringRef);

    fn mark_break_boundary(&mut self, after_break: bool) -> Self::BreakMark;
    fn apply_break_trim(&mut self, after_break: bool, mark: Self::BreakMark);

    fn produces_output(&self, child_id: u32) -> bool;
    fn has_no_h_data(&self, node_id: u32) -> bool;

    /// A directive renders only through `hName`, so a sink that ignores node data drops the subtree.
    fn open_directive(&mut self, node_id: u32) -> Option<Children>;
    fn close_dynamic_element(&mut self);

    #[cfg(feature = "mdx")]
    fn open_mdx_jsx_element(&mut self, node_id: u32, node_type: MdastNodeType) -> bool;
    #[cfg(feature = "mdx")]
    fn mdx_leaf(&mut self, node_id: u32, node_type: MdastNodeType);
}

pub(crate) struct EmitCtx<'a, 'src> {
    pub(crate) view: &'a Arena<Mdast>,
    pub(crate) refs: &'a CollectedRefs<'src>,
    pub(crate) options: &'a ConvertOptions,
}

#[inline]
fn open_plain<S: ConvertSink>(sink: &mut S, tag: &'static str, pos: Pos) {
    sink.open_element(tag, pos);
    sink.finish_attrs();
}

/// Plugins build list nodes by hand, so every list decode is sized before it reads.
#[inline]
fn list_data_of(node_id: u32, view: &Arena<Mdast>) -> Option<ListData> {
    let data = view.get_type_data(node_id);
    (data.len() >= size_of::<ListData>()).then(|| decode_list_data(data))
}

#[inline]
fn list_item_data_of(node_id: u32, view: &Arena<Mdast>) -> Option<ListItemData> {
    let data = view.get_type_data(node_id);
    (data.len() >= size_of::<ListItemData>()).then(|| decode_list_item_data(data))
}

fn list_is_loose(list_id: u32, view: &Arena<Mdast>) -> bool {
    list_data_of(list_id, view).is_some_and(|d| d.spread)
        || view
            .get_children(list_id)
            .iter()
            .any(|&item_id| list_item_data_of(item_id, view).is_some_and(|d| d.spread))
}

/// A list item can be reparented onto anything, including nothing.
fn enclosing_list_is_loose(node_id: u32, view: &Arena<Mdast>) -> bool {
    let parent_id = view.get_node(node_id).parent;
    if parent_id as usize >= view.len() {
        return false;
    }
    list_is_loose(parent_id, view)
}

pub(crate) fn emit_node<S: ConvertSink>(
    node_id: u32,
    ctx: &EmitCtx<'_, '_>,
    sink: &mut S,
    depth: u32,
) {
    crate::stack::with_headroom(depth, || emit_node_at(node_id, ctx, sink, depth));
}

fn emit_node_at<S: ConvertSink>(node_id: u32, ctx: &EmitCtx<'_, '_>, sink: &mut S, depth: u32) {
    let view = ctx.view;
    match MdastNodeType::from_u8(view.get_node(node_id).node_type) {
        Some(MdastNodeType::Root) => {
            sink.open_root(Pos::Node(node_id));
            emit_children_wrapped(node_id, ctx, sink, depth);
            emit_footnotes_section(ctx, sink, depth);
            sink.close_root();
        }

        Some(MdastNodeType::Paragraph) => emit_inline_wrapper(node_id, "p", ctx, sink, depth),

        Some(MdastNodeType::Heading) => {
            let data = view.get_type_data(node_id);
            let level = if data.is_empty() {
                1
            } else {
                decode_heading_data(data).depth
            };
            let tag = match level {
                1 => "h1",
                2 => "h2",
                3 => "h3",
                4 => "h4",
                5 => "h5",
                _ => "h6",
            };
            emit_inline_wrapper(node_id, tag, ctx, sink, depth);
        }

        Some(MdastNodeType::ThematicBreak) => {
            sink.open_source_void("hr", node_id);
            sink.finish_source_void();
        }

        Some(MdastNodeType::Blockquote) => {
            sink.open_source_element("blockquote", node_id);
            if sink.finish_source_attrs() == Children::Recurse {
                emit_children_with_newlines(node_id, ctx, sink, depth);
            }
            sink.close_element("blockquote");
        }

        Some(MdastNodeType::List) => {
            let list_data = list_data_of(node_id, view);
            let tag = if list_data.is_some_and(|d| d.ordered) {
                "ol"
            } else {
                "ul"
            };
            let has_task_items = list_contains_task_item(node_id, view);
            sink.open_source_element(tag, node_id);
            if let Some(d) = list_data.filter(|d| d.ordered && d.start != 1) {
                let start = d.start.to_string();
                sink.attr(START, AttrValue::number(&start));
            }
            if has_task_items {
                sink.attr(CLASS_NAME, AttrValue::class_list("contains-task-list"));
            }
            if sink.finish_source_attrs() == Children::Recurse {
                emit_list_items(node_id, ctx, sink, depth);
            }
            sink.close_element(tag);
        }

        Some(MdastNodeType::ListItem) => {
            emit_list_item(
                node_id,
                enclosing_list_is_loose(node_id, view),
                ctx,
                sink,
                depth,
            );
        }

        Some(MdastNodeType::Html) => {
            let value = view.get_str(decode_string_ref_data(view.get_type_data(node_id)));
            sink.raw_html(value, Pos::Node(node_id));
        }

        Some(MdastNodeType::Code) => {
            let code_data = decode_code_data(view.get_type_data(node_id));
            sink.open_source_element("pre", node_id);
            if sink.finish_source_attrs() == Children::Replaced {
                sink.close_element("pre");
                return;
            }
            sink.open_element("code", Pos::Node(node_id));
            if code_data.lang.len > 0 {
                let lang = view.get_str(code_data.lang);
                // A character reference can decode to whitespace, so only the first word names the language.
                let first = lang.split(char::is_whitespace).next().unwrap_or("");
                sink.attr(
                    CLASS_NAME,
                    AttrValue::Prefixed(PROP_SPACE_SEP, "language-", first),
                );
            }
            sink.finish_attrs();
            sink.code_info(code_data.lang, code_data.meta);
            sink.code_block_text(view.get_str(code_data.value), Pos::None);
            sink.close_element("code");
            sink.close_element("pre");
        }

        Some(MdastNodeType::Text) => {
            let value = decode_string_ref_data(view.get_type_data(node_id));
            sink.text_trimmed(value, Pos::Node(node_id));
        }

        Some(MdastNodeType::Emphasis) => emit_inline_wrapper(node_id, "em", ctx, sink, depth),
        Some(MdastNodeType::Strong) => emit_inline_wrapper(node_id, "strong", ctx, sink, depth),
        Some(MdastNodeType::Delete) => emit_inline_wrapper(node_id, "del", ctx, sink, depth),
        Some(MdastNodeType::Superscript) => emit_inline_wrapper(node_id, "sup", ctx, sink, depth),
        Some(MdastNodeType::Subscript) => emit_inline_wrapper(node_id, "sub", ctx, sink, depth),
        Some(MdastNodeType::DescriptionTerm) => {
            emit_inline_wrapper(node_id, "dt", ctx, sink, depth)
        }

        Some(MdastNodeType::InlineCode) => {
            let value_ref = decode_string_ref_data(view.get_type_data(node_id));
            sink.open_source_element("code", node_id);
            if sink.finish_source_attrs() == Children::Recurse {
                let value = view.get_str(value_ref);
                if value.contains(['\n', '\r']) {
                    let normalized = code_span_line_endings_to_spaces(value);
                    sink.text(&normalized, Pos::Node(node_id));
                } else {
                    sink.text(value, Pos::Node(node_id));
                }
            }
            sink.close_element("code");
        }

        Some(MdastNodeType::Break) => {
            sink.open_source_void("br", node_id);
            sink.finish_source_void();
            sink.newline();
        }

        Some(MdastNodeType::Link) => {
            let link_data = decode_link_data(view.get_type_data(node_id));
            let url = normalize_url(view.get_str(link_data.url));
            sink.open_source_element("a", node_id);
            sink.attr(HREF, AttrValue::text(&url));
            if link_data.title.len > 0 {
                sink.attr(TITLE, AttrValue::pooled(link_data.title));
            }
            if sink.finish_source_attrs() == Children::Recurse {
                emit_children(node_id, ctx, sink, depth);
            }
            sink.close_element("a");
        }

        Some(MdastNodeType::Image) => {
            let img_data = decode_image_data(view.get_type_data(node_id));
            let url = normalize_url(view.get_str(img_data.url));
            sink.open_source_void("img", node_id);
            sink.attr(SRC, AttrValue::text(&url));
            sink.attr(ALT, AttrValue::pooled(img_data.alt));
            if img_data.title.len > 0 {
                sink.attr(TITLE, AttrValue::pooled(img_data.title));
            }
            sink.finish_source_void();
        }

        Some(MdastNodeType::DescriptionList) => {
            sink.open_source_element("dl", node_id);
            if sink.finish_source_attrs() == Children::Recurse {
                emit_children_with_newlines(node_id, ctx, sink, depth);
            }
            sink.close_element("dl");
        }

        Some(MdastNodeType::DescriptionDetails) => {
            sink.open_source_element("dd", node_id);
            if sink.finish_source_attrs() == Children::Recurse {
                let data = view.get_type_data(node_id);
                let loose = !data.is_empty() && decode_description_details_data(data).spread;
                if loose {
                    emit_children_with_newlines(node_id, ctx, sink, depth);
                } else {
                    emit_children_unwrap_paragraphs_task(node_id, None, ctx, sink, depth);
                }
            }
            sink.close_element("dd");
        }

        Some(MdastNodeType::Table) => emit_table(node_id, ctx, sink, depth),

        Some(MdastNodeType::Math) => {
            let math_data = decode_math_data(view.get_type_data(node_id));
            sink.open_source_element("pre", node_id);
            if sink.finish_source_attrs() == Children::Replaced {
                sink.close_element("pre");
                return;
            }
            sink.open_element("code", Pos::None);
            sink.attr(
                CLASS_NAME,
                AttrValue::class_list("language-math math-display"),
            );
            sink.finish_attrs();
            sink.text(view.get_str(math_data.value), Pos::None);
            sink.close_element("code");
            sink.close_element("pre");
        }

        Some(MdastNodeType::InlineMath) => {
            let math_data = decode_math_data(view.get_type_data(node_id));
            sink.open_source_element("code", node_id);
            sink.attr(
                CLASS_NAME,
                AttrValue::class_list("language-math math-inline"),
            );
            if sink.finish_source_attrs() == Children::Recurse {
                sink.text(view.get_str(math_data.value), Pos::None);
            }
            sink.close_element("code");
        }

        Some(MdastNodeType::Definition | MdastNodeType::Yaml | MdastNodeType::Toml) => {}

        Some(MdastNodeType::LinkReference) => {
            let data = view.get_type_data(node_id);
            if data.len() >= 20 {
                let identifier = view.get_str(decode_reference_data(data).identifier);
                match ctx.refs.defs.get(identifier) {
                    Some(def) => {
                        let url = normalize_url(view.get_str(def.url));
                        sink.open_source_element("a", node_id);
                        sink.attr(HREF, AttrValue::text(&url));
                        if !def.title.is_empty() {
                            sink.attr(TITLE, AttrValue::pooled(def.title));
                        }
                        if sink.finish_source_attrs() == Children::Recurse {
                            emit_children(node_id, ctx, sink, depth);
                        }
                        sink.close_element("a");
                    }
                    None => emit_children(node_id, ctx, sink, depth),
                }
            }
        }

        Some(MdastNodeType::ImageReference) => {
            let data = view.get_type_data(node_id);
            if data.len() >= 20 {
                let identifier = view.get_str(decode_reference_data(data).identifier);
                if let Some(def) = ctx.refs.defs.get(identifier) {
                    let url = normalize_url(view.get_str(def.url));
                    // Parser-emitted ImageReferences store alt inline; plugin-synthesized ones use children.
                    let stored_alt = decode_image_reference_alt(data);
                    let alt = if stored_alt.is_empty() {
                        std::borrow::Cow::Owned(extract_text_content(node_id, view))
                    } else {
                        std::borrow::Cow::Borrowed(view.get_str(stored_alt))
                    };
                    sink.open_source_void("img", node_id);
                    sink.attr(SRC, AttrValue::text(&url));
                    sink.attr(ALT, AttrValue::text(&alt));
                    if !def.title.is_empty() {
                        sink.attr(TITLE, AttrValue::pooled(def.title));
                    }
                    sink.finish_source_void();
                }
            }
        }

        Some(MdastNodeType::FootnoteReference) => emit_footnote_reference(node_id, ctx, sink),

        Some(MdastNodeType::FootnoteDefinition) => {}

        #[cfg(feature = "mdx")]
        Some(node_type @ (MdastNodeType::MdxJsxFlowElement | MdastNodeType::MdxJsxTextElement)) => {
            if sink.open_mdx_jsx_element(node_id, node_type) {
                emit_children(node_id, ctx, sink, depth);
                sink.close_dynamic_element();
            }
        }

        #[cfg(feature = "mdx")]
        Some(
            node_type @ (MdastNodeType::MdxFlowExpression
            | MdastNodeType::MdxTextExpression
            | MdastNodeType::MdxjsEsm),
        ) => sink.mdx_leaf(node_id, node_type),

        #[cfg(not(feature = "mdx"))]
        Some(
            MdastNodeType::MdxJsxFlowElement
            | MdastNodeType::MdxJsxTextElement
            | MdastNodeType::MdxFlowExpression
            | MdastNodeType::MdxTextExpression
            | MdastNodeType::MdxjsEsm,
        ) => emit_children(node_id, ctx, sink, depth),

        Some(
            MdastNodeType::ContainerDirective
            | MdastNodeType::LeafDirective
            | MdastNodeType::TextDirective,
        ) => {
            if let Some(action) = sink.open_directive(node_id) {
                if action == Children::Recurse {
                    emit_children(node_id, ctx, sink, depth);
                }
                sink.close_dynamic_element();
            }
        }

        Some(MdastNodeType::Custom) => {
            let value = decode_custom_data(view.get_type_data(node_id)).value;
            let has_children = !view.get_children(node_id).is_empty();
            // Everything but a bare `value` leaf stays an element so children are never dropped.
            if value.len > 0 && !has_children && sink.has_no_h_data(node_id) {
                sink.text_pooled(value, Pos::Node(node_id));
            } else {
                emit_inline_wrapper(node_id, "div", ctx, sink, depth);
            }
        }

        Some(MdastNodeType::TableRow | MdastNodeType::TableCell) | None => {
            emit_children(node_id, ctx, sink, depth)
        }
    }
}

fn emit_inline_wrapper<S: ConvertSink>(
    node_id: u32,
    tag: &'static str,
    ctx: &EmitCtx<'_, '_>,
    sink: &mut S,
    depth: u32,
) {
    sink.open_source_element(tag, node_id);
    if sink.finish_source_attrs() == Children::Recurse {
        emit_children(node_id, ctx, sink, depth);
    }
    sink.close_element(tag);
}

fn emit_children<S: ConvertSink>(node_id: u32, ctx: &EmitCtx<'_, '_>, sink: &mut S, depth: u32) {
    let view = ctx.view;
    let break_ty = MdastNodeType::Break as u8;
    let mut prev_was_break = false;
    for &child_id in view.get_children(node_id) {
        let mark = sink.mark_break_boundary(prev_was_break);
        emit_node(child_id, ctx, sink, depth + 1);
        sink.apply_break_trim(prev_was_break, mark);
        prev_was_break = view.get_node(child_id).node_type == break_ty;
    }
}

fn emit_children_with_newlines<S: ConvertSink>(
    node_id: u32,
    ctx: &EmitCtx<'_, '_>,
    sink: &mut S,
    depth: u32,
) {
    let view = ctx.view;
    let children = view.get_children(node_id);
    sink.newline();
    if children.is_empty() {
        return;
    }
    for &child_id in children {
        if !sink.produces_output(child_id) {
            continue;
        }
        emit_node(child_id, ctx, sink, depth + 1);
        sink.newline();
    }
}

/// Looseness is a property of the list, so resolving it per item would be quadratic.
fn emit_list_items<S: ConvertSink>(list_id: u32, ctx: &EmitCtx<'_, '_>, sink: &mut S, depth: u32) {
    let view = ctx.view;
    let items_are_loose = list_is_loose(list_id, view);
    sink.newline();
    for &child_id in view.get_children(list_id) {
        if !sink.produces_output(child_id) {
            continue;
        }
        if MdastNodeType::from_u8(view.get_node(child_id).node_type)
            == Some(MdastNodeType::ListItem)
        {
            crate::stack::with_headroom(depth + 1, || {
                emit_list_item(child_id, items_are_loose, ctx, sink, depth + 1)
            });
        } else {
            emit_node(child_id, ctx, sink, depth + 1);
        }
        sink.newline();
    }
}

fn emit_list_item<S: ConvertSink>(
    node_id: u32,
    list_is_loose: bool,
    ctx: &EmitCtx<'_, '_>,
    sink: &mut S,
    depth: u32,
) {
    let task = list_item_data_of(node_id, ctx.view).filter(|d| d.checked != 2);
    sink.open_source_element("li", node_id);
    if task.is_some() {
        sink.attr(CLASS_NAME, AttrValue::class_list("task-list-item"));
    }
    if sink.finish_source_attrs() == Children::Recurse {
        if list_is_loose {
            emit_children_with_newlines_task(node_id, task, ctx, sink, depth);
        } else {
            emit_children_unwrap_paragraphs_task(node_id, task, ctx, sink, depth);
        }
    }
    sink.close_element("li");
}

fn emit_children_wrapped<S: ConvertSink>(
    node_id: u32,
    ctx: &EmitCtx<'_, '_>,
    sink: &mut S,
    depth: u32,
) {
    let view = ctx.view;
    let mut has_output = false;
    for &child_id in view.get_children(node_id) {
        if sink.produces_output(child_id) {
            if has_output {
                sink.newline();
            }
            has_output = true;
        }
        emit_node(child_id, ctx, sink, depth + 1);
    }
}

fn emit_checkbox<S: ConvertSink>(item_data: ListItemData, sink: &mut S) {
    sink.open_void("input", Pos::None);
    sink.attr(TYPE, AttrValue::text("checkbox"));
    sink.attr(CHECKED, AttrValue::Flag(item_data.checked == 1));
    sink.attr(DISABLED, AttrValue::Flag(true));
    sink.finish_void();
    sink.text(" ", Pos::None);
}

fn emit_children_with_newlines_task<S: ConvertSink>(
    node_id: u32,
    task: Option<ListItemData>,
    ctx: &EmitCtx<'_, '_>,
    sink: &mut S,
    depth: u32,
) {
    let view = ctx.view;
    let children = view.get_children(node_id);
    if children.is_empty() {
        if let Some(td) = task {
            sink.newline();
            open_plain(sink, "p", Pos::Node(node_id));
            emit_checkbox(td, sink);
            sink.close_element("p");
            sink.newline();
        }
        return;
    }
    sink.newline();
    let mut first = true;
    for &child_id in children {
        let is_para = MdastNodeType::from_u8(view.get_node(child_id).node_type)
            == Some(MdastNodeType::Paragraph);
        match (first, is_para, task) {
            (true, true, Some(td)) => {
                open_plain(sink, "p", Pos::Node(child_id));
                emit_checkbox(td, sink);
                emit_children(child_id, ctx, sink, depth + 1);
                sink.close_element("p");
            }
            (true, false, Some(td)) => {
                emit_checkbox(td, sink);
                emit_node(child_id, ctx, sink, depth + 1);
            }
            _ => emit_node(child_id, ctx, sink, depth + 1),
        }
        // Children that render to nothing must not leave a stray separator.
        if sink.produces_output(child_id) {
            sink.newline();
        }
        first = false;
    }
}

fn emit_children_unwrap_paragraphs_task<S: ConvertSink>(
    node_id: u32,
    task: Option<ListItemData>,
    ctx: &EmitCtx<'_, '_>,
    sink: &mut S,
    depth: u32,
) {
    let view = ctx.view;
    let mut first = true;
    let mut prev_was_block = false;
    for &child_id in view.get_children(node_id) {
        if !sink.produces_output(child_id) {
            continue;
        }
        let is_para = MdastNodeType::from_u8(view.get_node(child_id).node_type)
            == Some(MdastNodeType::Paragraph);
        if is_para {
            if let (true, Some(td)) = (first, task) {
                emit_checkbox(td, sink);
            }
            emit_children(child_id, ctx, sink, depth + 1);
            prev_was_block = false;
        } else {
            if !prev_was_block {
                sink.newline();
            }
            if let (true, Some(td)) = (first, task) {
                emit_checkbox(td, sink);
            }
            emit_node(child_id, ctx, sink, depth + 1);
            sink.newline();
            prev_was_block = true;
        }
        first = false;
    }
}

fn emit_table<S: ConvertSink>(node_id: u32, ctx: &EmitCtx<'_, '_>, sink: &mut S, depth: u32) {
    let view = ctx.view;
    let alignments = decode_table_alignments(view.get_type_data(node_id));
    sink.open_source_element("table", node_id);
    if sink.finish_source_attrs() == Children::Replaced {
        sink.close_element("table");
        return;
    }
    let child_ids = view.get_children(node_id);
    if !child_ids.is_empty() {
        sink.newline();
        open_plain(sink, "thead", Pos::Node(child_ids[0]));
        sink.newline();
        emit_table_row(child_ids[0], true, &alignments, ctx, sink, depth);
        sink.newline();
        sink.close_element("thead");
        sink.newline();

        if let Some((&last_body, _)) = child_ids[1..].split_last() {
            open_plain(sink, "tbody", Pos::Span(child_ids[1], last_body));
            sink.newline();
            for &row_id in &child_ids[1..] {
                emit_table_row(row_id, false, &alignments, ctx, sink, depth);
                sink.newline();
            }
            sink.close_element("tbody");
            sink.newline();
        }
    }
    sink.close_element("table");
}

fn emit_table_row<S: ConvertSink>(
    row_id: u32,
    is_header: bool,
    alignments: &[ColumnAlign],
    ctx: &EmitCtx<'_, '_>,
    sink: &mut S,
    depth: u32,
) {
    let view = ctx.view;
    open_plain(sink, "tr", Pos::Node(row_id));
    sink.newline();
    let all_cells = view.get_children(row_id);
    // mdast-util-to-hast truncates source cells past the header column count and pads underflow.
    let column_count = if alignments.is_empty() {
        all_cells.len()
    } else {
        alignments.len()
    };
    let max_cells = all_cells.len().min(column_count);
    let cell_ids = &all_cells[..max_cells];
    let cell_tag = if is_header { "th" } else { "td" };
    for (col_idx, &cell_id) in cell_ids.iter().enumerate() {
        open_cell(
            sink,
            cell_tag,
            alignments.get(col_idx).copied(),
            Pos::Node(cell_id),
        );
        emit_children(cell_id, ctx, sink, depth);
        sink.close_element(cell_tag);
        sink.newline();
    }
    for col_idx in cell_ids.len()..column_count {
        open_cell(sink, cell_tag, alignments.get(col_idx).copied(), Pos::None);
        sink.close_element(cell_tag);
        sink.newline();
    }
    sink.close_element("tr");
}

fn open_cell<S: ConvertSink>(
    sink: &mut S,
    cell_tag: &'static str,
    align: Option<ColumnAlign>,
    pos: Pos,
) {
    sink.open_element(cell_tag, pos);
    let style = match align.unwrap_or(ColumnAlign::None) {
        ColumnAlign::None => None,
        ColumnAlign::Left => Some("text-align: left"),
        ColumnAlign::Right => Some("text-align: right"),
        ColumnAlign::Center => Some("text-align: center"),
    };
    if let Some(style) = style {
        sink.attr(STYLE, AttrValue::text(style));
    }
    sink.finish_attrs();
}

fn emit_footnote_reference<S: ConvertSink>(node_id: u32, ctx: &EmitCtx<'_, '_>, sink: &mut S) {
    let view = ctx.view;
    let data = view.get_type_data(node_id);
    if data.len() < 20 {
        return;
    }
    let identifier = view.get_str(decode_reference_data(data).identifier);
    let Some(number) = ctx
        .refs
        .footnotes
        .as_ref()
        .and_then(|m| m.get(identifier).copied())
    else {
        // Orphan reference: remark keeps these as literal `[^id]` text.
        sink.text(&format!("[^{}]", identifier), Pos::None);
        return;
    };
    let safe_id = footnote_fragment_id(identifier);
    let occurrence = ctx
        .refs
        .footnote_ref_occurrence
        .get(&node_id)
        .copied()
        .unwrap_or(1);
    open_plain(sink, "sup", Pos::Node(node_id));
    sink.open_element("a", Pos::Node(node_id));
    let href = format!("#{}fn-{}", ctx.options.clobber_prefix, safe_id);
    sink.attr(HREF, AttrValue::text(&href));
    let id_attr = if occurrence > 1 {
        format!(
            "{}fnref-{}-{}",
            ctx.options.clobber_prefix, safe_id, occurrence
        )
    } else {
        format!("{}fnref-{}", ctx.options.clobber_prefix, safe_id)
    };
    sink.attr(ID, AttrValue::text(&id_attr));
    sink.attr(DATA_FOOTNOTE_REF, AttrValue::Flag(true));
    sink.attr(ARIA_DESCRIBED_BY, AttrValue::class_list("footnote-label"));
    sink.finish_attrs();
    sink.text(&number.to_string(), Pos::None);
    sink.close_element("a");
    sink.close_element("sup");
}

fn emit_footnotes_section<S: ConvertSink>(ctx: &EmitCtx<'_, '_>, sink: &mut S, depth: u32) {
    if ctx.refs.footnote_defs.is_empty() {
        return;
    }
    let view = ctx.view;
    sink.newline();
    sink.open_element("section", Pos::None);
    sink.attr(DATA_FOOTNOTES, AttrValue::Flag(true));
    sink.attr(CLASS_NAME, AttrValue::class_list("footnotes"));
    sink.finish_attrs();
    sink.open_element("h2", Pos::None);
    sink.attr(CLASS_NAME, AttrValue::class_list("sr-only"));
    sink.attr(ID, AttrValue::text("footnote-label"));
    sink.finish_attrs();
    sink.text(&ctx.options.footnote_label, Pos::None);
    sink.close_element("h2");
    sink.newline();
    open_plain(sink, "ol", Pos::None);
    sink.newline();

    for &def_id in &ctx.refs.footnote_defs {
        let def_data = view.get_type_data(def_id);
        if def_data.len() < 16 {
            continue;
        }
        let fd = decode_footnote_definition_data(def_data);
        let identifier = view.get_str(fd.identifier);
        let number = ctx
            .refs
            .footnotes
            .as_ref()
            .and_then(|m| m.get(identifier).copied())
            .expect("footnote identifier missing from collected numbers");
        let safe_id = footnote_fragment_id(identifier);
        sink.open_element("li", Pos::Node(def_id));
        let id_attr = format!("{}fn-{}", ctx.options.clobber_prefix, safe_id);
        sink.attr(ID, AttrValue::text(&id_attr));
        sink.finish_attrs();
        sink.newline();

        let children = view.get_children(def_id);
        let ends_in_paragraph = children
            .last()
            .is_some_and(|&cid| view.get_node(cid).node_type == MdastNodeType::Paragraph as u8);
        let total_refs = ctx
            .refs
            .footnote_ref_totals
            .get(identifier)
            .copied()
            .unwrap_or(1)
            .max(1);

        if children.is_empty() {
            emit_footnote_backrefs(&safe_id, number, total_refs, true, ctx, sink);
        } else {
            for (i, &child_id) in children.iter().enumerate() {
                if i > 0 {
                    sink.newline();
                }
                if ends_in_paragraph && i + 1 == children.len() {
                    emit_paragraph_with_backrefs(
                        child_id,
                        &safe_id,
                        number,
                        total_refs,
                        ctx,
                        sink,
                        depth + 1,
                    );
                } else {
                    emit_node(child_id, ctx, sink, depth + 1);
                }
            }
            if !ends_in_paragraph {
                sink.newline();
                emit_footnote_backrefs(&safe_id, number, total_refs, true, ctx, sink);
            }
        }

        sink.newline();
        sink.close_element("li");
        sink.newline();
    }

    sink.close_element("ol");
    sink.newline();
    sink.close_element("section");
}

/// remark-gfm merges the separator space into the last text node, so that value skips `trim-lines`.
fn emit_paragraph_with_backrefs<S: ConvertSink>(
    para_id: u32,
    identifier: &str,
    number: usize,
    total_refs: usize,
    ctx: &EmitCtx<'_, '_>,
    sink: &mut S,
    depth: u32,
) {
    let view = ctx.view;
    open_plain(sink, "p", Pos::Node(para_id));
    if let Some((&last_id, prefix)) = view.get_children(para_id).split_last() {
        for &cid in prefix {
            emit_node(cid, ctx, sink, depth + 1);
        }
        let data = view.get_type_data(last_id);
        let last_is_text = view.get_node(last_id).node_type == MdastNodeType::Text as u8;
        if last_is_text && data.len() >= 8 {
            sink.text_with_trailing_space(StringRef::from_bytes(data), Pos::Node(last_id));
        } else {
            emit_node(last_id, ctx, sink, depth + 1);
            sink.text(" ", Pos::None);
        }
    }
    emit_footnote_backrefs(identifier, number, total_refs, false, ctx, sink);
    sink.close_element("p");
}

/// A [`Backref`] pre-split on `{reference}`, so the per-reference loop never re-scans it.
enum BackrefTemplate<'a> {
    Literal(&'a str),
    Split(&'a str, &'a str),
    Callback(&'a dyn Fn(usize, usize) -> String),
}

impl<'a> BackrefTemplate<'a> {
    fn new(backref: &'a Backref) -> Self {
        match backref {
            Backref::Template(tpl) => match tpl.split_once("{reference}") {
                Some((before, after)) => BackrefTemplate::Split(before, after),
                None => BackrefTemplate::Literal(tpl),
            },
            Backref::Callback(cb) => BackrefTemplate::Callback(cb),
        }
    }

    fn is_template(&self) -> bool {
        !matches!(self, BackrefTemplate::Callback(_))
    }

    fn write(&self, out: &mut String, number: usize, k: usize) {
        out.clear();
        match self {
            BackrefTemplate::Literal(text) => out.push_str(text),
            BackrefTemplate::Split(before, after) => {
                out.push_str(before);
                write_reference_token(out, number, k);
                out.push_str(after);
            }
            BackrefTemplate::Callback(cb) => out.push_str(&cb(number, k)),
        }
    }
}

fn write_reference_token(out: &mut String, number: usize, k: usize) {
    let _ = write!(out, "{number}");
    if k > 1 {
        let _ = write!(out, "-{k}");
    }
}

/// remark's loose wrap newlines the separator when the backrefs are `<li>` children.
fn emit_footnote_backrefs<S: ConvertSink>(
    identifier: &str,
    number: usize,
    total_refs: usize,
    li_children: bool,
    ctx: &EmitCtx<'_, '_>,
    sink: &mut S,
) {
    let label = BackrefTemplate::new(&ctx.options.footnote_back_label);
    let content = BackrefTemplate::new(&ctx.options.footnote_back_content);
    let content_is_template = content.is_template();
    let mut aria_buf = String::new();
    let mut content_buf = String::new();
    let mut tail_buf = String::new();

    for k in 1..=total_refs.max(1) {
        if k > 1 {
            if li_children {
                sink.newline();
            }
            sink.text(" ", Pos::None);
            if li_children {
                sink.newline();
            }
        }
        label.write(&mut aria_buf, number, k);
        content.write(&mut content_buf, number, k);
        sink.open_element("a", Pos::None);
        tail_buf.clear();
        tail_buf.push('#');
        tail_buf.push_str(&ctx.options.clobber_prefix);
        tail_buf.push_str("fnref-");
        tail_buf.push_str(identifier);
        if k > 1 {
            let _ = write!(tail_buf, "-{k}");
        }
        sink.attr(HREF, AttrValue::text(&tail_buf));
        sink.attr(DATA_FOOTNOTE_BACKREF, AttrValue::text(""));
        sink.attr(ARIA_LABEL, AttrValue::text(&aria_buf));
        sink.attr(CLASS_NAME, AttrValue::class_list("data-footnote-backref"));
        sink.finish_attrs();
        sink.text(&content_buf, Pos::None);
        // Template mode auto-appends <sup>K</sup>; callbacks emit their own.
        if k > 1 && content_is_template {
            open_plain(sink, "sup", Pos::None);
            tail_buf.clear();
            let _ = write!(tail_buf, "{k}");
            sink.text(&tail_buf, Pos::None);
            sink.close_element("sup");
        }
        sink.close_element("a");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use satteri_property_info::property_to_attribute;

    /// The streaming sink writes attribute names the arena sink derives from property names.
    #[test]
    fn attribute_names_match_property_info() {
        let names = [
            ALT,
            ARIA_DESCRIBED_BY,
            ARIA_LABEL,
            CHECKED,
            CLASS_NAME,
            DATA_FOOTNOTES,
            DATA_FOOTNOTE_BACKREF,
            DATA_FOOTNOTE_REF,
            DISABLED,
            HREF,
            ID,
            SRC,
            START,
            STYLE,
            TITLE,
            TYPE,
        ];
        for name in names {
            assert_eq!(
                property_to_attribute(name.property, false),
                name.attribute,
                "property {}",
                name.property
            );
        }
    }
}
