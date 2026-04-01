//! Fast markdown parser: pulldown-cmark → `tryckeri-mdast::MdastArena`.
//!
//! This crate bridges pulldown-cmark's event stream into the flat MdastArena
//! representation used by the rest of the pipeline (HAST, plugins, MDX compile).

use tryckeri_mdast::{
    encode_code_data, encode_expression_data, encode_footnote_definition_data, encode_heading_data,
    encode_image_data, encode_link_data, encode_list_data, encode_list_item_data, encode_math_data,
    encode_mdx_jsx_element_data, encode_string_ref_data, encode_table_data, ColumnAlign, LineIndex,
    MdastArena, MdastBuilder, MdastNodeType, StringRef, MDX_ATTR_BOOLEAN_PROP,
    MDX_ATTR_EXPRESSION_PROP, MDX_ATTR_LITERAL_PROP, MDX_ATTR_SPREAD,
};
use tryckeri_pulldown_cmark::{
    CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd, TextMergeWithOffset,
};

mod jsx_attr_parser;
use jsx_attr_parser::{parse_jsx_attributes_from_tag, JsxAttr};

pub use tryckeri_mdast;

/// Parse options controlling which extensions are enabled.
#[derive(Debug, Clone)]
pub struct ParseOptions {
    pulldown: Options,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            pulldown: Options::ENABLE_TABLES
                | Options::ENABLE_FOOTNOTES
                | Options::ENABLE_STRIKETHROUGH
                | Options::ENABLE_TASKLISTS
                | Options::ENABLE_MATH
                | Options::ENABLE_HEADING_ATTRIBUTES
                | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS,
        }
    }
}

impl ParseOptions {
    /// Enable MDX extensions (JSX, expressions, ESM).
    pub fn mdx() -> Self {
        Self {
            pulldown: Self::default().pulldown | Options::ENABLE_MDX,
        }
    }

    /// Get the underlying pulldown-cmark options.
    pub fn pulldown_options(&self) -> Options {
        self.pulldown
    }
}

