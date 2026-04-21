//! Direct arena builder: walks the pulldown-cmark internal tree and builds
//! a `satteri_arena::Arena` without going through the Event iterator.

use satteri_arena::{decode_string_ref_data, Arena, ArenaBuilder, LineIndex, StringRef};
use satteri_ast::mdast::{
    encode_mdx_jsx_element_data, encode_table_data, CodeData, ColumnAlign, ExpressionData,
    FootnoteDefinitionData, ImageData, LinkData, ListData, ListItemData, MathData, MdastNodeType,
    ReferenceData,
};
use satteri_ast::shared::{
    MDX_ATTR_BOOLEAN_PROP, MDX_ATTR_EXPRESSION_PROP, MDX_ATTR_LITERAL_PROP, MDX_ATTR_SPREAD,
};

use crate::parse::{DefaultParserCallbacks, ItemBody, JsxAttr, ParserInner};
use crate::{Alignment, HeadingLevel, LinkType, Options};

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

                // TightParagraph: close it like a regular paragraph.
                // MDAST spec requires listItem > paragraph > text even for
                // tight lists.

                // Inside an image: skip closing non-Image containers
                // (they were never opened in the MDAST builder).
                if image_alt_buf.is_some()
                    && !matches!(inner.tree[ix].item.body, ItemBody::Image(_))
                {
                    image_depth = image_depth.saturating_sub(1);
                    inner.tree.next_sibling(ix);
                    continue;
                }

                let item = inner.tree[ix].item;
                let mut end = item.end as u32;
                if item.body.is_block_level() {
                    let src = source.as_bytes();
                    while end > item.start as u32
                        && matches!(src.get(end as usize - 1), Some(b'\n' | b'\r'))
                    {
                        end -= 1;
                    }
                }
                let (end_line, end_col) = cursor.offset_to_line_col(end);

                match &inner.tree[ix].item.body {
                    // Code block close: write accumulated content.
                    ItemBody::FencedCodeBlock(_) | ItemBody::IndentCodeBlock => {
                        if let Some(mut content) = code_block_buf.take() {
                            // Match remark / mdast-util-from-markdown: `Code.value`
                            // never carries the trailing `\n` before the closing
                            // fence. mdast-util-to-hast re-adds it at render time;
                            // our mdast→hast converter does the same.
                            if content.ends_with('\n') {
                                content.pop();
                            }
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
                            let raw_end = item.end as u32;
                            let (raw_end_line, raw_end_col) = cursor.offset_to_line_col(raw_end);
                            builder.set_position_current(
                                orig_start,
                                raw_end,
                                orig_start_line,
                                orig_start_col,
                                raw_end_line,
                                raw_end_col,
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
                    ItemBody::ListItem(_) => {
                        let id = builder.current_node_id();
                        let is_spread = {
                            let mut child = inner.tree[ix].child;
                            let mut para_count = 0u32;
                            let mut has_other_block = false;
                            while let Some(c) = child {
                                match inner.tree[c].item.body {
                                    ItemBody::Paragraph | ItemBody::TightParagraph => {
                                        para_count += 1;
                                    }
                                    _ if inner.tree[c].item.body.is_block_level()
                                        && !matches!(inner.tree[c].item.body,
                                            ItemBody::List(..) | ItemBody::BlockQuote(..)
                                            | ItemBody::DefinitionList(..)
                                            | ItemBody::Container(..)
                                            | ItemBody::ListItem(..)
                                        ) =>
                                    {
                                        has_other_block = true;
                                    }
                                    _ => {}
                                }
                                child = inner.tree[c].next;
                            }
                            para_count > 1 || (para_count >= 1 && has_other_block)
                        };
                        if is_spread {
                            let existing = builder.arena_ref().get_type_data(id).to_vec();
                            if existing.len() >= 2 {
                                let mut data = existing;
                                data[1] = 1; // spread = true
                                builder.set_data_current(&data);
                            }
                        }
                        let node = builder.arena_ref().get_node(id);
                        let orig_start = node.start_offset;
                        let orig_start_line = node.start_line;
                        let orig_start_col = node.start_column;
                        let (cont_end, cont_end_line, cont_end_col) =
                            if let Some(last_child) = builder.last_sibling_id() {
                                let lc = builder.arena_ref().get_node(last_child);
                                (lc.end_offset, lc.end_line, lc.end_column)
                            } else {
                                (end, end_line, end_col)
                            };
                        builder.set_position_current(
                            orig_start,
                            cont_end,
                            orig_start_line,
                            orig_start_col,
                            cont_end_line,
                            cont_end_col,
                        );
                        builder.close_node();
                    }
                    ItemBody::List(is_tight, _, _) => {
                        let id = builder.current_node_id();
                        let any_item_spread = {
                            let children = builder.arena_ref().get_children(id).to_vec();
                            children.iter().any(|&child_id| {
                                let cdata = builder.arena_ref().get_type_data(child_id);
                                if cdata.len() >= 2 {
                                    cdata[1] != 0
                                } else {
                                    false
                                }
                            })
                        };
                        if !is_tight || any_item_spread {
                            let existing = builder.arena_ref().get_type_data(id).to_vec();
                            if existing.len() >= 8 && existing[5] == 0 {
                                let mut data = existing;
                                data[5] = 1;
                                builder.set_data_current(&data);
                            }
                        }
                        let id = builder.current_node_id();
                        let node = builder.arena_ref().get_node(id);
                        let orig_start = node.start_offset;
                        let orig_start_line = node.start_line;
                        let orig_start_col = node.start_column;
                        let (cont_end, cont_end_line, cont_end_col) =
                            if let Some(last_child) = builder.last_sibling_id() {
                                let lc = builder.arena_ref().get_node(last_child);
                                (lc.end_offset, lc.end_line, lc.end_column)
                            } else {
                                (end, end_line, end_col)
                            };
                        builder.set_position_current(
                            orig_start,
                            cont_end,
                            orig_start_line,
                            orig_start_col,
                            cont_end_line,
                            cont_end_col,
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
                        let use_last_child = matches!(
                            item.body,
                            ItemBody::BlockQuote(..)
                                | ItemBody::Container(..)
                        );
                        let (cont_end, cont_end_line, cont_end_col) = if use_last_child {
                            if let Some(last_child) = builder.last_sibling_id() {
                                let lc = builder.arena_ref().get_node(last_child);
                                (lc.end_offset, lc.end_line, lc.end_column)
                            } else {
                                (end, end_line, end_col)
                            }
                        } else {
                            (end, end_line, end_col)
                        };
                        builder.set_position_current(
                            orig_start,
                            cont_end,
                            orig_start_line,
                            orig_start_col,
                            cont_end_line,
                            cont_end_col,
                        );
                        builder.close_node();
                    }
                }

                inner.tree.next_sibling(ix);
            }
            Some(cur_ix) => {
                // TightParagraph: emit as a regular paragraph node.
                // MDAST spec requires listItem > paragraph > text even for
                // tight lists.

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
                    match &item.body {
                        ItemBody::Text { .. } => {
                            buf.push_str(&source[item.start..item.end]);
                            inner.tree.next_sibling(cur_ix);
                            continue;
                        }
                        ItemBody::SynthesizeText(cow_ix) => {
                            let cow = inner.allocs.take_cow(*cow_ix);
                            buf.push_str(&cow);
                            inner.tree.next_sibling(cur_ix);
                            continue;
                        }
                        _ => {}
                    }
                }

                // Inside an image: accumulate alt text but skip MDAST
                // node emission. Image is a void node in the MDAST spec.
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
                            inner.tree.push();
                            continue;
                        }
                        _ => {
                            if inner.tree[cur_ix].child.is_some() {
                                image_depth += 1;
                                inner.tree.push();
                                continue;
                            }
                        }
                    }
                    // Leaf node: advance past it.
                    inner.tree.next_sibling(cur_ix);
                    continue;
                }

                // Map ItemBody to arena node.
                match item.body {
                    ItemBody::Paragraph | ItemBody::TightParagraph => {
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
                        let (link_type, dest_url, title, _id) = inner.allocs.take_link(link_ix);
                        let url_ref = if matches!(link_type, LinkType::Email) {
                            let mailto = format!("mailto:{}", &*dest_url);
                            builder.alloc_string(&mailto)
                        } else {
                            builder.alloc_string(&dest_url)
                        };
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

                    ItemBody::Text { backslash_escaped } => {
                        let text_value: &str = &source[item.start..item.end];

                        // Merge with previous sibling text node when
                        // adjacent or separated by a gap (backslash escape).
                        let prev_id = builder.last_sibling_id();
                        let merged = if let Some(pid) = prev_id {
                            let prev = builder.arena_ref().get_node(pid);
                            if prev.node_type == MdastNodeType::Text as u8 {
                                let prev_data = builder.arena_ref().get_type_data(pid);
                                if prev_data.len() >= 8 {
                                    let prev_sr = StringRef::from_bytes(prev_data);
                                    let prev_text = builder.arena_ref().get_str(prev_sr);
                                    let combined = [prev_text, text_value].concat();
                                    let new_sr = builder.alloc_string(&combined);
                                    let pn = builder.arena_ref().get_node(pid);
                                    builder.update_leaf_full(
                                        pid,
                                        pn.start_offset,
                                        end,
                                        pn.start_line,
                                        pn.start_column,
                                        end_line,
                                        end_col,
                                        &new_sr.as_bytes(),
                                    );
                                    true
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        };
                        if !merged {
                            let (sr, pos_start, pos_start_col) = if backslash_escaped && start > 0 {
                                (
                                    builder.alloc_string(text_value),
                                    start - 1,
                                    start_col.saturating_sub(1),
                                )
                            } else {
                                (StringRef::new(start, end - start), start, start_col)
                            };
                            let pos_start_line = if backslash_escaped && start > 0 {
                                cursor.offset_to_line_col(start - 1).0
                            } else {
                                start_line
                            };
                            builder.add_leaf_full(
                                MdastNodeType::Text as u8,
                                pos_start,
                                end,
                                pos_start_line,
                                pos_start_col,
                                end_line,
                                end_col,
                                &sr.as_bytes(),
                            );
                        }
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
                        let prev_id = builder.last_sibling_id();
                        let merged = if let Some(pid) = prev_id {
                            let prev = builder.arena_ref().get_node(pid);
                            if prev.node_type == MdastNodeType::Text as u8 {
                                let prev_data = builder.arena_ref().get_type_data(pid);
                                if prev_data.len() >= 8 {
                                    let prev_sr = StringRef::from_bytes(prev_data);
                                    let prev_text = builder.arena_ref().get_str(prev_sr);
                                    let combined = [prev_text, "\n"].concat();
                                    let new_sr = builder.alloc_string(&combined);
                                    let pn = builder.arena_ref().get_node(pid);
                                    builder.update_leaf_full(
                                        pid,
                                        pn.start_offset,
                                        end,
                                        pn.start_line,
                                        pn.start_column,
                                        end_line,
                                        end_col,
                                        &new_sr.as_bytes(),
                                    );
                                    true
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        };
                        if !merged {
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
                        }
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
                        let mut rule_end = end;
                        let src = source.as_bytes();
                        while rule_end > start
                            && matches!(src.get(rule_end as usize - 1), Some(b'\n' | b'\r'))
                        {
                            rule_end -= 1;
                        }
                        let (rule_end_line, rule_end_col) = cursor.offset_to_line_col(rule_end);
                        builder.add_leaf_full(
                            MdastNodeType::ThematicBreak as u8,
                            start,
                            rule_end,
                            start_line,
                            start_col,
                            rule_end_line,
                            rule_end_col,
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
                        let text_value: &str = &source[item.start..item.end];
                        let prev_id = builder.last_sibling_id();
                        let merged = if let Some(pid) = prev_id {
                            let prev = builder.arena_ref().get_node(pid);
                            if prev.node_type == MdastNodeType::Text as u8 {
                                let prev_data = builder.arena_ref().get_type_data(pid);
                                if prev_data.len() >= 8 {
                                    let prev_sr = StringRef::from_bytes(prev_data);
                                    let prev_text = builder.arena_ref().get_str(prev_sr);
                                    let combined = [prev_text, text_value].concat();
                                    let new_sr = builder.alloc_string(&combined);
                                    let pn = builder.arena_ref().get_node(pid);
                                    builder.update_leaf_full(
                                        pid,
                                        pn.start_offset,
                                        end,
                                        pn.start_line,
                                        pn.start_column,
                                        end_line,
                                        end_col,
                                        &new_sr.as_bytes(),
                                    );
                                    true
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        };
                        if !merged {
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
                        }
                        inner.tree.next_sibling(cur_ix);
                    }

                    // Skip these silently.
                    ItemBody::Root => {
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

    if options.contains(Options::ENABLE_MDX) {
        mdx_mark_and_unravel(&mut arena);
    }

    (arena, mdx_errors)
}

fn mdx_mark_and_unravel(arena: &mut Arena) {
    let len = arena.len() as u32;
    for id in 0..len {
        let node = arena.get_node(id);
        if node.node_type != MdastNodeType::Paragraph as u8 {
            continue;
        }
        let parent_type = MdastNodeType::from_u8(arena.get_node(node.parent).node_type);
        if matches!(parent_type, Some(MdastNodeType::MdxJsxFlowElement | MdastNodeType::MdxJsxTextElement)) {
            continue;
        }
        let children = arena.get_children(id).to_vec();
        if children.is_empty() {
            continue;
        }
        let mut all_mdx = true;
        let mut has_mdx = false;
        for &child_id in &children {
            let child = arena.get_node(child_id);
            match MdastNodeType::from_u8(child.node_type) {
                Some(MdastNodeType::MdxJsxTextElement | MdastNodeType::MdxTextExpression) => {
                    has_mdx = true;
                }
                Some(MdastNodeType::Text) => {
                    let data = arena.get_type_data(child_id);
                    if !data.is_empty() {
                            let sr = decode_string_ref_data(data);
                        let text = arena.get_str(sr);
                        if !text.chars().all(|c| c.is_ascii_whitespace()) {
                            all_mdx = false;
                            break;
                        }
                    }
                }
                _ => {
                    all_mdx = false;
                    break;
                }
            }
        }
        if !all_mdx || !has_mdx {
            continue;
        }
        let mut promoted: Vec<u32> = Vec::new();
        for &child_id in &children {
            let child = arena.get_node(child_id);
            match MdastNodeType::from_u8(child.node_type) {
                Some(MdastNodeType::MdxJsxTextElement) => {
                    arena.get_node_mut(child_id).node_type = MdastNodeType::MdxJsxFlowElement as u8;
                    promoted.push(child_id);
                }
                Some(MdastNodeType::MdxTextExpression) => {
                    arena.get_node_mut(child_id).node_type = MdastNodeType::MdxFlowExpression as u8;
                    promoted.push(child_id);
                }
                Some(MdastNodeType::Text) => {
                    let data = arena.get_type_data(child_id);
                    if !data.is_empty() {
                            let sr = decode_string_ref_data(data);
                        let text = arena.get_str(sr);
                        if !text.chars().all(|c| c.is_ascii_whitespace()) {
                            promoted.push(child_id);
                        }
                    }
                }
                _ => {
                    promoted.push(child_id);
                }
            }
        }
        arena.replace_node_with_children(id, &promoted);
    }
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
