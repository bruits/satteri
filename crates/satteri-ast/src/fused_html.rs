//! Emit HTML straight from an MDAST arena, with no intermediate HAST arena.
//!
//! Output is byte-identical to `mdast_arena_to_hast_arena` + `hast_arena_to_html`.

use satteri_arena::{Arena, Mdast, StringRef, decode_string_ref_data};

use crate::convert::{
    Backref, CollectedRefs, ConvertOptions, code_span_line_endings_to_spaces, collect_refs,
    contains_h_key, extract_text_content, list_contains_task_item, normalize_url, resolve_backref,
    trim_lines_for_hast,
};
use crate::hast::escape::{escape_html_attr_value, escape_html_body_text};
use crate::mdast::{
    ColumnAlign, ListItemData, MdastNodeType, decode_code_data, decode_custom_data,
    decode_description_details_data, decode_footnote_definition_data, decode_heading_data,
    decode_image_data, decode_image_reference_alt, decode_link_data, decode_list_data,
    decode_list_item_data, decode_math_data, decode_reference_data, decode_table_alignments,
};

/// Render `view` to HTML without an intermediate arena, or `None` when the
/// document or options need the two-stage pipeline.
pub(crate) fn mdast_to_html_fused(view: &Arena<Mdast>, options: &ConvertOptions) -> Option<String> {
    if view.is_empty() || view.node_data.values().any(|blob| contains_h_key(blob)) {
        return None;
    }
    #[cfg(feature = "from-html")]
    if options.raw_html {
        return None;
    }
    let refs = collect_refs(view);
    let mut emitter = Emitter {
        out: String::with_capacity(view.string_pool().len()),
        view,
        refs: &refs,
        options,
        trim: Trim::None,
    };
    emitter.emit_node(0, 0);
    let mut out = emitter.out;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    Some(out)
}

/// After a `Break`, only the next sibling's own text or its element's first text child is trimmed.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Trim {
    None,
    Node,
    FirstChild,
}

struct Emitter<'a, 'src> {
    out: String,
    view: &'a Arena<Mdast>,
    refs: &'a CollectedRefs<'src>,
    options: &'a ConvertOptions,
    trim: Trim,
}

impl Emitter<'_, '_> {
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

    fn push_newline(&mut self) {
        self.trim = Trim::None;
        self.out.push('\n');
    }

    fn push_raw(&mut self, html: &str) {
        self.trim = Trim::None;
        self.out.push_str(html);
    }

    fn begin_element(&mut self, tag: &str) {
        self.trim = if self.trim == Trim::Node {
            Trim::FirstChild
        } else {
            Trim::None
        };
        self.out.push('<');
        self.out.push_str(tag);
    }

    fn begin_void_element(&mut self, tag: &str) {
        self.trim = Trim::None;
        self.out.push('<');
        self.out.push_str(tag);
    }

    fn attr(&mut self, name: &str, value: &str) {
        self.out.push(' ');
        self.out.push_str(name);
        self.out.push_str("=\"");
        escape_html_attr_value(&mut self.out, value);
        self.out.push('"');
    }

    fn attr_prefixed(&mut self, name: &str, prefix: &str, value: &str) {
        self.out.push(' ');
        self.out.push_str(name);
        self.out.push_str("=\"");
        self.out.push_str(prefix);
        escape_html_attr_value(&mut self.out, value);
        self.out.push('"');
    }

    fn attr_bool(&mut self, name: &str) {
        self.out.push(' ');
        self.out.push_str(name);
    }

    fn finish_open(&mut self) {
        self.out.push('>');
    }

    fn close_element(&mut self, tag: &str) {
        if self.trim == Trim::FirstChild {
            self.trim = Trim::None;
        }
        self.out.push_str("</");
        self.out.push_str(tag);
        self.out.push('>');
    }

    fn wrap(&mut self, node_id: u32, tag: &str, depth: u32) {
        self.begin_element(tag);
        self.finish_open();
        self.emit_children(node_id, depth);
        self.close_element(tag);
    }

    fn emit_node(&mut self, node_id: u32, depth: u32) {
        crate::stack::with_headroom(depth, || self.emit_node_at(node_id, depth));
    }