/// Parse markdown source into a MdastArena.
///
/// Returns `(arena, mdx_errors)` where `mdx_errors` contains any MDX
/// validation errors collected during parsing (empty for non-MDX input).
pub fn parse(source: &str, opts: &ParseOptions) -> (MdastArena, Vec<(usize, String)>) {
    let line_index = LineIndex::from_source(source);
    let mut parser =
        TextMergeWithOffset::new(Parser::new_ext(source, opts.pulldown).into_offset_iter());
    let mut builder = MdastBuilder::new(source.to_string());

    // Open root node.
    builder.open_node(MdastNodeType::Root);
    let (end_line, end_col) = line_index.offset_to_line_col(source.len() as u32);
    builder.set_position_current(0, source.len() as u32, 1, 1, end_line, end_col);

    // Track HTML block and code block content accumulation.
    // These are containers in pulldown-cmark but leaves in MDAST.
    let mut html_block_buf: Option<String> = None;
    let mut code_block_buf: Option<String> = None;

    // Track image alt text accumulation (depth-counted for nested images).
    let mut image_alt_buf: Option<String> = None;
    let mut image_depth: usize = 0;

    // JSX tag pairing: when we see an opening JSX tag (not self-closing),
    // keep the arena node open and make subsequent events children.
    // The stack tracks open JSX tag names so we can match closing tags.
    let mut jsx_stack: Vec<(String, u32)> = Vec::new(); // (tag_name, start_offset)
                                                        // Count of End events to skip (from nodes we closed early during JSX pairing).
    let mut skip_end_events: usize = 0;
    let mut mdx_errors: Vec<(usize, String)> = Vec::new();

    for (event, range) in parser.by_ref() {
        let start = range.start as u32;
        let end = range.end as u32;
        let (start_line, start_col) = line_index.offset_to_line_col(start);
        let (end_line, end_col) = line_index.offset_to_line_col(end);

        // If we're inside an HTML block, accumulate content.
        if html_block_buf.is_some() {
            match &event {
                Event::Html(text) => {
                    html_block_buf.as_mut().unwrap().push_str(text);
                    continue;
                }
                Event::End(TagEnd::HtmlBlock) => {
                    let content = html_block_buf.take().unwrap();
                    let sr = builder.alloc_string(&content);
                    let id = builder.current_node_id();
                    let node = builder.arena_mut().get_node(id);
                    let orig_start = node.start_offset;
                    let orig_start_line = node.start_line;
                    let orig_start_col = node.start_column;
                    builder.set_position_current(
                        orig_start,
                        end,
                        orig_start_line,
                        orig_start_col,
                        end_line,
                        end_col,
                    );
                    builder.set_data_current(&encode_string_ref_data(sr));
                    builder.close_node();
                    continue;
                }
                _ => {}
            }
        }

        // If we're inside a code block, accumulate content.
        if code_block_buf.is_some() {
            match &event {
                Event::Text(text) => {
                    code_block_buf.as_mut().unwrap().push_str(text);
                    continue;
                }
                Event::End(TagEnd::CodeBlock) => {
                    let content = code_block_buf.take().unwrap();
                    let sr = builder.alloc_string(&content);
                    // Update the Code node's type_data to include the value.
                    let id = builder.current_node_id();
                    let existing_data = builder.arena_mut().get_type_data(id).to_vec();
                    if existing_data.len() >= 16 {
                        // Overwrite the value StringRef in CodeData (bytes 16-23).
                        let mut data = existing_data;
                        let sr_bytes = encode_string_ref_data(sr);
                        data[16..24].copy_from_slice(&sr_bytes);
                        builder.set_data_current(&data);
                    }
                    let node = builder.arena_mut().get_node(id);
                    let orig_start = node.start_offset;
                    let orig_start_line = node.start_line;
                    let orig_start_col = node.start_column;
                    builder.set_position_current(
                        orig_start,
                        end,
                        orig_start_line,
                        orig_start_col,
                        end_line,
                        end_col,
                    );
                    builder.close_node();
                    continue;
                }
                _ => {}
            }
        }

        // Accumulate image alt text (runs in parallel with normal event processing).
        if image_alt_buf.is_some() {
            match &event {
                Event::Text(t) => {
                    image_alt_buf.as_mut().unwrap().push_str(t);
                }
                Event::Code(c) => {
                    image_alt_buf.as_mut().unwrap().push_str(c);
                }
                Event::SoftBreak | Event::HardBreak => {
                    image_alt_buf.as_mut().unwrap().push(' ');
                }
                Event::Start(Tag::Image { .. }) => {
                    image_depth += 1;
                }
                Event::End(TagEnd::Image) => {
                    if image_depth > 0 {
                        image_depth -= 1;
                    } else {
                        // Closing the outermost image — update alt in ImageData.
                        let alt_text = image_alt_buf.take().unwrap();
                        let alt_ref = builder.alloc_string(&alt_text);
                        let id = builder.current_node_id();
                        let existing_data = builder.arena_mut().get_type_data(id).to_vec();
                        if existing_data.len() >= 24 {
                            // ImageData layout: url(8) + alt(8) + title(8) = 24 bytes.
                            // alt is bytes 8..16.
                            let mut data = existing_data;
                            let sr_bytes = encode_string_ref_data(alt_ref);
                            data[8..16].copy_from_slice(&sr_bytes);
                            builder.set_data_current(&data);
                        }
                        // Fall through to normal End(Image) handling below.
                    }
                }
                _ => {}
            }
        }

        match event {
            Event::Start(ref tag) => {
                // HtmlBlock and CodeBlock are containers in pulldown-cmark
                // but leaves with content in MDAST.
                match tag {
                    Tag::HtmlBlock => {
                        let _id = builder.open_node(MdastNodeType::Html);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        html_block_buf = Some(String::new());
                        continue;
                    }
                    Tag::CodeBlock(_) => {
                        let (node_type, data) = tag_to_node_type(tag, &mut builder, source);
                        let _id = builder.open_node(node_type);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        if let Some(d) = data {
                            builder.set_data_current(&d);
                        }
                        code_block_buf = Some(String::new());
                        continue;
                    }
                    Tag::MetadataBlock(_) => {
                        // Metadata blocks are also containers → leaf.
                        let _id = builder.open_node(MdastNodeType::Yaml);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        // Use the same html_block_buf trick to collect content.
                        html_block_buf = Some(String::new());
                        continue;
                    }
                    _ => {}
                }

                // JSX tag pairing: handle open/close/self-closing.
                if matches!(tag, Tag::MdxJsxFlowElement(_) | Tag::MdxJsxTextElement(_)) {
                    let raw = match tag {
                        Tag::MdxJsxFlowElement(s) | Tag::MdxJsxTextElement(s) => s.as_ref(),
                        _ => unreachable!(),
                    };
                    let jsx_kind = classify_jsx_tag(raw);

                    match jsx_kind {
                        JsxTagKind::SelfClosing => {
                            // Self-closing: open + immediately close.
                            let (node_type, data) = tag_to_node_type(tag, &mut builder, source);
                            let _id = builder.open_node(node_type);
                            builder.set_position_current(
                                start, end, start_line, start_col, end_line, end_col,
                            );
                            if let Some(d) = data {
                                builder.set_data_current(&d);
                            }
                            builder.close_node();
                            continue;
                        }
                        JsxTagKind::Opening(name) => {
                            // Opening: open node and push to stack.
                            let (node_type, data) = tag_to_node_type(tag, &mut builder, source);
                            let _id = builder.open_node(node_type);
                            builder.set_position_current(
                                start, end, start_line, start_col, end_line, end_col,
                            );
                            if let Some(d) = data {
                                builder.set_data_current(&d);
                            }
                            jsx_stack.push((name, start));
                            continue;
                        }
                        JsxTagKind::Closing(close_name) => {
                            if let Some((open_name, open_offset)) = jsx_stack.pop() {
                                if close_name != open_name {
                                    let open_loc =
                                        byte_offset_to_line_col(source, open_offset as usize);
                                    mdx_errors.push((
                                        start as usize,
                                        format!(
                                        "Unexpected closing tag `</{close_name}>`, expected \
                                         corresponding closing tag for `<{open_name}>` ({open_loc})"
                                    ),
                                    ));
                                }
                                let target_depth = find_jsx_depth(&builder);
                                let close_count = builder.stack_depth() - target_depth;

                                if close_count > 1 {
                                    skip_end_events += close_count - 1;
                                }
                                for _ in 0..close_count {
                                    let id = builder.current_node_id();
                                    let (
                                        orig_start,
                                        orig_start_line,
                                        orig_start_col,
                                        nt,
                                        node_start_line,
                                    ) = {
                                        let n = builder.arena_ref().get_node(id);
                                        (
                                            n.start_offset,
                                            n.start_line,
                                            n.start_column,
                                            n.node_type,
                                            n.start_line,
                                        )
                                    };
                                    builder.set_position_current(
                                        orig_start,
                                        end,
                                        orig_start_line,
                                        orig_start_col,
                                        end_line,
                                        end_col,
                                    );

                                    // Before closing the JSX node, check if it's
                                    // multi-line and should be promoted to flow.
                                    if nt == MdastNodeType::MdxJsxTextElement as u8
                                        && node_start_line != end_line
                                    {
                                        builder
                                            .change_node_type(id, MdastNodeType::MdxJsxFlowElement);
                                        wrap_bare_text_in_paragraphs(&mut builder, id);
                                    }

                                    builder.close_node();
                                }
                            } else {
                                mdx_errors.push((start as usize, format!(
                                    "Unexpected closing tag `</{close_name}>`, expected an open tag first"
                                )));
                            }
                            continue;
                        }
                    }
                }

                let (node_type, data) = tag_to_node_type(tag, &mut builder, source);
                let _id = builder.open_node(node_type);
                builder.set_position_current(start, end, start_line, start_col, end_line, end_col);
                if let Some(d) = data {
                    builder.set_data_current(&d);
                }

                // Start alt text accumulation for images.
                if matches!(tag, Tag::Image { .. }) && image_alt_buf.is_none() {
                    image_alt_buf = Some(String::new());
                }
            }
            Event::End(ref tag_end) => {
                // JSX End events from pulldown-cmark mark the end of the tag itself,
                // NOT the closing of the JSX container. Skip them — the container
                // is closed when we encounter the matching closing tag.
                if matches!(
                    tag_end,
                    TagEnd::MdxJsxFlowElement | TagEnd::MdxJsxTextElement
                ) {
                    continue;
                }

                // Skip End events for nodes we already closed during JSX pairing.
                if skip_end_events > 0 {
                    skip_end_events -= 1;
                    continue;
                }

                // MetadataBlock end is handled by the html_block_buf path above
                // (since we set html_block_buf for it). But the TagEnd is
                // MetadataBlock, not HtmlBlock, so we need to handle it.
                if matches!(tag_end, TagEnd::MetadataBlock(_)) && html_block_buf.is_none() {
                    // Already closed by the accumulation path.
                } else if matches!(tag_end, TagEnd::MetadataBlock(_)) {
                    let content = html_block_buf.take().unwrap();
                    let sr = builder.alloc_string(&content);
                    let id = builder.current_node_id();
                    let node = builder.arena_mut().get_node(id);
                    let orig_start = node.start_offset;
                    let orig_start_line = node.start_line;
                    let orig_start_col = node.start_column;
                    builder.set_position_current(
                        orig_start,
                        end,
                        orig_start_line,
                        orig_start_col,
                        end_line,
                        end_col,
                    );
                    builder.set_data_current(&encode_string_ref_data(sr));
                    builder.close_node();
                    continue;
                }

                // Update end position of the container.
                let id = builder.current_node_id();
                let node = builder.arena_mut().get_node(id);
                let orig_start = node.start_offset;
                let orig_start_line = node.start_line;
                let orig_start_col = node.start_column;
                builder.set_position_current(
                    orig_start,
                    end,
                    orig_start_line,
                    orig_start_col,
                    end_line,
                    end_col,
                );
                builder.close_node();
            }
            Event::Text(text) => {
                let sr = source_ref_or_alloc(source, &text, range.start, &mut builder);
                let id = builder.add_leaf(MdastNodeType::Text);
                builder
                    .arena_mut()
                    .set_position(id, start, end, start_line, start_col, end_line, end_col);
                builder
                    .arena_mut()
                    .set_type_data(id, &encode_string_ref_data(sr));
            }
            Event::Code(code) => {
                let sr = builder.alloc_string(&code);
                let id = builder.add_leaf(MdastNodeType::InlineCode);
                builder
                    .arena_mut()
                    .set_position(id, start, end, start_line, start_col, end_line, end_col);
                builder
                    .arena_mut()
                    .set_type_data(id, &encode_string_ref_data(sr));
            }
            Event::Html(html) => {
                // Standalone Html event (outside HtmlBlock).
                let sr = source_ref_or_alloc(source, &html, range.start, &mut builder);
                let id = builder.add_leaf(MdastNodeType::Html);
                builder
                    .arena_mut()
                    .set_position(id, start, end, start_line, start_col, end_line, end_col);
                builder
                    .arena_mut()
                    .set_type_data(id, &encode_string_ref_data(sr));
            }
            Event::InlineHtml(html) => {
                let sr = source_ref_or_alloc(source, &html, range.start, &mut builder);
                let id = builder.add_leaf(MdastNodeType::Html);
                builder
                    .arena_mut()
                    .set_position(id, start, end, start_line, start_col, end_line, end_col);
                builder
                    .arena_mut()
                    .set_type_data(id, &encode_string_ref_data(sr));
            }
            Event::SoftBreak => {
                let id = builder.add_leaf(MdastNodeType::Text);
                let sr = builder.alloc_string("\n");
                builder
                    .arena_mut()
                    .set_position(id, start, end, start_line, start_col, end_line, end_col);
                builder
                    .arena_mut()
                    .set_type_data(id, &encode_string_ref_data(sr));
            }
            Event::HardBreak => {
                let id = builder.add_leaf(MdastNodeType::Break);
                builder
                    .arena_mut()
                    .set_position(id, start, end, start_line, start_col, end_line, end_col);
            }
            Event::Rule => {
                let id = builder.add_leaf(MdastNodeType::ThematicBreak);
                builder
                    .arena_mut()
                    .set_position(id, start, end, start_line, start_col, end_line, end_col);
            }
            Event::TaskListMarker(checked) => {
                // Update the parent ListItem's checked state.
                // In tight lists, ListItem is the current stack top.
                // In loose lists, a Paragraph is open inside the ListItem,
                // so we walk up the stack to find it.
                let checked_val = if checked { 1 } else { 0 };
                let data = encode_list_item_data(checked_val, false);
                let depth = builder.stack_depth();
                for i in (0..depth).rev() {
                    if let Some(node_id) = builder.stack_node_id(i) {
                        if builder.arena_ref().get_node(node_id).node_type
                            == MdastNodeType::ListItem as u8
                        {
                            builder.arena_mut().set_type_data(node_id, &data);
                            break;
                        }
                    }
                }
            }
            Event::FootnoteReference(label) => {
                let sr = builder.alloc_string(&label);
                let id = builder.add_leaf(MdastNodeType::FootnoteReference);
                builder
                    .arena_mut()
                    .set_position(id, start, end, start_line, start_col, end_line, end_col);
                let data = tryckeri_mdast::encode_reference_data(sr, sr, 0);
                builder.arena_mut().set_type_data(id, &data);
            }
            Event::InlineMath(math) => {
                let sr = builder.alloc_string(&math);
                let id = builder.add_leaf(MdastNodeType::InlineMath);
                builder
                    .arena_mut()
                    .set_position(id, start, end, start_line, start_col, end_line, end_col);
                builder
                    .arena_mut()
                    .set_type_data(id, &encode_math_data(StringRef::empty(), sr));
            }
            Event::DisplayMath(math) => {
                let sr = builder.alloc_string(&math);
                let id = builder.add_leaf(MdastNodeType::Math);
                builder
                    .arena_mut()
                    .set_position(id, start, end, start_line, start_col, end_line, end_col);
                builder
                    .arena_mut()
                    .set_type_data(id, &encode_math_data(StringRef::empty(), sr));
            }
            Event::MdxFlowExpression(expr) => {
                let sr = builder.alloc_string(&expr);
                let id = builder.add_leaf(MdastNodeType::MdxFlowExpression);
                builder
                    .arena_mut()
                    .set_position(id, start, end, start_line, start_col, end_line, end_col);
                builder
                    .arena_mut()
                    .set_type_data(id, &encode_expression_data(sr));
            }
            Event::MdxTextExpression(expr) => {
                let sr = builder.alloc_string(&expr);
                let id = builder.add_leaf(MdastNodeType::MdxTextExpression);
                builder
                    .arena_mut()
                    .set_position(id, start, end, start_line, start_col, end_line, end_col);
                builder
                    .arena_mut()
                    .set_type_data(id, &encode_expression_data(sr));
            }
            Event::MdxEsm(code) => {
                let sr = builder.alloc_string(&code);
                let id = builder.add_leaf(MdastNodeType::MdxjsEsm);
                builder
                    .arena_mut()
                    .set_position(id, start, end, start_line, start_col, end_line, end_col);
                builder
                    .arena_mut()
                    .set_type_data(id, &encode_expression_data(sr));
            }
        }
    }

    // Check for unclosed JSX tags.
    for (name, offset) in &jsx_stack {
        let loc = byte_offset_to_line_col(source, *offset as usize);
        mdx_errors.push((
            *offset as usize,
            format!("Expected a closing tag for `<{name}>` ({loc})"),
        ));
    }

    // Merge with pulldown-cmark parser-level MDX errors.
    let parser_errors = parser.inner().mdx_errors();
    if !parser_errors.is_empty() {
        mdx_errors.extend_from_slice(parser_errors);
        mdx_errors.sort_by_key(|(offset, _)| *offset);
    }

    // Close root.
    builder.close_node();
    (builder.finish(), mdx_errors)
}

