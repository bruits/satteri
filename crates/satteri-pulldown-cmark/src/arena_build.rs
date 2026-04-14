//! Direct arena builder: walks the pulldown-cmark internal tree and builds
//! a `satteri_arena::Arena` without going through the Event iterator.

use satteri_arena::{Arena, ArenaBuilder, LineIndex, StringRef};
use satteri_ast::mdast::{
    encode_mdx_jsx_element_data, encode_table_data, CodeData, ColumnAlign, ExpressionData,
    FootnoteDefinitionData, ImageData, LinkData, ListData, ListItemData, MathData, MdastNodeType,
    ReferenceData,
};
use satteri_ast::shared::{
    MDX_ATTR_BOOLEAN_PROP, MDX_ATTR_EXPRESSION_PROP, MDX_ATTR_LITERAL_PROP, MDX_ATTR_SPREAD,
};

use crate::parse::{DefaultParserCallbacks, ItemBody, JsxAttr, ParserInner};
use crate::{Alignment, HeadingLevel, Options};

/// Default options: tables, footnotes, strikethrough, task lists, math, heading attributes, YAML metadata.
pub const DEFAULT_OPTIONS: Options = Options::from_bits_truncate(
    Options::ENABLE_TABLES.bits()
        | Options::ENABLE_FOOTNOTES.bits()
        | Options::ENABLE_STRIKETHROUGH.bits()
        | Options::ENABLE_TASKLISTS.bits()
        | Options::ENABLE_MATH.bits()
        | Options::ENABLE_HEADING_ATTRIBUTES.bits()
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS.bits(),
);

/// MDX options: default options plus JSX, expressions, and ESM.
pub const MDX_OPTIONS: Options =
    Options::from_bits_truncate(DEFAULT_OPTIONS.bits() | Options::ENABLE_MDX.bits());