    fn emit_node_at(&mut self, node_id: u32, depth: u32) {
        let view = self.view;
        match MdastNodeType::from_u8(view.get_node(node_id).node_type) {
            Some(MdastNodeType::Root) => {
                self.emit_children_wrapped(node_id, depth);
                self.emit_footnotes_section(depth);
            }

            Some(MdastNodeType::Paragraph) => self.wrap(node_id, "p", depth),

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
                self.wrap(node_id, tag, depth);
            }

            Some(MdastNodeType::ThematicBreak) => {
                self.begin_void_element("hr");
                self.finish_open();
            }

            Some(MdastNodeType::Blockquote) => {
                self.begin_element("blockquote");
                self.finish_open();
                self.emit_children_with_newlines(node_id, depth);
                self.close_element("blockquote");
            }

            Some(MdastNodeType::List) => {
                let list_data = decode_list_data(view.get_type_data(node_id));
                let tag = if list_data.ordered { "ol" } else { "ul" };
                let has_task_items = list_contains_task_item(node_id, view);
                self.begin_element(tag);
                if list_data.ordered && list_data.start != 1 {
                    self.attr("start", &list_data.start.to_string());
                }
                if has_task_items {
                    self.attr("class", "contains-task-list");
                }
                self.finish_open();
                self.emit_children_with_newlines(node_id, depth);
                self.close_element(tag);
            }

            Some(MdastNodeType::ListItem) => {
                let data = view.get_type_data(node_id);
                let item_data = if data.is_empty() {
                    None
                } else {
                    Some(decode_list_item_data(data))
                };
                let task = item_data.filter(|d| d.checked != 2);
                self.begin_element("li");
                if task.is_some() {
                    self.attr("class", "task-list-item");
                }
                self.finish_open();

                let parent_id = view.get_node(node_id).parent;
                let parent_data = view.get_type_data(parent_id);
                let loose = (!parent_data.is_empty() && decode_list_data(parent_data).spread)
                    || view.get_children(parent_id).iter().any(|&sibling_id| {
                        let sd = view.get_type_data(sibling_id);
                        !sd.is_empty() && decode_list_item_data(sd).spread
                    });
                if loose {
                    self.emit_children_with_newlines_task(node_id, task, depth);
                } else {
                    self.emit_children_unwrap_paragraphs_task(node_id, task, depth);
                }
                self.close_element("li");
            }

            Some(MdastNodeType::Html) => {
                let value = view.get_str(decode_string_ref_data(view.get_type_data(node_id)));
                self.push_raw(value);
            }

            Some(MdastNodeType::Code) => {
                let code_data = decode_code_data(view.get_type_data(node_id));
                let value = view.get_str(code_data.value);
                self.begin_element("pre");
                self.finish_open();
                self.begin_element("code");
                if code_data.lang.len > 0 {
                    let lang = view.get_str(code_data.lang);
                    // A character reference can decode to whitespace, so only the first word names the language.
                    let first = lang.split(char::is_whitespace).next().unwrap_or("");
                    self.attr_prefixed("class", "language-", first);
                }
                self.finish_open();
                self.push_text(value);
                if !value.is_empty() {
                    self.out.push('\n');
                }
                self.close_element("code");
                self.close_element("pre");
            }

            Some(MdastNodeType::Text) => {
                let value = view.get_str(decode_string_ref_data(view.get_type_data(node_id)));
                self.push_text(&trim_lines_for_hast(value));
            }

            Some(MdastNodeType::Emphasis) => self.wrap(node_id, "em", depth),
            Some(MdastNodeType::Strong) => self.wrap(node_id, "strong", depth),
            Some(MdastNodeType::Delete) => self.wrap(node_id, "del", depth),
            Some(MdastNodeType::Superscript) => self.wrap(node_id, "sup", depth),
            Some(MdastNodeType::Subscript) => self.wrap(node_id, "sub", depth),
            Some(MdastNodeType::DescriptionTerm) => self.wrap(node_id, "dt", depth),

            Some(MdastNodeType::InlineCode) => {
                let value = view.get_str(decode_string_ref_data(view.get_type_data(node_id)));
                self.begin_element("code");
                self.finish_open();
                if value.contains(['\n', '\r']) {
                    self.push_text(&code_span_line_endings_to_spaces(value));
                } else {
                    self.push_text(value);
                }
                self.close_element("code");
            }

            Some(MdastNodeType::Break) => {
                self.begin_void_element("br");
                self.finish_open();
                self.out.push('\n');
            }

            Some(MdastNodeType::Link) => {
                let link_data = decode_link_data(view.get_type_data(node_id));
                let url = normalize_url(view.get_str(link_data.url));
                self.begin_element("a");
                self.attr("href", &url);
                if link_data.title.len > 0 {
                    self.attr("title", view.get_str(link_data.title));
                }
                self.finish_open();
                self.emit_children(node_id, depth);
                self.close_element("a");
            }

            Some(MdastNodeType::Image) => {
                let img_data = decode_image_data(view.get_type_data(node_id));
                let url = normalize_url(view.get_str(img_data.url));
                self.begin_void_element("img");
                self.attr("src", &url);
                self.attr("alt", view.get_str(img_data.alt));
                if img_data.title.len > 0 {
                    self.attr("title", view.get_str(img_data.title));
                }
                self.finish_open();
            }

            Some(MdastNodeType::DescriptionList) => {
                self.begin_element("dl");
                self.finish_open();
                self.emit_children_with_newlines(node_id, depth);
                self.close_element("dl");
            }

            Some(MdastNodeType::DescriptionDetails) => {
                self.begin_element("dd");
                self.finish_open();
                let data = view.get_type_data(node_id);
                let loose = !data.is_empty() && decode_description_details_data(data).spread;
                if loose {
                    self.emit_children_with_newlines(node_id, depth);
                } else {
                    self.emit_children_unwrap_paragraphs_task(node_id, None, depth);
                }
                self.close_element("dd");
            }

            Some(MdastNodeType::Table) => self.emit_table(node_id, depth),

            Some(MdastNodeType::Math) => {
                let math_data = decode_math_data(view.get_type_data(node_id));
                let value = view.get_str(math_data.value);
                self.begin_element("pre");
                self.finish_open();
                self.begin_element("code");
                self.attr("class", "language-math math-display");
                self.finish_open();
                self.push_text(value);
                self.close_element("code");
                self.close_element("pre");
            }

            Some(MdastNodeType::InlineMath) => {
                let math_data = decode_math_data(view.get_type_data(node_id));
                let value = view.get_str(math_data.value);
                self.begin_element("code");
                self.attr("class", "language-math math-inline");
                self.finish_open();
                self.push_text(value);
                self.close_element("code");
            }

            Some(MdastNodeType::Definition | MdastNodeType::Yaml | MdastNodeType::Toml) => {}

            Some(MdastNodeType::LinkReference) => {
                let data = view.get_type_data(node_id);
                if data.len() >= 20 {
                    let identifier = view.get_str(decode_reference_data(data).identifier);
                    match self.refs.defs.get(identifier) {
                        Some(def) => {
                            let url = normalize_url(view.get_str(def.url));
                            let title = def.title;
                            self.begin_element("a");
                            self.attr("href", &url);
                            if !title.is_empty() {
                                self.attr("title", view.get_str(title));
                            }
                            self.finish_open();
                            self.emit_children(node_id, depth);
                            self.close_element("a");
                        }
                        None => self.emit_children(node_id, depth),
                    }
                }
            }

            Some(MdastNodeType::ImageReference) => {
                let data = view.get_type_data(node_id);
                if data.len() >= 20 {
                    let identifier = view.get_str(decode_reference_data(data).identifier);
                    if let Some(def) = self.refs.defs.get(identifier) {
                        let url = normalize_url(view.get_str(def.url));
                        let title = def.title;
                        // Parser-emitted ImageReferences store alt inline; plugin-synthesized ones use children.
                        let stored_alt = decode_image_reference_alt(data);
                        let alt = if stored_alt.is_empty() {
                            std::borrow::Cow::Owned(extract_text_content(node_id, view))
                        } else {
                            std::borrow::Cow::Borrowed(view.get_str(stored_alt))
                        };
                        self.begin_void_element("img");
                        self.attr("src", &url);
                        self.attr("alt", &alt);
                        if !title.is_empty() {
                            self.attr("title", view.get_str(title));
                        }
                        self.finish_open();
                    }
                }
            }

            Some(MdastNodeType::FootnoteReference) => {
                self.emit_footnote_reference(node_id);
            }

            Some(MdastNodeType::FootnoteDefinition) => {}

            #[cfg(feature = "mdx")]
            Some(
                MdastNodeType::MdxJsxFlowElement
                | MdastNodeType::MdxJsxTextElement
                | MdastNodeType::MdxFlowExpression
                | MdastNodeType::MdxTextExpression
                | MdastNodeType::MdxjsEsm,
            ) => {}

            Some(
                MdastNodeType::ContainerDirective
                | MdastNodeType::LeafDirective
                | MdastNodeType::TextDirective,
            ) => {}

            Some(MdastNodeType::Custom) => {
                let value = decode_custom_data(view.get_type_data(node_id)).value;
                let has_children = !view.get_children(node_id).is_empty();
                if value.len > 0 && !has_children {
                    self.push_text(view.get_str(value));
                } else {
                    self.wrap(node_id, "div", depth);
                }
            }

            _ => self.emit_children(node_id, depth),
        }
    }

    fn emit_footnote_reference(&mut self, node_id: u32) {
        let view = self.view;
        let data = view.get_type_data(node_id);
        if data.len() < 20 {
            return;
        }
        let identifier = view.get_str(decode_reference_data(data).identifier);
        let Some(number) = self
            .refs
            .footnotes
            .as_ref()
            .and_then(|m| m.get(identifier).copied())
        else {
            // Orphan reference: remark keeps these as literal `[^id]` text.
            let literal = format!("[^{}]", identifier);
            self.push_text(&literal);
            return;
        };
        // remark lowercases the identifier so fragment targets resist collisions across source casing.
        let safe_id = identifier.to_ascii_lowercase();
        let occurrence = self
            .refs
            .footnote_ref_occurrence
            .get(&node_id)
            .copied()
            .unwrap_or(1);
        self.begin_element("sup");
        self.finish_open();
        self.begin_element("a");
        self.attr_prefixed("href", "#user-content-fn-", &safe_id);
        if occurrence > 1 {
            self.attr(
                "id",
                &format!("user-content-fnref-{}-{}", safe_id, occurrence),
            );
        } else {
            self.attr_prefixed("id", "user-content-fnref-", &safe_id);
        }
        self.attr_bool("data-footnote-ref");
        self.attr("aria-describedby", "footnote-label");
        self.finish_open();
        self.push_text(&number.to_string());
        self.close_element("a");
        self.close_element("sup");
    }

    fn emit_table(&mut self, node_id: u32, depth: u32) {
        let view = self.view;
        let alignments = decode_table_alignments(view.get_type_data(node_id));
        self.begin_element("table");
        self.finish_open();
        let child_ids = view.get_children(node_id);
        if !child_ids.is_empty() {
            self.push_newline();
            self.begin_element("thead");
            self.finish_open();
            self.push_newline();
            self.emit_table_row(child_ids[0], true, &alignments, depth);
            self.push_newline();
            self.close_element("thead");
            self.push_newline();

            if child_ids.len() > 1 {
                self.begin_element("tbody");
                self.finish_open();
                self.push_newline();
                for &row_id in &child_ids[1..] {
                    self.emit_table_row(row_id, false, &alignments, depth);
                    self.push_newline();
                }
                self.close_element("tbody");
                self.push_newline();
            }
        }
        self.close_element("table");
    }

    fn emit_table_row(
        &mut self,
        row_id: u32,
        is_header: bool,
        alignments: &[ColumnAlign],
        depth: u32,
    ) {
        let view = self.view;
        self.begin_element("tr");
        self.finish_open();
        self.push_newline();
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
            self.begin_cell(cell_tag, alignments.get(col_idx).copied());
            self.emit_children(cell_id, depth);
            self.close_element(cell_tag);
            self.push_newline();
        }
        for col_idx in cell_ids.len()..column_count {
            self.begin_cell(cell_tag, alignments.get(col_idx).copied());
            self.close_element(cell_tag);
            self.push_newline();
        }
        self.close_element("tr");
    }

    fn begin_cell(&mut self, cell_tag: &str, align: Option<ColumnAlign>) {
        self.begin_element(cell_tag);
        let style = match align.unwrap_or(ColumnAlign::None) {
            ColumnAlign::None => None,
            ColumnAlign::Left => Some("text-align: left"),
            ColumnAlign::Right => Some("text-align: right"),
            ColumnAlign::Center => Some("text-align: center"),
        };
        if let Some(style) = style {
            self.attr("style", style);
        }
        self.finish_open();
    }

    fn emit_children(&mut self, node_id: u32, depth: u32) {
        let view = self.view;
        let break_ty = MdastNodeType::Break as u8;
        let mut prev_was_break = false;
        for &child_id in view.get_children(node_id) {
            if prev_was_break {
                self.trim = Trim::Node;
            }
            self.emit_node(child_id, depth + 1);
            if prev_was_break {
                self.trim = Trim::None;
            }
            prev_was_break = view.get_node(child_id).node_type == break_ty;
        }
    }

    fn emit_children_with_newlines(&mut self, node_id: u32, depth: u32) {
        let view = self.view;
        let children = view.get_children(node_id);
        self.push_newline();
        if children.is_empty() {
            return;
        }
        for &child_id in children {
            if !produces_output(view, child_id) {
                continue;
            }
            self.emit_node(child_id, depth + 1);
            self.push_newline();
        }
    }

    fn emit_children_wrapped(&mut self, node_id: u32, depth: u32) {
        let view = self.view;
        let mut has_output = false;
        for &child_id in view.get_children(node_id) {
            if produces_output(view, child_id) {
                if has_output {
                    self.push_newline();
                }
                has_output = true;
            }
            self.emit_node(child_id, depth + 1);
        }
    }

    fn emit_checkbox(&mut self, item_data: ListItemData) {
        self.begin_void_element("input");
        self.attr("type", "checkbox");
        if item_data.checked == 1 {
            self.attr_bool("checked");
        }
        self.attr_bool("disabled");
        self.finish_open();
        self.push_text(" ");
    }

    fn emit_children_with_newlines_task(
        &mut self,
        node_id: u32,
        task: Option<ListItemData>,
        depth: u32,
    ) {
        let view = self.view;
        let children = view.get_children(node_id);
        if children.is_empty() {
            if let Some(td) = task {
                self.push_newline();
                self.begin_element("p");
                self.finish_open();
                self.emit_checkbox(td);
                self.close_element("p");
                self.push_newline();
            }
            return;
        }
        self.push_newline();
        let mut first = true;
        for &child_id in children {
            let is_para = MdastNodeType::from_u8(view.get_node(child_id).node_type)
                == Some(MdastNodeType::Paragraph);
            match (first, is_para, task) {
                (true, true, Some(td)) => {
                    self.begin_element("p");
                    self.finish_open();
                    self.emit_checkbox(td);
                    self.emit_children(child_id, depth + 1);
                    self.close_element("p");
                }
                (true, false, Some(td)) => {
                    self.emit_checkbox(td);
                    self.emit_node(child_id, depth + 1);
                }
                _ => self.emit_node(child_id, depth + 1),
            }
            // Children that render to nothing must not leave a stray `\n`.
            if produces_output(view, child_id) {
                self.push_newline();
            }
            first = false;
        }
    }

    fn emit_children_unwrap_paragraphs_task(
        &mut self,
        node_id: u32,
        task: Option<ListItemData>,
        depth: u32,
    ) {
        let view = self.view;
        let mut first = true;
        let mut prev_was_block = false;
        for &child_id in view.get_children(node_id) {
            if !produces_output(view, child_id) {
                continue;
            }
            let is_para = MdastNodeType::from_u8(view.get_node(child_id).node_type)
                == Some(MdastNodeType::Paragraph);
            if is_para {
                if let (true, Some(td)) = (first, task) {
                    self.emit_checkbox(td);
                }
                self.emit_children(child_id, depth + 1);
                prev_was_block = false;
            } else {
                if !prev_was_block {
                    self.push_newline();
                }
                if let (true, Some(td)) = (first, task) {
                    self.emit_checkbox(td);
                }
                self.emit_node(child_id, depth + 1);
                self.push_newline();
                prev_was_block = true;
            }
            first = false;
        }
    }

    fn emit_footnotes_section(&mut self, depth: u32) {
        if self.refs.footnote_defs.is_empty() {
            return;
        }
        let view = self.view;
        self.push_newline();
        self.begin_element("section");
        self.attr_bool("data-footnotes");
        self.attr("class", "footnotes");
        self.finish_open();
        self.begin_element("h2");
        self.attr("class", "sr-only");
        self.attr("id", "footnote-label");
        self.finish_open();
        let label = self.options.footnote_label.clone();
        self.push_text(&label);
        self.close_element("h2");
        self.push_newline();
        self.begin_element("ol");
        self.finish_open();
        self.push_newline();

        for &def_id in &self.refs.footnote_defs {
            let def_data = view.get_type_data(def_id);
            if def_data.len() < 16 {
                continue;
            }
            let fd = decode_footnote_definition_data(def_data);
            let identifier = view.get_str(fd.identifier);
            let number = self
                .refs
                .footnotes
                .as_ref()
                .and_then(|m| m.get(identifier).copied())
                .expect("footnote identifier missing from collected numbers");
            let safe_id = identifier.to_ascii_lowercase();
            self.begin_element("li");
            self.attr_prefixed("id", "user-content-fn-", &safe_id);
            self.finish_open();
            self.push_newline();

            let children = view.get_children(def_id);
            let last_para_idx = children
                .iter()
                .enumerate()
                .rev()
                .find(|&(_, &cid)| view.get_node(cid).node_type == MdastNodeType::Paragraph as u8)
                .map(|(i, _)| i);
            let total_refs = self
                .refs
                .footnote_ref_totals
                .get(identifier)
                .copied()
                .unwrap_or(1)
                .max(1);

            if children.is_empty() {
                self.emit_footnote_backrefs(&safe_id, number, total_refs);
            } else {
                for (i, &child_id) in children.iter().enumerate() {
                    if i > 0 {
                        self.push_newline();
                    }
                    if Some(i) == last_para_idx {
                        self.emit_paragraph_with_backrefs(
                            child_id,
                            &safe_id,
                            number,
                            total_refs,
                            depth + 1,
                        );
                    } else {
                        self.emit_node(child_id, depth + 1);
                    }
                }
                if last_para_idx.is_none() {
                    self.push_newline();
                    self.emit_footnote_backrefs(&safe_id, number, total_refs);
                }
            }

            self.push_newline();
            self.close_element("li");
            self.push_newline();
        }

        self.close_element("ol");
        self.push_newline();
        self.close_element("section");
    }

    fn emit_paragraph_with_backrefs(
        &mut self,
        para_id: u32,
        identifier: &str,
        number: usize,
        total_refs: usize,
        depth: u32,
    ) {
        let view = self.view;
        self.begin_element("p");
        self.finish_open();
        if let Some((&last_id, prefix)) = view.get_children(para_id).split_last() {
            for &cid in prefix {
                self.emit_node(cid, depth + 1);
            }
            let data = view.get_type_data(last_id);
            let last_is_text = view.get_node(last_id).node_type == MdastNodeType::Text as u8;
            if last_is_text && data.len() >= 8 {
                self.push_text(view.get_str(StringRef::from_bytes(data)));
                self.out.push(' ');
            } else {
                self.emit_node(last_id, depth + 1);
                self.push_text(" ");
            }
        }
        self.emit_footnote_backrefs(identifier, number, total_refs);
        self.close_element("p");
    }

    fn emit_footnote_backrefs(&mut self, identifier: &str, number: usize, total_refs: usize) {
        for k in 1..=total_refs.max(1) {
            if k > 1 {
                self.push_text(" ");
            }
            let aria = resolve_backref(&self.options.footnote_back_label, number, k);
            let back_content = resolve_backref(&self.options.footnote_back_content, number, k);
            self.begin_element("a");
            if k > 1 {
                self.attr("href", &format!("#user-content-fnref-{}-{}", identifier, k));
            } else {
                self.attr_prefixed("href", "#user-content-fnref-", identifier);
            }
            self.attr("data-footnote-backref", "");
            self.attr("aria-label", &aria);
            self.attr("class", "data-footnote-backref");
            self.finish_open();
            self.push_text(&back_content);
            // Template mode auto-appends <sup>K</sup>; callbacks emit their own.
            if k > 1 && matches!(self.options.footnote_back_content, Backref::Template(_)) {
                self.begin_element("sup");
                self.finish_open();
                self.push_text(&k.to_string());
                self.close_element("sup");
            }
            self.close_element("a");
        }
    }
}

/// Mirrors `convert::produces_hast_output`: without an `hName` a directive never renders.
fn produces_output(view: &Arena<Mdast>, child_id: u32) -> bool {
    !matches!(
        MdastNodeType::from_u8(view.get_node(child_id).node_type),
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

#[cfg(test)]
mod tests {
    use satteri_property_info::property_to_attribute;

    /// The fused path hardcodes what the two-stage path derives from hast property names.
    #[test]
    fn hardcoded_attribute_names_match_property_info() {
        let pairs = [
            ("start", "start"),
            ("className", "class"),
            ("href", "href"),
            ("title", "title"),
            ("src", "src"),
            ("alt", "alt"),
            ("type", "type"),
            ("checked", "checked"),
            ("disabled", "disabled"),
            ("id", "id"),
            ("style", "style"),
            ("dataFootnoteRef", "data-footnote-ref"),
            ("ariaDescribedBy", "aria-describedby"),
            ("dataFootnotes", "data-footnotes"),
            ("dataFootnoteBackref", "data-footnote-backref"),
            ("ariaLabel", "aria-label"),
        ];
        for (property, attribute) in pairs {
            assert_eq!(property_to_attribute(property, false), attribute);
        }
    }
}