/// Convert a pulldown-cmark Tag to a MdastNodeType + optional type data.
fn tag_to_node_type(
    tag: &Tag<'_>,
    builder: &mut MdastBuilder,
    _source: &str,
) -> (MdastNodeType, Option<Vec<u8>>) {
    match tag {
        Tag::Paragraph => (MdastNodeType::Paragraph, None),
        Tag::Heading { level, .. } => {
            let depth = heading_level_to_u8(*level);
            (MdastNodeType::Heading, Some(encode_heading_data(depth)))
        }
        Tag::BlockQuote(_) => (MdastNodeType::Blockquote, None),
        Tag::CodeBlock(kind) => {
            let (lang, meta) = match kind {
                CodeBlockKind::Fenced(info) => {
                    let info_str = info.as_ref();
                    let (lang_str, meta_str) = match info_str.split_once(char::is_whitespace) {
                        Some((l, m)) => (l, m.trim()),
                        None => (info_str, ""),
                    };
                    let lang_ref = if lang_str.is_empty() {
                        StringRef::empty()
                    } else {
                        builder.alloc_string(lang_str)
                    };
                    let meta_ref = if meta_str.is_empty() {
                        StringRef::empty()
                    } else {
                        builder.alloc_string(meta_str)
                    };
                    (lang_ref, meta_ref)
                }
                CodeBlockKind::Indented => (StringRef::empty(), StringRef::empty()),
            };
            // Value will be filled by child Text events; for now store empty.
            (
                MdastNodeType::Code,
                Some(encode_code_data(lang, meta, StringRef::empty(), b'`')),
            )
        }
        Tag::List(first_item, is_tight) => {
            let ordered = first_item.is_some();
            let start_num = first_item.unwrap_or(0) as u32;
            (
                MdastNodeType::List,
                Some(encode_list_data(ordered, start_num, !is_tight)),
            )
        }
        Tag::Item => (
            MdastNodeType::ListItem,
            Some(encode_list_item_data(2, false)), // 2 = not a task item
        ),
        Tag::FootnoteDefinition(label) => {
            let sr = builder.alloc_string(label);
            (
                MdastNodeType::FootnoteDefinition,
                Some(encode_footnote_definition_data(sr, sr)),
            )
        }
        Tag::Table(alignments) => {
            let aligns: Vec<ColumnAlign> = alignments
                .iter()
                .map(|a| match a {
                    tryckeri_pulldown_cmark::Alignment::None => ColumnAlign::None,
                    tryckeri_pulldown_cmark::Alignment::Left => ColumnAlign::Left,
                    tryckeri_pulldown_cmark::Alignment::Center => ColumnAlign::Center,
                    tryckeri_pulldown_cmark::Alignment::Right => ColumnAlign::Right,
                })
                .collect();
            (MdastNodeType::Table, Some(encode_table_data(&aligns)))
        }
        Tag::TableHead => (MdastNodeType::TableRow, None),
        Tag::TableRow => (MdastNodeType::TableRow, None),
        Tag::TableCell => (MdastNodeType::TableCell, None),
        Tag::Emphasis => (MdastNodeType::Emphasis, None),
        Tag::Strong => (MdastNodeType::Strong, None),
        Tag::Strikethrough => (MdastNodeType::Delete, None),
        Tag::Link {
            dest_url, title, ..
        } => {
            let url_ref = builder.alloc_string(dest_url);
            let title_ref = if title.is_empty() {
                StringRef::empty()
            } else {
                builder.alloc_string(title)
            };
            (
                MdastNodeType::Link,
                Some(encode_link_data(url_ref, title_ref)),
            )
        }
        Tag::Image {
            dest_url, title, ..
        } => {
            let url_ref = builder.alloc_string(dest_url);
            let title_ref = if title.is_empty() {
                StringRef::empty()
            } else {
                builder.alloc_string(title)
            };
            let alt_ref = StringRef::empty(); // alt is filled by child text
            (
                MdastNodeType::Image,
                Some(encode_image_data(url_ref, alt_ref, title_ref)),
            )
        }
        Tag::HtmlBlock => (MdastNodeType::Html, None),
        Tag::MetadataBlock(_) => (MdastNodeType::Yaml, None),
        Tag::DefinitionList | Tag::DefinitionListTitle | Tag::DefinitionListDefinition => {
            // Map definition lists to paragraphs for now.
            (MdastNodeType::Paragraph, None)
        }
        Tag::Superscript | Tag::Subscript => (MdastNodeType::Emphasis, None),
        Tag::ContainerBlock(_, _) => (MdastNodeType::Blockquote, None),
        Tag::MdxJsxFlowElement(raw) => {
            let data = encode_jsx_element(raw, builder);
            (MdastNodeType::MdxJsxFlowElement, Some(data))
        }
        Tag::MdxJsxTextElement(raw) => {
            let data = encode_jsx_element(raw, builder);
            (MdastNodeType::MdxJsxTextElement, Some(data))
        }
    }
}