/// Parse markdown source into an Arena.
///
/// Returns `(arena, mdx_errors)` where `mdx_errors` contains any MDX
/// validation errors collected during parsing (empty for non-MDX input).
pub fn parse(source: &str, options: Options) -> (Arena, Vec<(usize, String)>) {
    let line_index = LineIndex::from_source(source);
    let mut cursor = line_index.cursor();

    // Pre-allocate based on source size heuristics.
    let estimated_nodes = source.len() / 18 + 16;
    let source_extra = source.len() / 2;
    let mut source_buf = String::with_capacity(source.len() + source_extra);
    source_buf.push_str(source);
    let arena = Arena::with_capacity(
        source_buf,
        estimated_nodes,
        estimated_nodes,
        estimated_nodes * 9,
    );
    let mut builder = ArenaBuilder::from_arena(arena);

    // Build the pulldown-cmark parser (runs first pass).
    let mut inner = ParserInner::new(source, options);
    let mut callbacks = DefaultParserCallbacks;

    // Open root node.
    builder.open_node(MdastNodeType::Root as u8);
    let (end_line, end_col) = cursor.offset_to_line_col(source.len() as u32);
    builder.set_position_current(0, source.len() as u32, 1, 1, end_line, end_col);

    // Accumulation buffers for special container→leaf conversions.
    let mut html_block_buf: Option<String> = None;
    let mut code_block_buf: Option<String> = None;
    let mut image_alt_buf: Option<String> = None;
    let mut image_depth: usize = 0;

    // JSX tag pairing state.
    let mut jsx_stack: Vec<(String, u32)> = Vec::new();
    let mut mdx_errors: Vec<(usize, String)> = Vec::new();

    // Walk the tree iteratively.
    loop {
        match inner.tree.cur() {
            None => {
                // Backing out of a container, emit close.
                let ix = match inner.tree.pop() {
                    Some(ix) => ix,
                    None => break, // Done: popped past root.
                };

                // Skip TightParagraph (no MDAST node).
                if matches!(inner.tree[ix].item.body, ItemBody::TightParagraph) {
                    inner.tree.next_sibling(ix);
                    continue;
                }

                let item = inner.tree[ix].item;
                let end = item.end as u32;
                let (end_line, end_col) = cursor.offset_to_line_col(end);

                match &inner.tree[ix].item.body {
                    // Code block close: write accumulated content.
                    ItemBody::FencedCodeBlock(_) | ItemBody::IndentCodeBlock => {
                        if let Some(content) = code_block_buf.take() {
                            let sr = builder.alloc_string(&content);
                            let id = builder.current_node_id();
                            let existing_data = builder.arena_ref().get_type_data(id).to_vec();
                            if existing_data.len() >= 16 {
                                let mut data = existing_data;
                                data[16..24].copy_from_slice(&sr.as_bytes());
                                builder.set_data_current(&data);
                            }
                            let node = builder.arena_ref().get_node(id);
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
                    }
                    // HTML block close: write accumulated content.
                    ItemBody::HtmlBlock => {
                        if let Some(content) = html_block_buf.take() {
                            let sr = builder.alloc_string(&content);
                            let id = builder.current_node_id();
                            let node = builder.arena_ref().get_node(id);
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
                            builder.set_data_current(&sr.as_bytes());
                            builder.close_node();
                        }
                    }
                    // Metadata block close: same as HTML block.
                    ItemBody::MetadataBlock(_) => {
                        if let Some(content) = html_block_buf.take() {
                            let sr = builder.alloc_string(&content);
                            let id = builder.current_node_id();
                            let node = builder.arena_ref().get_node(id);
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
                            builder.set_data_current(&sr.as_bytes());
                            builder.close_node();
                        }
                    }
                    // Image close: finalize alt text.
                    ItemBody::Image(_) => {
                        if image_depth > 0 {
                            image_depth -= 1;
                        } else if let Some(alt_text) = image_alt_buf.take() {
                            let alt_ref = builder.alloc_string(&alt_text);
                            let id = builder.current_node_id();
                            let existing_data = builder.arena_ref().get_type_data(id).to_vec();
                            if existing_data.len() >= 24 {
                                let mut data = existing_data;
                                data[8..16].copy_from_slice(&alt_ref.as_bytes());
                                builder.set_data_current(&data);
                            }
                        }
                        // Update end position and close.
                        let id = builder.current_node_id();
                        let node = builder.arena_ref().get_node(id);
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
                    // Regular container close.
                    _ => {
                        let id = builder.current_node_id();
                        let node = builder.arena_ref().get_node(id);
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
                }

                inner.tree.next_sibling(ix);
            }
            Some(cur_ix) => {
                // Skip TightParagraph: push through to children.
                if matches!(inner.tree[cur_ix].item.body, ItemBody::TightParagraph) {
                    inner.tree.push();
                    continue;
                }

                // Resolve inline markup if needed.
                if inner.tree[cur_ix].item.body.is_maybe_inline() {
                    inner.handle_inline(&mut callbacks);
                }

                let item = inner.tree[cur_ix].item;
                let start = item.start as u32;
                let end = item.end as u32;
                let (start_line, start_col) = cursor.offset_to_line_col(start);
                let (end_line, end_col) = cursor.offset_to_line_col(end);

                // If we're accumulating content for an HTML/code block, handle it.
                if let Some(buf) = html_block_buf.as_mut() {
                    match &item.body {
                        ItemBody::Text { .. } | ItemBody::Html | ItemBody::SoftBreak => {
                            let text = if matches!(item.body, ItemBody::SoftBreak) {
                                "\n"
                            } else {
                                &source[item.start..item.end]
                            };
                            buf.push_str(text);
                            inner.tree.next_sibling(cur_ix);
                            continue;
                        }
                        _ => {}
                    }
                }

                if let Some(buf) = code_block_buf.as_mut() {
                    if let ItemBody::Text { .. } = &item.body {
                        buf.push_str(&source[item.start..item.end]);
                        inner.tree.next_sibling(cur_ix);
                        continue;
                    }
                }

                // Accumulate image alt text.
                if let Some(buf) = image_alt_buf.as_mut() {
                    match &item.body {
                        ItemBody::Text { .. } => {
                            buf.push_str(&source[item.start..item.end]);
                        }
                        ItemBody::Code(cow_ix) => {
                            let cow = inner.allocs.take_cow(*cow_ix);
                            buf.push_str(&cow);
                        }
                        ItemBody::SoftBreak | ItemBody::HardBreak(_) => {
                            buf.push(' ');
                        }
                        ItemBody::SynthesizeText(cow_ix) => {
                            let cow = inner.allocs.take_cow(*cow_ix);
                            buf.push_str(&cow);
                        }
                        ItemBody::SynthesizeChar(c) => {
                            buf.push(*c);
                        }
                        ItemBody::Image(_) => {
                            image_depth += 1;
                        }
                        _ => {}
                    }
                }

                // Map ItemBody to arena node.
                match item.body {
                    ItemBody::Paragraph => {
                        builder.open_node(MdastNodeType::Paragraph as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        inner.tree.push();
                    }
                    ItemBody::Heading(level, _) => {
                        let depth = heading_level_to_u8(level);
                        builder.open_node(MdastNodeType::Heading as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        builder.set_data_current(&[depth]);
                        inner.tree.push();
                    }
                    ItemBody::BlockQuote(_) => {
                        builder.open_node(MdastNodeType::Blockquote as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        inner.tree.push();
                    }
                    ItemBody::FencedCodeBlock(cow_ix) => {
                        let info_cow = inner.allocs.take_cow(cow_ix);
                        let info_str = info_cow.as_ref();
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
                        builder.open_node(MdastNodeType::Code as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        let cd = CodeData {
                            lang: lang_ref,
                            meta: meta_ref,
                            value: StringRef::empty(),
                            fence_char: b'`',
                            _pad: [0; 3],
                        };
                        builder.set_data_current(&cd.to_bytes());
                        code_block_buf = Some(String::with_capacity(256));
                        inner.tree.push();
                    }
                    ItemBody::IndentCodeBlock => {
                        builder.open_node(MdastNodeType::Code as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        let cd = CodeData {
                            lang: StringRef::empty(),
                            meta: StringRef::empty(),
                            value: StringRef::empty(),
                            fence_char: b' ',
                            _pad: [0; 3],
                        };
                        builder.set_data_current(&cd.to_bytes());
                        code_block_buf = Some(String::with_capacity(256));
                        inner.tree.push();
                    }
                    ItemBody::List(is_tight, c, listitem_start) => {
                        let ordered = c == b'.' || c == b')';
                        let start_num = if ordered { listitem_start as u32 } else { 0 };
                        builder.open_node(MdastNodeType::List as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        let ld = ListData {
                            start: start_num,
                            ordered,
                            spread: !is_tight,
                            _pad: [0; 2],
                        };
                        builder.set_data_current(&ld.to_bytes());
                        inner.tree.push();
                    }
                    ItemBody::ListItem(_) => {
                        builder.open_node(MdastNodeType::ListItem as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        builder.set_data_current(
                            &ListItemData {
                                checked: 2,
                                spread: false,
                            }
                            .to_bytes(),
                        );
                        inner.tree.push();
                    }
                    ItemBody::Table(align_ix) => {
                        let alignments = inner.allocs.take_alignment(align_ix);
                        let aligns: Vec<ColumnAlign> = alignments
                            .iter()
                            .map(|a| match a {
                                Alignment::None => ColumnAlign::None,
                                Alignment::Left => ColumnAlign::Left,
                                Alignment::Center => ColumnAlign::Center,
                                Alignment::Right => ColumnAlign::Right,
                            })
                            .collect();
                        builder.open_node(MdastNodeType::Table as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        builder.set_data_current(&encode_table_data(&aligns));
                        inner.tree.push();
                    }
                    ItemBody::TableHead | ItemBody::TableRow => {
                        builder.open_node(MdastNodeType::TableRow as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        inner.tree.push();
                    }
                    ItemBody::TableCell => {
                        builder.open_node(MdastNodeType::TableCell as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        inner.tree.push();
                    }
                    ItemBody::Emphasis => {
                        builder.open_node(MdastNodeType::Emphasis as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        inner.tree.push();
                    }
                    ItemBody::Strong => {
                        builder.open_node(MdastNodeType::Strong as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        inner.tree.push();
                    }
                    ItemBody::Strikethrough => {
                        builder.open_node(MdastNodeType::Delete as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        inner.tree.push();
                    }
                    ItemBody::Link(link_ix) => {
                        let (_link_type, dest_url, title, _id) = inner.allocs.take_link(link_ix);
                        let url_ref = builder.alloc_string(&dest_url);
                        let title_ref = if title.is_empty() {
                            StringRef::empty()
                        } else {
                            builder.alloc_string(&title)
                        };
                        builder.open_node(MdastNodeType::Link as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        builder.set_data_current(
                            &LinkData {
                                url: url_ref,
                                title: title_ref,
                            }
                            .to_bytes(),
                        );
                        inner.tree.push();
                    }
                    ItemBody::Image(link_ix) => {
                        let (_link_type, dest_url, title, _id) = inner.allocs.take_link(link_ix);
                        let url_ref = builder.alloc_string(&dest_url);
                        let title_ref = if title.is_empty() {
                            StringRef::empty()
                        } else {
                            builder.alloc_string(&title)
                        };
                        let alt_ref = StringRef::empty();
                        builder.open_node(MdastNodeType::Image as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        builder.set_data_current(
                            &ImageData {
                                url: url_ref,
                                alt: alt_ref,
                                title: title_ref,
                            }
                            .to_bytes(),
                        );
                        if image_alt_buf.is_none() {
                            image_alt_buf = Some(String::with_capacity(64));
                        }
                        inner.tree.push();
                    }
                    ItemBody::FootnoteDefinition(cow_ix) => {
                        let label_cow = inner.allocs.take_cow(cow_ix);
                        let sr = builder.alloc_string(&label_cow);
                        builder.open_node(MdastNodeType::FootnoteDefinition as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        builder.set_data_current(
                            &FootnoteDefinitionData {
                                identifier: sr,
                                label: sr,
                            }
                            .to_bytes(),
                        );
                        inner.tree.push();
                    }
                    ItemBody::HtmlBlock => {
                        builder.open_node(MdastNodeType::Html as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        html_block_buf = Some(String::with_capacity(128));
                        inner.tree.push();
                    }
                    ItemBody::MetadataBlock(_) => {
                        builder.open_node(MdastNodeType::Yaml as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        html_block_buf = Some(String::with_capacity(128));
                        inner.tree.push();
                    }
                    // MDX JSX elements.
                    ItemBody::MdxJsxFlowElement(jsx_ix) | ItemBody::MdxJsxTextElement(jsx_ix) => {
                        let is_flow = matches!(item.body, ItemBody::MdxJsxFlowElement(_));
                        let jsx = inner.allocs.take_jsx_element(jsx_ix);

                        if jsx.is_closing {
                            let close_name = jsx.name.as_ref();
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
                                let id = builder.current_node_id();
                                let node = builder.arena_ref().get_node(id);
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
                            } else {
                                mdx_errors.push((
                                    start as usize,
                                    format!("Unexpected closing tag `</{close_name}>`, expected an open tag first"),
                                ));
                            }
                        } else {
                            let node_type = if is_flow {
                                MdastNodeType::MdxJsxFlowElement
                            } else {
                                MdastNodeType::MdxJsxTextElement
                            };
                            let data = encode_jsx_element_data(&jsx, &mut builder);
                            builder.open_node(node_type as u8);
                            builder.set_position_current(
                                start, end, start_line, start_col, end_line, end_col,
                            );
                            builder.set_data_current(&data);
                            if jsx.is_self_closing {
                                builder.close_node();
                            } else {
                                jsx_stack.push((jsx.name.to_string(), start));
                            }
                        }
                        inner.tree.next_sibling(cur_ix);
                        continue;
                    }

                    // Definition list → map to paragraph for now.
                    ItemBody::DefinitionList(_)
                    | ItemBody::DefinitionListTitle
                    | ItemBody::DefinitionListDefinition(_) => {
                        builder.open_node(MdastNodeType::Paragraph as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        inner.tree.push();
                    }
                    // Superscript/Subscript → Emphasis for now.
                    ItemBody::Superscript | ItemBody::Subscript => {
                        builder.open_node(MdastNodeType::Emphasis as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        inner.tree.push();
                    }
                    // Container extension blocks → Blockquote.
                    ItemBody::Container(_, _, _) => {
                        builder.open_node(MdastNodeType::Blockquote as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        inner.tree.push();
                    }

                    ItemBody::Text { .. } => {
                        let sr = StringRef::new(start, end - start);
                        builder.add_leaf_full(
                            MdastNodeType::Text as u8,
                            start,
                            end,
                            start_line,
                            start_col,
                            end_line,
                            end_col,
                            &sr.as_bytes(),
                        );
                        inner.tree.next_sibling(cur_ix);
                    }
                    ItemBody::Code(cow_ix) => {
                        let cow = inner.allocs.take_cow(cow_ix);
                        let sr = builder.alloc_string(&cow);
                        builder.add_leaf_full(
                            MdastNodeType::InlineCode as u8,
                            start,
                            end,
                            start_line,
                            start_col,
                            end_line,
                            end_col,
                            &sr.as_bytes(),
                        );
                        inner.tree.next_sibling(cur_ix);
                    }
                    ItemBody::SynthesizeText(cow_ix) => {
                        let cow = inner.allocs.take_cow(cow_ix);
                        let sr = builder.alloc_string(&cow);
                        builder.add_leaf_full(
                            MdastNodeType::Text as u8,
                            start,
                            end,
                            start_line,
                            start_col,
                            end_line,
                            end_col,
                            &sr.as_bytes(),
                        );
                        inner.tree.next_sibling(cur_ix);
                    }
                    ItemBody::SynthesizeChar(c) => {
                        let s = String::from(c);
                        let sr = builder.alloc_string(&s);
                        builder.add_leaf_full(
                            MdastNodeType::Text as u8,
                            start,
                            end,
                            start_line,
                            start_col,
                            end_line,
                            end_col,
                            &sr.as_bytes(),
                        );
                        inner.tree.next_sibling(cur_ix);
                    }
                    ItemBody::Html => {
                        let sr = StringRef::new(start, end - start);
                        builder.add_leaf_full(
                            MdastNodeType::Html as u8,
                            start,
                            end,
                            start_line,
                            start_col,
                            end_line,
                            end_col,
                            &sr.as_bytes(),
                        );
                        inner.tree.next_sibling(cur_ix);
                    }
                    ItemBody::InlineHtml => {
                        let sr = StringRef::new(start, end - start);
                        builder.add_leaf_full(
                            MdastNodeType::Html as u8,
                            start,
                            end,
                            start_line,
                            start_col,
                            end_line,
                            end_col,
                            &sr.as_bytes(),
                        );
                        inner.tree.next_sibling(cur_ix);
                    }
                    ItemBody::OwnedInlineHtml(cow_ix) => {
                        let cow = inner.allocs.take_cow(cow_ix);
                        let sr = builder.alloc_string(&cow);
                        builder.add_leaf_full(
                            MdastNodeType::Html as u8,
                            start,
                            end,
                            start_line,
                            start_col,
                            end_line,
                            end_col,
                            &sr.as_bytes(),
                        );
                        inner.tree.next_sibling(cur_ix);
                    }
                    ItemBody::SoftBreak => {
                        let sr = builder.alloc_string("\n");
                        builder.add_leaf_full(
                            MdastNodeType::Text as u8,
                            start,
                            end,
                            start_line,
                            start_col,
                            end_line,
                            end_col,
                            &sr.as_bytes(),
                        );
                        inner.tree.next_sibling(cur_ix);
                    }
                    ItemBody::HardBreak(_) => {
                        builder.add_leaf_full(
                            MdastNodeType::Break as u8,
                            start,
                            end,
                            start_line,
                            start_col,
                            end_line,
                            end_col,
                            &[],
                        );
                        inner.tree.next_sibling(cur_ix);
                    }
                    ItemBody::Rule => {
                        builder.add_leaf_full(
                            MdastNodeType::ThematicBreak as u8,
                            start,
                            end,
                            start_line,
                            start_col,
                            end_line,
                            end_col,
                            &[],
                        );
                        inner.tree.next_sibling(cur_ix);
                    }
                    ItemBody::TaskListMarker(checked) => {
                        let checked_val = if checked { 1 } else { 0 };
                        let data = ListItemData {
                            checked: checked_val,
                            spread: false,
                        }
                        .to_bytes();
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
                        inner.tree.next_sibling(cur_ix);
                    }
                    ItemBody::FootnoteReference(cow_ix) => {
                        let cow = inner.allocs.take_cow(cow_ix);
                        let sr = builder.alloc_string(&cow);
                        let data = ReferenceData {
                            identifier: sr,
                            label: sr,
                            reference_kind: 0,
                            _pad: [0; 3],
                        }
                        .to_bytes();
                        builder.add_leaf_full(
                            MdastNodeType::FootnoteReference as u8,
                            start,
                            end,
                            start_line,
                            start_col,
                            end_line,
                            end_col,
                            &data,
                        );
                        inner.tree.next_sibling(cur_ix);
                    }
                    ItemBody::Math(cow_ix, is_display) => {
                        let cow = inner.allocs.take_cow(cow_ix);
                        let sr = builder.alloc_string(&cow);
                        let node_type = if is_display {
                            MdastNodeType::Math
                        } else {
                            MdastNodeType::InlineMath
                        };
                        builder.add_leaf_full(
                            node_type as u8,
                            start,
                            end,
                            start_line,
                            start_col,
                            end_line,
                            end_col,
                            &MathData {
                                meta: StringRef::empty(),
                                value: sr,
                            }
                            .to_bytes(),
                        );
                        inner.tree.next_sibling(cur_ix);
                    }
                    ItemBody::MdxFlowExpression(cow_ix) => {
                        let cow = inner.allocs.take_cow(cow_ix);
                        let sr = builder.alloc_string(&cow);
                        builder.add_leaf_full(
                            MdastNodeType::MdxFlowExpression as u8,
                            start,
                            end,
                            start_line,
                            start_col,
                            end_line,
                            end_col,
                            &ExpressionData { value: sr }.to_bytes(),
                        );
                        inner.tree.next_sibling(cur_ix);
                    }
                    ItemBody::MdxTextExpression(cow_ix) => {
                        let cow = inner.allocs.take_cow(cow_ix);
                        let sr = builder.alloc_string(&cow);
                        builder.add_leaf_full(
                            MdastNodeType::MdxTextExpression as u8,
                            start,
                            end,
                            start_line,
                            start_col,
                            end_line,
                            end_col,
                            &ExpressionData { value: sr }.to_bytes(),
                        );
                        inner.tree.next_sibling(cur_ix);
                    }
                    ItemBody::MdxEsm(cow_ix) => {
                        let cow = inner.allocs.take_cow(cow_ix);
                        let sr = builder.alloc_string(&cow);
                        builder.add_leaf_full(
                            MdastNodeType::MdxjsEsm as u8,
                            start,
                            end,
                            start_line,
                            start_col,
                            end_line,
                            end_col,
                            &ExpressionData { value: sr }.to_bytes(),
                        );
                        inner.tree.next_sibling(cur_ix);
                    }

                    // Unresolved inline markers, should have been resolved by handle_inline.
                    ItemBody::MaybeEmphasis(..)
                    | ItemBody::MaybeMath(..)
                    | ItemBody::MaybeSmartQuote(..)
                    | ItemBody::MaybeCode(..)
                    | ItemBody::MaybeHtml
                    | ItemBody::MaybeLinkOpen
                    | ItemBody::MaybeLinkClose(..)
                    | ItemBody::MaybeImage => {
                        // Treat as text.
                        let sr = StringRef::new(start, end - start);
                        builder.add_leaf_full(
                            MdastNodeType::Text as u8,
                            start,
                            end,
                            start_line,
                            start_col,
                            end_line,
                            end_col,
                            &sr.as_bytes(),
                        );
                        inner.tree.next_sibling(cur_ix);
                    }

                    // Skip these silently.
                    ItemBody::TightParagraph | ItemBody::Root => {
                        inner.tree.push();
                    }

                    // Catch-all for anything unexpected.
                    _ => {
                        inner.tree.next_sibling(cur_ix);
                    }
                }
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

    // Merge parser-level MDX errors.
    if !inner.mdx_errors.is_empty() {
        mdx_errors.extend_from_slice(&inner.mdx_errors);
        mdx_errors.sort_by_key(|(offset, _)| *offset);
    }

    // Close root.
    builder.close_node();
    let mut arena = builder.finish();
    arena.parse_options = options.bits();
    (arena, mdx_errors)
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

use crate::parse::JsxElementData;

fn encode_jsx_element_data(jsx: &JsxElementData<'_>, builder: &mut ArenaBuilder) -> Vec<u8> {
    let name_ref = if jsx.name.is_empty() {
        StringRef::empty()
    } else {
        builder.alloc_string(&jsx.name)
    };

    let attr_tuples: Vec<(u8, StringRef, StringRef)> = jsx
        .attrs
        .iter()
        .map(|attr| match attr {
            JsxAttr::Boolean(n) => {
                let n = builder.alloc_string(n);
                (MDX_ATTR_BOOLEAN_PROP, n, StringRef::empty())
            }
            JsxAttr::Literal(n, v) => {
                let n = builder.alloc_string(n);
                let v = builder.alloc_string(v);
                (MDX_ATTR_LITERAL_PROP, n, v)
            }
            JsxAttr::Expression(n, v) => {
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