/// Wrap bare text children (non-newline) of a multi-line JSX element in Paragraph nodes.
///
/// Transforms children like `[Text("\n"), Text("b"), Text("\n")]` into
/// `[Paragraph([Text("b")])]` (newline-only text nodes are dropped).
///
/// Must be called BEFORE `close_node()` while the node's children are still pending.
fn wrap_bare_text_in_paragraphs(builder: &mut MdastBuilder, _jsx_id: u32) {
    // Phase 1: Read — collect child info without holding borrows.
    let old_children: Vec<u32> = builder.current_children_mut().clone();
    let mut is_newline: Vec<bool> = Vec::with_capacity(old_children.len());

    for &child_id in &old_children {
        let node = builder.arena_ref().get_node(child_id);
        let newline = if node.node_type == MdastNodeType::Text as u8 {
            let data = builder.arena_ref().get_type_data(child_id);
            if data.len() >= 8 {
                let sr = tryckeri_mdast::decode_string_ref_data(data);
                let text = builder.arena_ref().get_str(sr);
                text.chars().all(|c| c == '\n' || c == '\r')
            } else {
                false
            }
        } else {
            false
        };
        is_newline.push(newline);
    }

    // Phase 2: Find runs of non-newline nodes.
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut run_start: Option<usize> = None;
    for (i, &nl) in is_newline.iter().enumerate() {
        if nl {
            if let Some(s) = run_start.take() {
                runs.push((s, i));
            }
        } else if run_start.is_none() {
            run_start = Some(i);
        }
    }
    if let Some(s) = run_start {
        runs.push((s, old_children.len()));
    }

    // If there's only one run covering everything, it's inline — don't wrap.
    if runs.is_empty() || (runs.len() == 1 && runs[0].0 == 0 && runs[0].1 == old_children.len()) {
        return;
    }

    // Phase 3: Mutate — create paragraph wrappers, rebuild children.
    let mut new_children: Vec<u32> = Vec::new();

    for &(rs, re) in &runs {
        let run_child_ids: Vec<u32> = old_children[rs..re].to_vec();

        // Read positions before mutating.
        let first = builder.arena_ref().get_node(run_child_ids[0]);
        let last = builder.arena_ref().get_node(*run_child_ids.last().unwrap());
        let pos = (
            first.start_offset,
            last.end_offset,
            first.start_line,
            first.start_column,
            last.end_line,
            last.end_column,
        );

        let para_id = builder.arena_mut().alloc_node(MdastNodeType::Paragraph);
        builder.arena_mut().set_children(para_id, &run_child_ids);
        for &c in &run_child_ids {
            builder.arena_mut().set_parent(c, para_id);
        }
        builder
            .arena_mut()
            .set_position(para_id, pos.0, pos.1, pos.2, pos.3, pos.4, pos.5);
        new_children.push(para_id);
    }

    // Replace children (drop newline-only nodes, keep paragraph-wrapped runs).
    let children = builder.current_children_mut();
    children.clear();
    children.extend_from_slice(&new_children);
}

/// Convert byte offset to a "line:column" string for error messages.
fn byte_offset_to_line_col(source: &str, offset: usize) -> String {
    let mut line = 1usize;
    let mut col = 1usize;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    format!("{line}:{col}")
}

/// Find the stack depth of the open JSX element.
/// Scans from top of stack downward, returns the 1-based depth to close to.
fn find_jsx_depth(builder: &MdastBuilder) -> usize {
    let depth = builder.stack_depth();
    for i in (0..depth).rev() {
        if let Some(node_id) = builder.stack_node_id(i) {
            let nt = builder.arena_ref().get_node(node_id).node_type;
            if nt == MdastNodeType::MdxJsxFlowElement as u8
                || nt == MdastNodeType::MdxJsxTextElement as u8
            {
                return i;
            }
        }
    }
    depth.saturating_sub(1)
}

/// Classify a JSX tag as opening, closing, or self-closing.
#[derive(Debug)]
enum JsxTagKind {
    Opening(String),
    Closing(String),
    SelfClosing,
}

fn classify_jsx_tag(raw: &str) -> JsxTagKind {
    let s = raw.trim();
    // Closing: starts with `</` (check before self-closing since `</>` ends with `/>`)
    if s.starts_with("</") {
        let name = extract_jsx_name(s);
        return JsxTagKind::Closing(name.to_string());
    }
    // Self-closing: ends with `/>`
    if s.ends_with("/>") {
        return JsxTagKind::SelfClosing;
    }
    // Check for self-contained: `<Name ...>...</Name>` or `<>...</>`
    // (flow JSX elements can contain open+close in a single event).
    let name = extract_jsx_name(s);
    if !name.is_empty() {
        let close_tag = format!("</{name}>");
        if s.contains(&close_tag) {
            return JsxTagKind::SelfClosing;
        }
    } else {
        // Fragment: check for `</>` closing
        if s.contains("</>") {
            return JsxTagKind::SelfClosing;
        }
    }
    JsxTagKind::Opening(name.to_string())
}

/// Extract the tag name from a raw JSX string like `<Component x={1} />`.
fn extract_jsx_name(raw: &str) -> &str {
    let s = raw.trim_start_matches('<').trim_start_matches('/');
    // Find end of name: space, `/`, `>`, `{`, or newline.
    let end = s
        .find(|c: char| c.is_whitespace() || c == '/' || c == '>' || c == '{')
        .unwrap_or(s.len());
    &s[..end]
}

/// Parse a raw JSX tag string, extract name + attributes, and encode as MDAST type_data.
fn encode_jsx_element(raw: &str, builder: &mut MdastBuilder) -> Vec<u8> {
    let name = extract_jsx_name(raw);
    let name_ref = if name.is_empty() {
        StringRef::empty()
    } else {
        builder.alloc_string(name)
    };

    let parsed_attrs = parse_jsx_attributes_from_tag(raw);
    let attr_tuples: Vec<(u8, StringRef, StringRef)> = parsed_attrs
        .iter()
        .map(|attr| match attr {
            JsxAttr::BooleanProp(n) => {
                let n = builder.alloc_string(n);
                (MDX_ATTR_BOOLEAN_PROP, n, StringRef::empty())
            }
            JsxAttr::LiteralProp(n, v) => {
                let n = builder.alloc_string(n);
                let v = builder.alloc_string(v);
                (MDX_ATTR_LITERAL_PROP, n, v)
            }
            JsxAttr::ExpressionProp(n, v) => {
                let n = builder.alloc_string(n);
                let v = builder.alloc_string(v);
                (MDX_ATTR_EXPRESSION_PROP, n, v)
            }
            JsxAttr::Spread(v) => {
                let v = builder.alloc_string(v);
                (MDX_ATTR_SPREAD, StringRef::empty(), v)
            }
        })
        .collect();

    encode_mdx_jsx_element_data(name_ref, &attr_tuples)
}

/// Try to create a StringRef that points into the source (zero-copy).
/// Falls back to allocating a copy if the text doesn't match the source range.
fn source_ref_or_alloc(
    source: &str,
    text: &str,
    offset: usize,
    builder: &mut MdastBuilder,
) -> StringRef {
    // Check if the text is a direct slice of the source at the expected offset.
    if let Some(slice) = source.get(offset..offset + text.len()) {
        if slice == text {
            return StringRef::new(offset as u32, text.len() as u32);
        }
    }
    builder.alloc_string(text)
}

fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tryckeri_mdast::{decode_heading_data, decode_list_item_data};

    #[test]
    fn parse_simple_paragraph() {
        let (arena, _) = parse("hello world", &ParseOptions::default());
        assert!(arena.len() >= 2); // root + paragraph + text
        let root = arena.get_node(0);
        assert_eq!(root.node_type, MdastNodeType::Root as u8);
    }

    #[test]
    fn parse_heading() {
        let (arena, _) = parse("# Hello\n", &ParseOptions::default());
        // Find the heading node.
        let heading = (0..arena.len() as u32)
            .map(|i| arena.get_node(i))
            .find(|n| n.node_type == MdastNodeType::Heading as u8)
            .expect("should have a heading");
        let hd = decode_heading_data(arena.get_type_data(heading.id));
        assert_eq!(hd.depth, 1);
    }

    #[test]
    fn parse_emphasis() {
        let (arena, _) = parse("*hello*", &ParseOptions::default());
        let has_em = (0..arena.len() as u32)
            .any(|i| arena.get_node(i).node_type == MdastNodeType::Emphasis as u8);
        assert!(has_em);
    }

    #[test]
    fn parse_link() {
        let (arena, _) = parse("[text](https://example.com)", &ParseOptions::default());
        let link = (0..arena.len() as u32)
            .map(|i| arena.get_node(i))
            .find(|n| n.node_type == MdastNodeType::Link as u8)
            .expect("should have a link");
        let data = tryckeri_mdast::decode_link_data(arena.get_type_data(link.id));
        assert_eq!(arena.get_str(data.url), "https://example.com");
    }

    #[test]
    fn parse_code_block() {
        let (arena, _) = parse("```rust\nfn main() {}\n```\n", &ParseOptions::default());
        let code = (0..arena.len() as u32)
            .map(|i| arena.get_node(i))
            .find(|n| n.node_type == MdastNodeType::Code as u8)
            .expect("should have a code block");
        let data = tryckeri_mdast::decode_code_data(arena.get_type_data(code.id));
        assert_eq!(arena.get_str(data.lang), "rust");
    }

    #[test]
    fn parse_list() {
        let (arena, _) = parse("- a\n- b\n", &ParseOptions::default());
        let list = (0..arena.len() as u32)
            .map(|i| arena.get_node(i))
            .find(|n| n.node_type == MdastNodeType::List as u8)
            .expect("should have a list");
        let data = tryckeri_mdast::decode_list_data(arena.get_type_data(list.id));
        assert!(!data.ordered);
    }

    #[test]
    fn parse_mdx_expression() {
        let (arena, _) = parse("{1 + 1}\n", &ParseOptions::mdx());
        let expr = (0..arena.len() as u32)
            .map(|i| arena.get_node(i))
            .find(|n| n.node_type == MdastNodeType::MdxFlowExpression as u8)
            .expect("should have an MDX expression");
        let data = tryckeri_mdast::decode_expression_data(arena.get_type_data(expr.id));
        assert_eq!(arena.get_str(data.value), "1 + 1");
    }

    #[test]
    fn parse_mdx_jsx() {
        let (arena, _) = parse("<Component />\n", &ParseOptions::mdx());
        let jsx = (0..arena.len() as u32)
            .map(|i| arena.get_node(i))
            .find(|n| n.node_type == MdastNodeType::MdxJsxFlowElement as u8)
            .expect("should have an MDX JSX element");
        let data = arena.get_type_data(jsx.id);
        let name_ref = tryckeri_mdast::decode_mdx_jsx_element_name(data);
        assert_eq!(arena.get_str(name_ref), "Component");
        assert_eq!(tryckeri_mdast::decode_mdx_jsx_attr_count(data), 0);
    }

    #[test]
    fn parse_mdx_esm() {
        let (arena, _) = parse("import a from 'b'\n\nc\n", &ParseOptions::mdx());
        let esm = (0..arena.len() as u32)
            .map(|i| arena.get_node(i))
            .find(|n| n.node_type == MdastNodeType::MdxjsEsm as u8)
            .expect("should have an MDX ESM node");
        let data = tryckeri_mdast::decode_expression_data(arena.get_type_data(esm.id));
        assert!(arena.get_str(data.value).contains("import"));
    }

    #[test]
    fn parse_mdx_expression_in_heading() {
        let (arena, errors) = parse("# {title}\n", &ParseOptions::mdx());
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);

        let heading = (0..arena.len() as u32)
            .map(|i| arena.get_node(i))
            .find(|n| n.node_type == MdastNodeType::Heading as u8)
            .expect("should have a heading");

        let children = arena.get_children(heading.id);
        assert!(
            !children.is_empty(),
            "heading should have children (the MDX expression)"
        );

        let expr = children
            .iter()
            .map(|&id| arena.get_node(id))
            .find(|n| n.node_type == MdastNodeType::MdxTextExpression as u8)
            .expect("heading should have MdxTextExpression child");
        let data = tryckeri_mdast::decode_expression_data(arena.get_type_data(expr.id));
        assert_eq!(arena.get_str(data.value), "title");
    }

    #[test]
    fn parse_mdx_expression_mixed_with_text_in_heading() {
        let (arena, errors) = parse("## Hello {name}\n", &ParseOptions::mdx());
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);

        let heading = (0..arena.len() as u32)
            .map(|i| arena.get_node(i))
            .find(|n| n.node_type == MdastNodeType::Heading as u8)
            .expect("should have a heading");

        let children = arena.get_children(heading.id);
        assert!(
            children.len() >= 2,
            "heading should have text + expression children, got {} children",
            children.len()
        );

        // Should have a text child and an expression child
        let has_text = children
            .iter()
            .any(|&id| arena.get_node(id).node_type == MdastNodeType::Text as u8);
        let has_expr = children
            .iter()
            .any(|&id| arena.get_node(id).node_type == MdastNodeType::MdxTextExpression as u8);
        assert!(has_text, "heading should have text child");
        assert!(has_expr, "heading should have expression child");
    }

    #[test]
    fn parse_task_list() {
        let (arena, _) = parse("- [ ] unchecked\n- [x] checked\n", &ParseOptions::default());
        let items: Vec<_> = (0..arena.len() as u32)
            .filter(|&i| arena.get_node(i).node_type == MdastNodeType::ListItem as u8)
            .collect();
        assert_eq!(items.len(), 2);

        let first = decode_list_item_data(arena.get_type_data(items[0]));
        assert_eq!(first.checked, 0, "first item should be unchecked");

        let second = decode_list_item_data(arena.get_type_data(items[1]));
        assert_eq!(second.checked, 1, "second item should be checked");
    }

    #[test]
    fn parse_regular_list_not_task() {
        let (arena, _) = parse("- plain item\n", &ParseOptions::default());
        let item = (0..arena.len() as u32)
            .find(|&i| arena.get_node(i).node_type == MdastNodeType::ListItem as u8)
            .expect("should have a list item");
        let data = decode_list_item_data(arena.get_type_data(item));
        assert_eq!(
            data.checked, 2,
            "regular list item should not be a task item"
        );
    }

    #[test]
    fn roundtrip_to_buffer() {
        let (arena, _) = parse("# Hello\n\nworld\n", &ParseOptions::default());
        let buf = arena.to_raw_buffer();
        let view = MdastArena::from_raw_buffer(&buf).expect("valid buffer");
        assert_eq!(view.len(), arena.len());
    }
}
