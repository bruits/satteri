//! Direct arena builder: walks the pulldown-cmark internal tree and builds
//! a `satteri_arena::Arena` without going through the Event iterator.

use satteri_arena::{decode_string_ref_data, Arena, ArenaBuilder, LineIndex, StringRef};
use satteri_ast::mdast::{
    encode_directive_data, encode_image_reference_data, encode_mdx_jsx_element_data,
    encode_reference_data, encode_table_data, CodeData, ColumnAlign, DefinitionData,
    ExpressionData, FootnoteDefinitionData, ImageData, LinkData, ListData, ListItemData, MathData,
    MdastNodeType, ReferenceData,
};
use satteri_ast::shared::{
    MDX_ATTR_BOOLEAN_PROP, MDX_ATTR_EXPRESSION_PROP, MDX_ATTR_LITERAL_PROP, MDX_ATTR_SPREAD,
};

use crate::parse::{DefaultParserCallbacks, ItemBody, JsxAttr, ParserInner};
use crate::{Alignment, HeadingLevel, LinkType, Options};

/// Default options: GFM (tables, strikethrough, task lists, autolink-literal),
/// footnotes, math, YAML metadata.
/// Note: heading attributes (`# Title {#id .cls}`) are intentionally NOT enabled
/// here — remark doesn't parse them and stripping them breaks conformance for
/// headings that incidentally contain `{…}` text.
pub const DEFAULT_OPTIONS: Options = Options::from_bits_truncate(
    Options::ENABLE_GFM.bits()
        | Options::ENABLE_TABLES.bits()
        | Options::ENABLE_FOOTNOTES.bits()
        | Options::ENABLE_STRIKETHROUGH.bits()
        | Options::ENABLE_TASKLISTS.bits()
        | Options::ENABLE_MATH.bits()
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
    // ENABLE_GFM is the umbrella flag for the GitHub Flavored Markdown
    // feature set. Expand it into the granular flags the parser checks so
    // callers don't have to remember which sub-flags GFM implies.
    let options = if options.contains(Options::ENABLE_GFM) {
        options | Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS
    } else {
        options
    };

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
                    // Math block close: write accumulated content.
                    ItemBody::MathBlock(_) => {
                        if let Some(mut content) = code_block_buf.take() {
                            // Drop the trailing line terminator (remark keeps
                            // CRLF inside the value but strips the one right
                            // before the closing fence).
                            if content.ends_with("\r\n") {
                                content.truncate(content.len() - 2);
                            } else if content.ends_with('\n') || content.ends_with('\r') {
                                content.pop();
                            }
                            let meta_str = match &inner.tree[ix].item.body {
                                ItemBody::MathBlock(cow_ix) => {
                                    let cow = inner.allocs.take_cow(*cow_ix);
                                    cow.to_string()
                                }
                                _ => String::new(),
                            };
                            let meta_ref = if meta_str.is_empty() {
                                StringRef::empty()
                            } else {
                                builder.alloc_string(&meta_str)
                            };
                            let value_ref = builder.alloc_string(&content);

                            builder.set_data_current(
                                &MathData {
                                    meta: meta_ref,
                                    value: value_ref,
                                }
                                .to_bytes(),
                            );
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
                    // Code block close: write accumulated content.
                    ItemBody::FencedCodeBlock(_) | ItemBody::IndentCodeBlock => {
                        if let Some(mut content) = code_block_buf.take() {
                            // Drop the trailing line terminator (remark keeps
                            // CRLF inside the value but strips the one right
                            // before the closing fence).
                            if content.ends_with("\r\n") {
                                content.truncate(content.len() - 2);
                            } else if content.ends_with('\n') || content.ends_with('\r') {
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
                            let mut code_end = end;
                            let mut code_end_line = end_line;
                            let mut code_end_col = end_col;
                            if matches!(inner.tree[ix].item.body, ItemBody::IndentCodeBlock) {
                                let src = source.as_bytes();
                                let mut pos = code_end as usize;
                                while pos < src.len() {
                                    if src[pos] == b'\r' {
                                        pos += 1;
                                    }
                                    if pos < src.len() && src[pos] == b'\n' {
                                        pos += 1;
                                    }
                                    if pos >= src.len() {
                                        break;
                                    }
                                    let line_start = pos;
                                    let is_indented = matches!(src.get(pos), Some(b'\t'))
                                        || src[pos..].starts_with(b"    ");
                                    if !is_indented {
                                        break;
                                    }
                                    while pos < src.len() && src[pos] != b'\n' && src[pos] != b'\r'
                                    {
                                        pos += 1;
                                    }
                                    let line_content = &src[line_start..pos];
                                    let all_ws =
                                        line_content.iter().all(|&b| b == b' ' || b == b'\t');
                                    if !all_ws {
                                        break;
                                    }
                                    code_end = pos as u32;
                                    let (el, ec) = cursor.offset_to_line_col(code_end);
                                    code_end_line = el;
                                    code_end_col = ec;
                                }
                            }
                            let node = builder.arena_ref().get_node(id);
                            let orig_start = node.start_offset;
                            let orig_start_line = node.start_line;
                            let orig_start_col = node.start_column;
                            builder.set_position_current(
                                orig_start,
                                code_end,
                                orig_start_line,
                                orig_start_col,
                                code_end_line,
                                code_end_col,
                            );
                            builder.close_node();
                        }
                    }
                    // HTML block close: write accumulated content.
                    ItemBody::HtmlBlock(is_type_6_or_7) => {
                        if let Some(content) = html_block_buf.take() {
                            let at_eof = item.end >= source.len();
                            let trimmed = if *is_type_6_or_7 || !at_eof {
                                content.trim_end_matches('\n')
                            } else {
                                content.as_str()
                            };
                            let sr = builder.alloc_string(trimmed);
                            let id = builder.current_node_id();
                            let node = builder.arena_ref().get_node(id);
                            let orig_start = node.start_offset;
                            let orig_start_line = node.start_line;
                            let orig_start_col = node.start_column;
                            let trimmed_len = content.len() - trimmed.len();
                            let raw_end = (item.end as u32).saturating_sub(trimmed_len as u32);
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
                    // Metadata block close: strip exactly one trailing line
                    // terminator (the newline before the closing fence).
                    // Any earlier blank line inside the block is content and
                    // must be preserved — matching remark-frontmatter.
                    ItemBody::MetadataBlock(_) => {
                        if let Some(mut content) = html_block_buf.take() {
                            if content.ends_with("\r\n") {
                                content.truncate(content.len() - 2);
                            } else if content.ends_with('\n') || content.ends_with('\r') {
                                content.pop();
                            }
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
                            inner.tree.next_sibling(ix);
                            continue;
                        }
                        if let Some(alt_text) = image_alt_buf.take() {
                            let alt_ref = builder.alloc_string(&alt_text);
                            let id = builder.current_node_id();
                            let node_type = builder.arena_ref().get_node(id).node_type;
                            let existing_data = builder.arena_ref().get_type_data(id).to_vec();
                            let is_image_ref = node_type == MdastNodeType::ImageReference as u8;
                            if is_image_ref && existing_data.len() >= 28 {
                                let mut data = existing_data;
                                data[20..28].copy_from_slice(&alt_ref.as_bytes());
                                builder.set_data_current(&data);
                            } else if !is_image_ref && existing_data.len() >= 24 {
                                let mut data = existing_data;
                                data[8..16].copy_from_slice(&alt_ref.as_bytes());
                                builder.set_data_current(&data);
                            }
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
                    }
                    ItemBody::ListItem(_, item_spread) => {
                        let id = builder.current_node_id();
                        let is_spread = *item_spread || {
                            let mut found = false;
                            let mut prev_end_line: Option<u32> = None;
                            for &child_id in builder.current_pending_children() {
                                let child_node = builder.arena_ref().get_node(child_id);
                                if let Some(pel) = prev_end_line {
                                    if child_node.start_line > pel + 1 {
                                        found = true;
                                        break;
                                    }
                                }
                                prev_end_line = Some(child_node.end_line);
                            }
                            found
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
                                let src = source.as_bytes();
                                let start_usize = orig_start as usize;
                                let end_usize = end as usize;
                                let first_nl = src[start_usize..end_usize]
                                    .iter()
                                    .position(|&b| b == b'\n' || b == b'\r')
                                    .map(|p| (start_usize + p) as u32)
                                    .unwrap_or(end);
                                let (el, ec) = cursor.offset_to_line_col(first_nl);
                                (first_nl, el, ec)
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
                    ItemBody::List(_is_tight, _, _) => {
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
                        let children = builder.arena_ref().get_children(id).to_vec();
                        let has_blank_between_items = {
                            let mut found = false;
                            let mut prev_end_line: Option<u32> = None;
                            for &child_id in &children {
                                let child_node = builder.arena_ref().get_node(child_id);
                                if let Some(pel) = prev_end_line {
                                    if child_node.start_line > pel + 1 {
                                        found = true;
                                        break;
                                    }
                                }
                                prev_end_line = Some(child_node.end_line);
                            }
                            found
                        };
                        if has_blank_between_items {
                            let existing = builder.arena_ref().get_type_data(id).to_vec();
                            if existing.len() >= 8 && existing[5] == 0 {
                                let mut data = existing;
                                data[5] = 1;
                                builder.arena_mut().set_type_data(id, &data);
                            }
                        }
                        // Already closed above; skip the common close_node path.
                        inner.tree.next_sibling(ix);
                        continue;
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
                            ItemBody::BlockQuote(..) | ItemBody::ContainerDirective(..)
                        );
                        let (cont_end, cont_end_line, cont_end_col) = if use_last_child {
                            if let Some(last_child) = builder.last_sibling_id() {
                                let lc = builder.arena_ref().get_node(last_child);
                                if lc.end_offset >= end {
                                    (lc.end_offset, lc.end_line, lc.end_column)
                                } else {
                                    (end, end_line, end_col)
                                }
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
                            // Remark preserves the newline in alt text rather
                            // than collapsing it to a space.
                            buf.push('\n');
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
                    ItemBody::MathBlock(_) => {
                        builder.open_node(MdastNodeType::Math as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        code_block_buf = Some(String::with_capacity(256));
                        inner.tree.push();
                    }
                    ItemBody::FencedCodeBlock(cow_ix) => {
                        let info_cow = inner.allocs.take_cow(cow_ix);
                        let info_str = info_cow.as_ref();
                        let (lang_str, meta_str) = match info_str.split_once(char::is_whitespace) {
                            // Keep trailing whitespace in meta to match remark/mdast.
                            Some((l, m)) => (l, m.trim_start()),
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
                    ItemBody::List(_is_tight, c, listitem_start) => {
                        let ordered = c == b'.' || c == b')';
                        let start_num = if ordered { listitem_start as u32 } else { 0 };
                        builder.open_node(MdastNodeType::List as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        let ld = ListData {
                            start: start_num,
                            ordered,
                            spread: false,
                            _pad: [0; 2],
                        };
                        builder.set_data_current(&ld.to_bytes());
                        inner.tree.push();
                    }
                    ItemBody::ListItem(_, spread) => {
                        builder.open_node(MdastNodeType::ListItem as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        builder.set_data_current(&ListItemData { checked: 2, spread }.to_bytes());
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
                        let (link_type, dest_url, title, id) = inner.allocs.take_link(link_ix);
                        if let Some(kind) = reference_kind(link_type) {
                            let (ref_end, ref_end_line, ref_end_col) =
                                reference_end(source, &mut cursor, end, kind);
                            let label_src =
                                extract_reference_label(source, start, ref_end, kind, false);
                            let label_ref = builder.alloc_string(label_src);
                            let identifier_ref = builder.alloc_string(&normalize_identifier(&id));
                            builder.open_node(MdastNodeType::LinkReference as u8);
                            builder.set_position_current(
                                start,
                                ref_end,
                                start_line,
                                start_col,
                                ref_end_line,
                                ref_end_col,
                            );
                            builder.set_data_current(&encode_reference_data(
                                identifier_ref,
                                label_ref,
                                kind,
                            ));
                            // The generic container-close path reads
                            // `item.end` and overrides the position we just
                            // set. For Collapsed references we need the span
                            // to cover the trailing `[]`, so sync the tree
                            // node's end up front.
                            inner.tree[cur_ix].item.end = ref_end as usize;
                        } else {
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
                        }
                        inner.tree.push();
                    }
                    ItemBody::Image(link_ix) => {
                        let (link_type, dest_url, title, id) = inner.allocs.take_link(link_ix);
                        if let Some(kind) = reference_kind(link_type) {
                            let (ref_end, ref_end_line, ref_end_col) =
                                reference_end(source, &mut cursor, end, kind);
                            let label_src =
                                extract_reference_label(source, start, ref_end, kind, true);
                            let label_ref = builder.alloc_string(label_src);
                            let identifier_ref = builder.alloc_string(&normalize_identifier(&id));
                            builder.open_node(MdastNodeType::ImageReference as u8);
                            builder.set_position_current(
                                start,
                                ref_end,
                                start_line,
                                start_col,
                                ref_end_line,
                                ref_end_col,
                            );
                            builder.set_data_current(&encode_image_reference_data(
                                identifier_ref,
                                label_ref,
                                kind,
                                StringRef::empty(),
                            ));
                            // Same fix as LinkReference: the generic close
                            // path reads `item.end`, which ignores the
                            // trailing `[]` for Collapsed refs. Sync it.
                            inner.tree[cur_ix].item.end = ref_end as usize;
                        } else {
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
                        }
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
                    ItemBody::HtmlBlock(_) => {
                        builder.open_node(MdastNodeType::Html as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        html_block_buf = Some(String::with_capacity(128));
                        inner.tree.push();
                    }
                    ItemBody::MetadataBlock(kind) => {
                        let node_type = match kind {
                            crate::MetadataBlockKind::YamlStyle => MdastNodeType::Yaml,
                            crate::MetadataBlockKind::PlusesStyle => MdastNodeType::Toml,
                        };
                        builder.open_node(node_type as u8);
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
                    ItemBody::ContainerDirective(_, dir_ix) => {
                        let dir = inner.allocs.directive_ref(dir_ix);
                        let name_sr = builder.alloc_string(&dir.name);
                        let attr_pairs: Vec<(StringRef, StringRef)> = dir
                            .attributes
                            .iter()
                            .map(|(k, v)| (builder.alloc_string(k), builder.alloc_string(v)))
                            .collect();
                        let type_data = encode_directive_data(name_sr, &attr_pairs);
                        // `label_start == label_end == 0` means no brackets at
                        // all; any other state (including `[]`) means brackets
                        // were present and remark emits an (possibly empty)
                        // directive-label paragraph.
                        let brackets_present = dir.label_start != 0 || dir.label_end != 0;
                        let has_label_content = dir.label_start < dir.label_end;
                        let label_text = if has_label_content {
                            Some(source[dir.label_start..dir.label_end].to_string())
                        } else {
                            None
                        };
                        builder.open_node(MdastNodeType::ContainerDirective as u8);
                        builder.set_data_current(&type_data);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        if brackets_present {
                            let bracket_offset = dir.label_start.saturating_sub(1) as u32;
                            let bracket_end = (dir.label_end + 1) as u32;
                            let (para_start_line, para_start_col) =
                                cursor.offset_to_line_col(bracket_offset);
                            let (para_end_line, para_end_col) =
                                cursor.offset_to_line_col(bracket_end);
                            builder.open_node(MdastNodeType::Paragraph as u8);
                            builder.set_position_current(
                                bracket_offset,
                                bracket_end,
                                para_start_line,
                                para_start_col,
                                para_end_line,
                                para_end_col,
                            );
                            let para_id = builder.current_node_id();
                            builder
                                .arena_mut()
                                .set_node_data(para_id, b"{\"directiveLabel\":true}".to_vec());
                            if let Some(label) = label_text {
                                let label_sr = builder.alloc_string(&label);
                                let (text_start_line, text_start_col) =
                                    cursor.offset_to_line_col(dir.label_start as u32);
                                let (text_end_line, text_end_col) =
                                    cursor.offset_to_line_col(dir.label_end as u32);
                                builder.add_leaf_full(
                                    MdastNodeType::Text as u8,
                                    dir.label_start as u32,
                                    dir.label_end as u32,
                                    text_start_line,
                                    text_start_col,
                                    text_end_line,
                                    text_end_col,
                                    &label_sr.as_bytes(),
                                );
                            }
                            builder.close_node();
                        }
                        inner.tree.push();
                    }
                    ItemBody::LeafDirective(dir_ix) => {
                        let dir = inner.allocs.directive_ref(dir_ix);
                        let name_sr = builder.alloc_string(&dir.name);
                        let attr_pairs: Vec<(StringRef, StringRef)> = dir
                            .attributes
                            .iter()
                            .map(|(k, v)| (builder.alloc_string(k), builder.alloc_string(v)))
                            .collect();
                        let type_data = encode_directive_data(name_sr, &attr_pairs);
                        let has_label = dir.label_start < dir.label_end;
                        builder.open_node(MdastNodeType::LeafDirective as u8);
                        builder.set_data_current(&type_data);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        if has_label {
                            let label = &source[dir.label_start..dir.label_end];
                            let label_sr = builder.alloc_string(label);
                            let (ls_line, ls_col) =
                                cursor.offset_to_line_col(dir.label_start as u32);
                            let (le_line, le_col) = cursor.offset_to_line_col(dir.label_end as u32);
                            builder.add_leaf_full(
                                MdastNodeType::Text as u8,
                                dir.label_start as u32,
                                dir.label_end as u32,
                                ls_line,
                                ls_col,
                                le_line,
                                le_col,
                                &label_sr.as_bytes(),
                            );
                        }
                        builder.close_node();
                        inner.tree.next_sibling(cur_ix);
                        continue;
                    }

                    ItemBody::TextDirective(dir_ix) => {
                        let dir = inner.allocs.directive_ref(dir_ix);
                        let name_sr = builder.alloc_string(&dir.name);
                        let attr_pairs: Vec<(StringRef, StringRef)> = dir
                            .attributes
                            .iter()
                            .map(|(k, v)| (builder.alloc_string(k), builder.alloc_string(v)))
                            .collect();
                        let type_data = encode_directive_data(name_sr, &attr_pairs);
                        let has_label = dir.label_start < dir.label_end;
                        builder.open_node(MdastNodeType::TextDirective as u8);
                        builder.set_data_current(&type_data);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        if has_label {
                            let label = &source[dir.label_start..dir.label_end];
                            let label_sr = builder.alloc_string(label);
                            let (ls_line, ls_col) =
                                cursor.offset_to_line_col(dir.label_start as u32);
                            let (le_line, le_col) = cursor.offset_to_line_col(dir.label_end as u32);
                            builder.add_leaf_full(
                                MdastNodeType::Text as u8,
                                dir.label_start as u32,
                                dir.label_end as u32,
                                ls_line,
                                ls_col,
                                le_line,
                                le_col,
                                &label_sr.as_bytes(),
                            );
                        }
                        builder.close_node();
                        inner.tree.next_sibling(cur_ix);
                        continue;
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
                        emit_text_merging(
                            &mut builder,
                            &cow,
                            start,
                            end,
                            start_line,
                            start_col,
                            end_line,
                            end_col,
                        );
                        inner.tree.next_sibling(cur_ix);
                    }
                    ItemBody::SynthesizeChar(c) => {
                        let s = String::from(c);
                        emit_text_merging(
                            &mut builder,
                            &s,
                            start,
                            end,
                            start_line,
                            start_col,
                            end_line,
                            end_col,
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
                        let src_bytes = source.as_bytes();
                        let break_text = {
                            let span = &src_bytes[item.start..item.end];
                            let has_cr = span.contains(&b'\r');
                            let has_lf = span.contains(&b'\n');
                            if has_cr && has_lf {
                                "\r\n"
                            } else if has_cr {
                                "\r"
                            } else {
                                "\n"
                            }
                        };
                        let prev_id = builder.last_sibling_id();
                        let merged = if let Some(pid) = prev_id {
                            let prev = builder.arena_ref().get_node(pid);
                            if prev.node_type == MdastNodeType::Text as u8 {
                                let prev_data = builder.arena_ref().get_type_data(pid);
                                if prev_data.len() >= 8 {
                                    let prev_sr = StringRef::from_bytes(prev_data);
                                    let prev_text = builder.arena_ref().get_str(prev_sr);
                                    let combined = [prev_text, break_text].concat();
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
                            let sr = builder.alloc_string(break_text);
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
                        let depth = builder.stack_depth();
                        for i in (0..depth).rev() {
                            if let Some(node_id) = builder.stack_node_id(i) {
                                if builder.arena_ref().get_node(node_id).node_type
                                    == MdastNodeType::ListItem as u8
                                {
                                    let prev = builder.arena_ref().get_type_data(node_id).to_vec();
                                    let prev_spread = prev.get(1).copied().unwrap_or(0) != 0;
                                    let data = ListItemData {
                                        checked: checked_val,
                                        spread: prev_spread,
                                    }
                                    .to_bytes();
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

    // Emit `definition` nodes for every refdef pulldown-cmark consumed during
    // parsing. They're collected by label in `allocs.refdefs` and we splice
    // them back into the root in source order to match remark's mdast.
    //
    // NB: pulldown-cmark doesn't track the containing block, so a definition
    // that lived inside e.g. a blockquote will land at root here. Plain
    // root-level definitions — the overwhelmingly common case — round-trip
    // exactly.
    let mut refdefs: Vec<(String, String, Option<String>, core::ops::Range<usize>)> = inner
        .allocs
        .refdefs
        .0
        .iter()
        .map(|(label, def)| {
            (
                label.as_ref().to_string(),
                def.dest.to_string(),
                def.title.as_ref().map(|t| t.to_string()),
                def.span.clone(),
            )
        })
        .collect();
    refdefs.sort_by_key(|(_, _, _, span)| span.start);
    for (label, dest, title, span) in refdefs {
        let start = span.start as u32;
        let end = span.end as u32;
        let (sl, sc) = cursor.offset_to_line_col(start);
        let (el, ec) = cursor.offset_to_line_col(end);
        let url_ref = builder.alloc_string(&dest);
        let title_ref = match &title {
            Some(t) => builder.alloc_string(t),
            None => StringRef::empty(),
        };
        let label_ref = builder.alloc_string(&label);
        let identifier_ref = builder.alloc_string(&normalize_identifier(&label));
        let data = DefinitionData {
            url: url_ref,
            title: title_ref,
            identifier: identifier_ref,
            label: label_ref,
        }
        .to_bytes();
        builder.add_leaf_full(
            MdastNodeType::Definition as u8,
            start,
            end,
            sl,
            sc,
            el,
            ec,
            &data,
        );
    }
    if !inner.allocs.refdefs.0.is_empty() {
        builder.sort_current_pending_children_by_start_offset();
    }

    // Close root.
    builder.close_node();
    let mut arena = builder.finish();
    arena.parse_options = options.bits();

    if options.contains(Options::ENABLE_MDX) {
        mdx_mark_and_unravel(&mut arena);
    }

    // GFM extension: promote bare URLs (http://…, https://…, www.…) inside
    // Text nodes to `link` nodes. Matches remark-gfm / mdast-util-gfm-autolink-literal.
    if options.contains(Options::ENABLE_GFM) {
        // When directives are also on, a URL with a port (`http://host:4321`)
        // gets split by the directive parser into
        // `text("…http://host") + textDirective("4321") + text("/…")`.
        // remark handles this cleanly because its autolink tokenizer runs
        // before the directive check; we're a post-pass, so we re-merge the
        // split first and then run the autolink scan.
        if options.contains(Options::ENABLE_CONTAINER_EXTENSIONS) {
            merge_directive_port_splits(&mut arena);
        }
        gfm_autolink_literal_pass(&mut arena);
    }

    if options.contains(Options::ENABLE_CONTAINER_EXTENSIONS) {
        // Directive labels are stored as a single Text node today. Remark
        // inline-parses them so constructs like `` `code` `` inside
        // `:::tip[Set a \`baseUrl\`]` end up as inlineCode. We mirror the
        // common case (backticks) here as a post-pass.
        directive_label_inline_code_pass(&mut arena);
        if options.contains(Options::ENABLE_MDX) {
            // Same idea for JSX tags inside a directive label —
            // `:::note[The <code>x</code> property]`.
            directive_label_jsx_pass(&mut arena);
        }
    }

    (arena, mdx_errors)
}

/// Scan a text buffer for a GFM autolink literal starting at `ix`.
/// Returns `(url_start, url_end, url_string)` when a literal matches.
///
/// A match starts at one of `http://`, `https://`, `ftp://`, `www.` and
/// extends through non-whitespace characters, with trailing punctuation
/// trimmed per the GFM spec.
/// Returns (start, raw_end, end, url) where `raw_end` is the position before
/// trailing punctuation was trimmed back, and `end` is the final URL boundary.
/// Callers that care about the trimmed-back tail (e.g. remark's text-node
/// split when the surrounding context has an unclosed `[`) need `raw_end`.
fn scan_autolink_literal(bytes: &[u8], ix: usize) -> Option<(usize, usize, usize, String)> {
    // Scheme. remark-gfm's autolink-literal extension handles http(s) and
    // `www.`, but not ftp — so we match that set exactly.
    let (proto_len, is_www) = if bytes[ix..].starts_with(b"http://") {
        (7, false)
    } else if bytes[ix..].starts_with(b"https://") {
        (8, false)
    } else if bytes[ix..].starts_with(b"www.") {
        (4, true)
    } else {
        return None;
    };

    // Preceding-character rule. `mdast-util-gfm-autolink-literal`'s transform
    // — which this post-pass mirrors — requires start-of-input, whitespace, or
    // unicode punctuation before the match. So `"http://…` (quote) is valid
    // but `abchttp://…` (letter) is not.
    if ix > 0 {
        let prev = bytes[ix - 1];
        let ok = if prev < 0x80 {
            prev.is_ascii_whitespace() || prev.is_ascii_punctuation()
        } else {
            // Non-ASCII: treat as "not-letter" ⇒ accept (matches remark's
            // unicodeWhitespace || unicodePunctuation for the common cases we
            // see — a full Unicode category check would be overkill here).
            match core::str::from_utf8(&bytes[ix.saturating_sub(4)..ix]) {
                Ok(s) => {
                    let c = s.chars().last().unwrap_or(' ');
                    c.is_whitespace() || !c.is_alphanumeric()
                }
                Err(_) => true,
            }
        };
        if !ok {
            return None;
        }
    }

    // Collect the URL body: everything until whitespace, `<`, ASCII control, or end.
    // Per GFM, valid URLs exclude control characters; matching remark's behavior
    // here avoids autolinking e.g. `http://\x07>` inside a broken `<...>`.
    let mut end = ix + proto_len;
    while end < bytes.len() {
        let b = bytes[end];
        if b <= b' ' || b == 0x7F || b == b'<' {
            break;
        }
        end += 1;
    }

    // Must have at least one char past the scheme.
    if end == ix + proto_len {
        return None;
    }

    // The GFM spec allows `.`, but a `www.` match must have a valid domain
    // (one more `.`-separated segment beyond `www.`). Reject `www.` alone.
    if is_www {
        let rest = &bytes[ix + proto_len..end];
        if rest.is_empty() {
            return None;
        }
    }

    let raw_end = end;

    // Trim trailing punctuation. Set mirrors micromark-gfm-autolink-literal's
    // trail tokenizer: `!"'*,.:;<?]_~` plus unbalanced `)` plus `&;`-
    // terminated entities. Interleaved so that e.g. trailing `")` is fully
    // stripped (`)` via balance, then `"` via the punctuation set).
    loop {
        if end <= ix + proto_len {
            break;
        }
        let last = bytes[end - 1];
        if matches!(
            last,
            b'!' | b'"'
                | b'\''
                | b'*'
                | b','
                | b'.'
                | b':'
                | b';'
                | b'<'
                | b'?'
                | b']'
                | b'_'
                | b'~'
        ) {
            end -= 1;
            continue;
        }
        if last == b')' {
            let segment = &bytes[ix..end];
            let opens = segment.iter().filter(|&&b| b == b'(').count();
            let closes = segment.iter().filter(|&&b| b == b')').count();
            if closes > opens {
                end -= 1;
                continue;
            }
        }
        break;
    }

    // Trim a trailing `;` only when it closes an HTML entity (`&...;`).
    if end > ix + proto_len && bytes[end - 1] == b';' {
        // Walk back looking for `&` before whitespace. If we find `&`, trim the entity.
        let mut j = end - 2;
        while j > ix {
            let c = bytes[j];
            if c == b'&' {
                end = j;
                break;
            }
            if !(c.is_ascii_alphanumeric() || c == b'#') {
                break;
            }
            j -= 1;
        }
    }

    if end <= ix + proto_len {
        return None;
    }

    // The domain (up to first `/`, `?`, `#`, or end) must contain a `.`
    // so that `https://localhost` or `www.` alone don't match — matching
    // remark-gfm's behavior (they DO match http/https/ftp without `.`,
    // but remark-gfm requires a `.` for the literal extension). To align
    // with the reference, allow http/https/ftp without `.` (remark accepts
    // them) but require a `.` for `www.`.
    let body = &bytes[ix + proto_len..end];
    if is_www {
        let domain_end = body
            .iter()
            .position(|&b| matches!(b, b'/' | b'?' | b'#'))
            .unwrap_or(body.len());
        if !body[..domain_end].contains(&b'.') {
            return None;
        }
    }

    let url_str = core::str::from_utf8(&bytes[ix..end]).ok()?;
    let full_url = if is_www {
        format!("http://{url_str}")
    } else {
        url_str.to_string()
    };
    Some((ix, raw_end, end, full_url))
}

#[inline]
fn is_email_local_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'+' | b'-' | b'_')
}

/// GFM extended email autolink. Given `@` at `at_ix`, walk backward for the
/// local-part and forward for the domain. Returns `(start, end, "mailto:...")`.
/// Mirrors `mdast-util-gfm-autolink-literal`: requires a `.` in the domain,
/// the TLD (last dot-segment) must contain at least one letter, and trailing
/// `.`/`-`/`_` are trimmed.
fn scan_email_autolink(bytes: &[u8], at_ix: usize) -> Option<(usize, usize, String)> {
    if at_ix >= bytes.len() || bytes[at_ix] != b'@' {
        return None;
    }
    // Walk backward to find the local-part start.
    let mut start = at_ix;
    while start > 0 && is_email_local_char(bytes[start - 1]) {
        start -= 1;
    }
    if start == at_ix {
        return None;
    }
    // Local-part cannot start with `.`, `-`, `_`, or `+` per remark's trimming.
    // Trim leading punctuation from the local-part.
    while start < at_ix && matches!(bytes[start], b'.' | b'-' | b'_' | b'+') {
        start += 1;
    }
    if start == at_ix {
        return None;
    }
    // Preceding char must be start-of-input, whitespace, or punctuation — not
    // another email-like char. Since the backward walk already consumes all
    // `is_email_local_char` bytes, the prev byte (if any) is guaranteed non-
    // local; just reject an immediate `@` or `/` that would indicate we're
    // inside another URL/email.
    if start > 0 {
        let prev = bytes[start - 1];
        if prev == b'@' || prev == b'/' {
            return None;
        }
    }
    // Forward: scan domain.
    let mut end = at_ix + 1;
    while end < bytes.len() {
        let b = bytes[end];
        if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_') {
            end += 1;
        } else {
            break;
        }
    }
    if end == at_ix + 1 {
        return None;
    }
    // Trim trailing `.`, `_`, `-` per remark: an email can't end on those.
    while end > at_ix + 1 {
        let last = bytes[end - 1];
        if matches!(last, b'.' | b'_' | b'-') {
            end -= 1;
        } else {
            break;
        }
    }
    if end == at_ix + 1 {
        return None;
    }
    // Domain must contain at least one `.`.
    let domain = &bytes[at_ix + 1..end];
    let last_dot = domain.iter().rposition(|&b| b == b'.')?;
    // TLD (last dot-segment) must contain at least one ASCII letter.
    let tld = &domain[last_dot + 1..];
    if tld.is_empty() || !tld.iter().any(|&b| b.is_ascii_alphabetic()) {
        return None;
    }
    // Underscore in the last two segments is invalid per remark
    // (`mdast-util-gfm-autolink-literal`'s `emailWithUnderscoreAtEnd` check).
    if tld.contains(&b'_') {
        return None;
    }
    if let Some(second_last_dot) = domain[..last_dot].iter().rposition(|&b| b == b'.') {
        if domain[second_last_dot + 1..last_dot].contains(&b'_') {
            return None;
        }
    } else if domain[..last_dot].contains(&b'_') {
        return None;
    }
    let email_str = core::str::from_utf8(&bytes[start..end]).ok()?;
    Some((start, end, format!("mailto:{email_str}")))
}

/// Re-merge `text + textDirective + text` sibling runs when the text ends
/// with a URL scheme and the directive's name is purely numeric (i.e. a port
/// number that got split off by the directive parser).
///
/// This is the inverse of the split that happens during inline parsing for
/// `http://host:4321/path`: the `:4321` looks like a textDirective, so the
/// inline parser emits `[text("..http://host"), textDirective("4321"), text("/path")]`.
/// GFM autolink would normally consume the whole URL as a single token before
/// the directive parser sees it, but since satteri's autolink runs as a post-
/// pass we reconstruct the original run here so autolink can find the URL.
/// Mirror of mdast-util-gfm-autolink-literal's `isCorrectDomain`: the URL's
/// domain (between `//` and the first `/`, `?`, `#`, or end) must contain a
/// dot to count as a valid autolink. Applied only in strict mode — see the
/// caller.
fn domain_has_dot(url: &str) -> bool {
    let after_scheme = match url.find("://") {
        Some(p) => &url[p + 3..],
        None => url,
    };
    let domain_end = after_scheme
        .find(['/', '?', '#'])
        .unwrap_or(after_scheme.len());
    after_scheme[..domain_end].contains('.')
}

/// Fold the bracket-depth running total forward over one string of text.
/// Returns `true` after consuming `s` iff there's a `[` (or `![`) with no
/// matching `]` so far. Backslash-escaped brackets are ignored.
fn update_bracket_depth(was_open: bool, s: &str) -> bool {
    let mut depth: i32 = if was_open { 1 } else { 0 };
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'\\' {
            i += 2;
            continue;
        }
        match c {
            b'[' => depth += 1,
            b']' if depth > 0 => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    depth > 0
}

fn merge_directive_port_splits(arena: &mut Arena) {
    // Explicitly skip Link / LinkReference — a bracketed link's label text
    // intentionally preserves `text + textDirective + text` splits (remark
    // keeps them because autolink doesn't recurse into labels).
    let parent_ids: Vec<u32> = (0..arena.len() as u32)
        .filter(|&id| {
            let n = arena.get_node(id);
            matches!(
                MdastNodeType::from_u8(n.node_type),
                Some(
                    MdastNodeType::Paragraph
                        | MdastNodeType::Heading
                        | MdastNodeType::Emphasis
                        | MdastNodeType::Strong
                        | MdastNodeType::Delete
                        | MdastNodeType::TableCell
                )
            )
        })
        .collect();

    for parent_id in parent_ids {
        let children = arena.get_children(parent_id).to_vec();
        if children.len() < 2 {
            continue;
        }
        let mut new_children: Vec<u32> = Vec::with_capacity(children.len());
        let mut i = 0;
        // When a potential link-label `[` remains unclosed in earlier siblings,
        // remark's autolink-literal never tokenizes URLs in the following text
        // and its post-transformer rejects no-dot domains. Merging back would
        // then resurrect URLs remark deliberately leaves alone (see
        // `docs/src/content/docs/ru/guides/testing.mdx` in the conformance
        // check). Track the running bracket depth across preceding siblings so
        // we can bail when we're inside a broken label attempt.
        let mut unmatched_open_bracket = false;
        while i < children.len() {
            let text_id = children[i];
            let text_node = arena.get_node(text_id);
            // Track bracket depth across every text node we visit so the
            // unmatched-`[` gate below sees a correct running total.
            let is_text = text_node.node_type == MdastNodeType::Text as u8;
            if is_text {
                let d = arena.get_type_data(text_id);
                if !d.is_empty() {
                    let s = arena.get_str(StringRef::from_bytes(d));
                    unmatched_open_bracket = update_bracket_depth(unmatched_open_bracket, s);
                }
            }
            // Need a text node whose value ends with `://<host>` (no path yet).
            if !is_text || i + 1 >= children.len() {
                new_children.push(text_id);
                i += 1;
                continue;
            }
            if unmatched_open_bracket {
                new_children.push(text_id);
                i += 1;
                continue;
            }
            let dir_id = children[i + 1];
            let dir_node = arena.get_node(dir_id);
            if dir_node.node_type != MdastNodeType::TextDirective as u8 {
                new_children.push(text_id);
                i += 1;
                continue;
            }
            // Directive name must be all ASCII digits (port number).
            let dir_data = arena.get_type_data(dir_id);
            if dir_data.len() < 8 {
                new_children.push(text_id);
                i += 1;
                continue;
            }
            let dir_name_sr = StringRef::from_bytes(&dir_data[..8]);
            let dir_name = arena.get_str(dir_name_sr).to_string();
            if dir_name.is_empty() || !dir_name.bytes().all(|b| b.is_ascii_digit()) {
                new_children.push(text_id);
                i += 1;
                continue;
            }

            // Text must end with `://<host>` — check by looking for `://`
            // after the last whitespace and then any non-whitespace host.
            let text_data = arena.get_type_data(text_id);
            let text_sr = StringRef::from_bytes(text_data);
            let text_val = arena.get_str(text_sr).to_string();
            let looks_like_url_host = {
                let after_ws = text_val
                    .rsplit(|c: char| c.is_whitespace())
                    .next()
                    .unwrap_or("");
                after_ws.contains("://")
            };
            if !looks_like_url_host {
                new_children.push(text_id);
                i += 1;
                continue;
            }

            // Build merged value. Trailing text (i+2) is merged too if present
            // and starts with a URL-path char, or we leave it standalone.
            let mut merged = text_val;
            merged.push(':');
            merged.push_str(&dir_name);

            let mut consumed = 2; // text + directive
            if i + 2 < children.len() {
                let after_id = children[i + 2];
                let after_node = arena.get_node(after_id);
                if after_node.node_type == MdastNodeType::Text as u8 {
                    let after_data = arena.get_type_data(after_id);
                    let after_sr = StringRef::from_bytes(after_data);
                    let after_val = arena.get_str(after_sr);
                    merged.push_str(after_val);
                    consumed = 3;
                }
            }

            let merged_sr = arena.alloc_string(&merged);
            let text_node_start = arena.get_node(text_id).start_offset;
            let last_id = children[i + consumed - 1];
            let last_node = arena.get_node(last_id);
            let end_offset = last_node.end_offset;
            let end_line = last_node.end_line;
            let end_column = last_node.end_column;
            let start_line = arena.get_node(text_id).start_line;
            let start_column = arena.get_node(text_id).start_column;

            // Reuse the first text node as the merged one.
            arena.set_type_data(text_id, &merged_sr.as_bytes());
            arena.set_position(
                text_id,
                text_node_start,
                end_offset,
                start_line,
                start_column,
                end_line,
                end_column,
            );
            // The leading text's brackets were already folded into
            // `unmatched_open_bracket` at the top of the loop; fold in the
            // remaining text (if any) from the trailing sibling we consumed.
            if consumed == 3 {
                let tail_sr = StringRef::from_bytes(arena.get_type_data(children[i + 2]));
                let tail = arena.get_str(tail_sr);
                unmatched_open_bracket = update_bracket_depth(unmatched_open_bracket, tail);
            }
            new_children.push(text_id);
            i += consumed;
        }
        if new_children.len() != children.len() {
            arena.set_children(parent_id, &new_children);
        }
    }
}

fn gfm_autolink_literal_pass(arena: &mut Arena) {
    let len = arena.len() as u32;
    // First collect the set of Text nodes containing URL candidates to avoid
    // mutating while iterating in a way that shifts indices. Alongside each
    // candidate we track whether we're inside a broken link-label attempt —
    // remark's autolink-literal skips such text during tokenization, and its
    // post-transformer then requires a `.` in the domain to match, so we mirror
    // that "require dot" rule only in the broken-label case.
    let mut candidates: Vec<(u32, bool)> = Vec::new();
    // Per-parent running bracket depth. Indexed by node id, sized to the
    // arena once: avoids the per-text-node HashMap entry/get hot in profiles.
    let mut bracket_depth_by_parent: Vec<i32> = vec![0; len as usize];
    let text_ty = MdastNodeType::Text as u8;
    for id in 0..len {
        let node = arena.get_node(id);
        if node.node_type != text_ty {
            continue;
        }
        let parent_id = node.parent;
        if parent_id == u32::MAX || parent_id >= len {
            continue;
        }
        let parent_type = MdastNodeType::from_u8(arena.get_node(parent_id).node_type);
        // Skip text inside link (would nest), code, imports, expressions, or
        // frontmatter.
        if matches!(
            parent_type,
            Some(
                MdastNodeType::Link
                    | MdastNodeType::InlineCode
                    | MdastNodeType::Code
                    | MdastNodeType::MdxjsEsm
                    | MdastNodeType::MdxFlowExpression
                    | MdastNodeType::MdxTextExpression
                    | MdastNodeType::Yaml
                    | MdastNodeType::Toml
            )
        ) {
            continue;
        }
        let tracks_brackets = matches!(
            parent_type,
            Some(
                MdastNodeType::Paragraph
                    | MdastNodeType::Heading
                    | MdastNodeType::Emphasis
                    | MdastNodeType::Strong
                    | MdastNodeType::Delete
                    | MdastNodeType::TableCell
            )
        );
        let data = arena.get_type_data(id);
        if data.is_empty() {
            continue;
        }
        let sr = StringRef::from_bytes(data);
        let text = arena.get_str(sr);
        let bytes = text.as_bytes();
        let mut matched = false;
        let mut search_from = 0;
        while let Some(rel) = memchr::memchr3(b'h', b'w', b'@', &bytes[search_from..]) {
            let i = search_from + rel;
            let b = bytes[i];
            if (b == b'h' || b == b'w') && scan_autolink_literal(bytes, i).is_some() {
                matched = true;
                break;
            }
            if b == b'@' && scan_email_autolink(bytes, i).is_some() {
                matched = true;
                break;
            }
            search_from = i + 1;
        }
        if tracks_brackets {
            let slot = &mut bracket_depth_by_parent[parent_id as usize];
            let was_open = *slot > 0;
            if matched {
                candidates.push((id, was_open));
            }
            if memchr::memchr2(b'[', b']', bytes).is_some() {
                let now_open = update_bracket_depth(was_open, text);
                *slot = if now_open { 1 } else { 0 };
            }
        } else if matched {
            candidates.push((id, false));
        }
    }

    for (node_id, strict) in candidates {
        split_text_with_autolinks(arena, node_id, strict);
    }
}

fn split_text_with_autolinks(arena: &mut Arena, text_id: u32, strict_domain: bool) {
    let node = arena.get_node(text_id);
    let start_offset = node.start_offset;
    let start_line = node.start_line;
    let start_column = node.start_column;
    let data = arena.get_type_data(text_id);
    if data.is_empty() {
        return;
    }
    let sr = StringRef::from_bytes(data);
    let text = arena.get_str(sr).to_string();
    let bytes = text.as_bytes();

    let mut replacements: Vec<(usize, usize, usize, String)> = Vec::new(); // (start, raw_end, end, url)
    let mut i = 0;
    while let Some(rel) = memchr::memchr3(b'h', b'w', b'@', &bytes[i..]) {
        i += rel;
        let b = bytes[i];
        if b == b'h' || b == b'w' {
            if let Some((s, raw_e, e, url)) = scan_autolink_literal(bytes, i) {
                if strict_domain && !domain_has_dot(&url) {
                    i += 1;
                    continue;
                }
                replacements.push((s, raw_e, e, url));
                i = raw_e;
                continue;
            }
        } else if let Some((s, e, url)) = scan_email_autolink(bytes, i) {
            // Don't double-match if the local-part overlaps an already-
            // emitted replacement.
            if replacements
                .last()
                .is_none_or(|&(_, _, prev_e, _)| s >= prev_e)
            {
                replacements.push((s, e, e, url));
            }
            i = e;
            continue;
        }
        i += 1;
    }

    if replacements.is_empty() {
        return;
    }

    // Remark-gfm keeps the trailing trim-back chars (e.g. `),` stripped from
    // the URL) as their own text node — rather than merging with the post-URL
    // tail — when the preceding text contains an unclosed `[` or `![`. This
    // mirrors a micromark quirk where the failed label/link attempt around the
    // autolink leaves fragmented text tokens that never coalesce.
    let preceded_by_open_bracket: Vec<bool> = replacements
        .iter()
        .map(|&(s, _, _, _)| {
            let mut depth: i32 = 0;
            let mut j = 0;
            while j < s {
                let c = bytes[j];
                if c == b'\\' {
                    j += 2;
                    continue;
                }
                match c {
                    b'[' => depth += 1,
                    b']' if depth > 0 => depth -= 1,
                    _ => {}
                }
                j += 1;
            }
            depth > 0
        })
        .collect();

    // Build the replacement nodes in order.
    let mut new_children: Vec<u32> = Vec::new();
    let mut cursor = 0usize;

    for (idx, (s, raw_e, e, url)) in replacements.into_iter().enumerate() {
        if s > cursor {
            let chunk = &text[cursor..s];
            let new_text_id = arena.alloc_node(MdastNodeType::Text as u8);
            let chunk_sr = arena.alloc_string(chunk);
            arena.set_type_data(new_text_id, &chunk_sr.as_bytes());
            arena.set_position(
                new_text_id,
                start_offset + cursor as u32,
                start_offset + s as u32,
                start_line,
                start_column + cursor as u32,
                start_line,
                start_column + s as u32,
            );
            new_children.push(new_text_id);
        }

        // Link node.
        let link_id = arena.alloc_node(MdastNodeType::Link as u8);
        let url_sr = arena.alloc_string(&url);
        let link_data = LinkData {
            url: url_sr,
            title: StringRef::empty(),
        };
        arena.set_type_data(link_id, &link_data.to_bytes());
        arena.set_position(
            link_id,
            start_offset + s as u32,
            start_offset + e as u32,
            start_line,
            start_column + s as u32,
            start_line,
            start_column + e as u32,
        );
        // Link text child = the displayed URL (without the synthetic http://).
        let displayed = &text[s..e];
        let link_text_id = arena.alloc_node(MdastNodeType::Text as u8);
        let disp_sr = arena.alloc_string(displayed);
        arena.set_type_data(link_text_id, &disp_sr.as_bytes());
        arena.set_position(
            link_text_id,
            start_offset + s as u32,
            start_offset + e as u32,
            start_line,
            start_column + s as u32,
            start_line,
            start_column + e as u32,
        );
        arena.set_children(link_id, &[link_text_id]);
        new_children.push(link_id);

        if preceded_by_open_bracket[idx] && raw_e > e {
            let chunk = &text[e..raw_e];
            let new_text_id = arena.alloc_node(MdastNodeType::Text as u8);
            let chunk_sr = arena.alloc_string(chunk);
            arena.set_type_data(new_text_id, &chunk_sr.as_bytes());
            arena.set_position(
                new_text_id,
                start_offset + e as u32,
                start_offset + raw_e as u32,
                start_line,
                start_column + e as u32,
                start_line,
                start_column + raw_e as u32,
            );
            new_children.push(new_text_id);
            cursor = raw_e;
        } else {
            cursor = e;
        }
    }

    if cursor < bytes.len() {
        let chunk = &text[cursor..];
        let new_text_id = arena.alloc_node(MdastNodeType::Text as u8);
        let chunk_sr = arena.alloc_string(chunk);
        arena.set_type_data(new_text_id, &chunk_sr.as_bytes());
        arena.set_position(
            new_text_id,
            start_offset + cursor as u32,
            start_offset + bytes.len() as u32,
            start_line,
            start_column + cursor as u32,
            start_line,
            start_column + bytes.len() as u32,
        );
        new_children.push(new_text_id);
    }

    arena.replace_node_with_children(text_id, &new_children);
}

/// Append a text value as an MDAST Text leaf, merging with the previous
/// sibling text node when possible. Matches the behavior remark inherits
/// from `mdast-util-from-markdown`, which coalesces adjacent text nodes
/// that result from entity decoding, character synthesis, etc.
#[allow(clippy::too_many_arguments)]
fn emit_text_merging(
    builder: &mut ArenaBuilder,
    text_value: &str,
    start: u32,
    end: u32,
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
) {
    if let Some(pid) = builder.last_sibling_id() {
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
                return;
            }
        }
    }
    let sr = builder.alloc_string(text_value);
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

/// For each `Text` node that lives directly under a directive's label, scan
/// for balanced backtick runs and split the text into `text + inlineCode + text`
/// pieces. This matches the common `:::tip[Set a \`baseUrl\`]` pattern without
/// needing to re-run the full inline parser on the label substring.
fn directive_label_inline_code_pass(arena: &mut Arena) {
    // Collect candidate text node ids first (pair: parent id, text id).
    let mut candidates: Vec<u32> = Vec::new();
    for id in 0..arena.len() as u32 {
        let node = arena.get_node(id);
        if node.node_type != MdastNodeType::Text as u8 {
            continue;
        }
        // Text value must contain a backtick to be worth processing.
        let data = arena.get_type_data(id);
        if data.is_empty() {
            continue;
        }
        let sr = StringRef::from_bytes(data);
        let text = arena.get_str(sr);
        if !text.contains('`') {
            continue;
        }

        let parent_id = node.parent;
        let parent = arena.get_node(parent_id);
        let parent_type = MdastNodeType::from_u8(parent.node_type);

        let is_directive_label = match parent_type {
            // Text directly under a leaf/text directive — the directive's
            // children ARE the label.
            Some(MdastNodeType::LeafDirective | MdastNodeType::TextDirective) => true,
            // Paragraph under a container directive is the label iff it has
            // the `directiveLabel:true` marker.
            Some(MdastNodeType::Paragraph) => {
                let node_data = arena.get_node_data(parent_id);
                node_data
                    .map(|d| d.starts_with(b"{\"directiveLabel\":true}"))
                    .unwrap_or(false)
            }
            _ => false,
        };
        if !is_directive_label {
            continue;
        }
        candidates.push(id);
    }

    for text_id in candidates {
        split_text_on_backticks(arena, text_id);
    }
}

/// Split a `Text` node's value into `text + inlineCode + text …` on balanced
/// backtick runs. Only handles the simple case (same-length opening/closing
/// runs, single-line), which is what directive labels carry in practice.
fn split_text_on_backticks(arena: &mut Arena, text_id: u32) {
    let data = arena.get_type_data(text_id);
    if data.is_empty() {
        return;
    }
    let sr = StringRef::from_bytes(data);
    let text = arena.get_str(sr).to_string();
    let bytes = text.as_bytes();

    // Find all balanced backtick pairs.
    #[derive(Clone, Copy)]
    struct Pair {
        open_start: usize,
        open_end: usize,
        close_start: usize,
        close_end: usize,
    }
    let mut pairs: Vec<Pair> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'`' {
            i += 1;
            continue;
        }
        // Count run length.
        let open_start = i;
        while i < bytes.len() && bytes[i] == b'`' {
            i += 1;
        }
        let open_end = i;
        let run_len = open_end - open_start;
        // Find matching closing run of the same length.
        let mut j = i;
        let matched_close: Option<(usize, usize)> = loop {
            if j >= bytes.len() {
                break None;
            }
            if bytes[j] == b'`' {
                let close_start = j;
                while j < bytes.len() && bytes[j] == b'`' {
                    j += 1;
                }
                let close_end = j;
                if close_end - close_start == run_len {
                    break Some((close_start, close_end));
                }
                // Not a match; skip this run and continue searching.
                continue;
            }
            j += 1;
        };
        if let Some((cs, ce)) = matched_close {
            pairs.push(Pair {
                open_start,
                open_end,
                close_start: cs,
                close_end: ce,
            });
            i = ce;
        }
    }

    if pairs.is_empty() {
        return;
    }

    // Build the replacement child list.
    let node = arena.get_node(text_id);
    let base_start = node.start_offset;
    let base_line = node.start_line;
    let base_col = node.start_column;

    let mut new_children: Vec<u32> = Vec::new();
    let mut cursor = 0usize;
    for p in pairs {
        // Leading plain text.
        if p.open_start > cursor {
            let segment = &text[cursor..p.open_start];
            if !segment.is_empty() {
                let seg_sr = arena.alloc_string(segment);
                let tid = arena.alloc_node(MdastNodeType::Text as u8);
                arena.set_type_data(tid, &seg_sr.as_bytes());
                arena.set_position(
                    tid,
                    base_start + cursor as u32,
                    base_start + p.open_start as u32,
                    base_line,
                    base_col + cursor as u32,
                    base_line,
                    base_col + p.open_start as u32,
                );
                new_children.push(tid);
            }
        }
        // Inline code.
        let code_value = &text[p.open_end..p.close_start];
        let code_sr = arena.alloc_string(code_value);
        let cid = arena.alloc_node(MdastNodeType::InlineCode as u8);
        arena.set_type_data(cid, &code_sr.as_bytes());
        arena.set_position(
            cid,
            base_start + p.open_start as u32,
            base_start + p.close_end as u32,
            base_line,
            base_col + p.open_start as u32,
            base_line,
            base_col + p.close_end as u32,
        );
        new_children.push(cid);
        cursor = p.close_end;
    }
    // Trailing plain text.
    if cursor < text.len() {
        let segment = &text[cursor..];
        let seg_sr = arena.alloc_string(segment);
        let tid = arena.alloc_node(MdastNodeType::Text as u8);
        arena.set_type_data(tid, &seg_sr.as_bytes());
        arena.set_position(
            tid,
            base_start + cursor as u32,
            base_start + text.len() as u32,
            base_line,
            base_col + cursor as u32,
            base_line,
            base_col + text.len() as u32,
        );
        new_children.push(tid);
    }

    arena.replace_node_with_children(text_id, &new_children);
}

/// Post-pass matching `directive_label_inline_code_pass` for JSX tags. For
/// each `Text` node directly under a directive label, split on balanced
/// `<Name>…</Name>` (or self-closing `<Name/>`) runs and emit
/// `mdxJsxTextElement` children. Also splits on balanced `{…}` spans and
/// emits `mdxTextExpression` nodes.
fn directive_label_jsx_pass(arena: &mut Arena) {
    let mut candidates: Vec<u32> = Vec::new();
    for id in 0..arena.len() as u32 {
        let node = arena.get_node(id);
        if node.node_type != MdastNodeType::Text as u8 {
            continue;
        }
        let data = arena.get_type_data(id);
        if data.is_empty() {
            continue;
        }
        let sr = StringRef::from_bytes(data);
        let text = arena.get_str(sr);
        if !text.contains('<') && !text.contains('{') {
            continue;
        }
        let parent_id = node.parent;
        let parent = arena.get_node(parent_id);
        let parent_type = MdastNodeType::from_u8(parent.node_type);
        let is_directive_label = match parent_type {
            Some(MdastNodeType::LeafDirective | MdastNodeType::TextDirective) => true,
            Some(MdastNodeType::Paragraph) => arena
                .get_node_data(parent_id)
                .map(|d| d.starts_with(b"{\"directiveLabel\":true}"))
                .unwrap_or(false),
            _ => false,
        };
        if !is_directive_label {
            continue;
        }
        candidates.push(id);
    }
    for text_id in candidates {
        split_text_on_jsx_tags(arena, text_id);
    }
    // Second pass picks up text nodes created by the first split and emits
    // MDX text expressions for `{…}` runs.
    let mut expr_candidates: Vec<u32> = Vec::new();
    for id in 0..arena.len() as u32 {
        let node = arena.get_node(id);
        if node.node_type != MdastNodeType::Text as u8 {
            continue;
        }
        let data = arena.get_type_data(id);
        if data.is_empty() {
            continue;
        }
        let sr = StringRef::from_bytes(data);
        let text = arena.get_str(sr);
        if !text.contains('{') {
            continue;
        }
        let parent_id = node.parent;
        let parent = arena.get_node(parent_id);
        let parent_type = MdastNodeType::from_u8(parent.node_type);
        let in_label = match parent_type {
            Some(MdastNodeType::LeafDirective | MdastNodeType::TextDirective) => true,
            Some(MdastNodeType::Paragraph) => arena
                .get_node_data(parent_id)
                .map(|d| d.starts_with(b"{\"directiveLabel\":true}"))
                .unwrap_or(false),
            // Also handle the children of a JSX text element created by the
            // first pass — they also live under a directive label.
            Some(MdastNodeType::MdxJsxTextElement) => {
                let grandparent_id = parent.parent;
                if grandparent_id == u32::MAX {
                    false
                } else {
                    let grandparent = arena.get_node(grandparent_id);
                    let gp_type = MdastNodeType::from_u8(grandparent.node_type);
                    matches!(
                        gp_type,
                        Some(MdastNodeType::LeafDirective | MdastNodeType::TextDirective)
                    ) || (gp_type == Some(MdastNodeType::Paragraph)
                        && arena
                            .get_node_data(grandparent_id)
                            .map(|d| d.starts_with(b"{\"directiveLabel\":true}"))
                            .unwrap_or(false))
                }
            }
            _ => false,
        };
        if !in_label {
            continue;
        }
        expr_candidates.push(id);
    }
    for text_id in expr_candidates {
        split_text_on_mdx_expressions(arena, text_id);
    }
}

/// Split a `Text` node on `{…}` spans (balanced braces, JS-aware) and emit
/// `mdxTextExpression` nodes for the matched spans.
fn split_text_on_mdx_expressions(arena: &mut Arena, text_id: u32) {
    use crate::mdx::scan_mdx_inline_expression;
    let data = arena.get_type_data(text_id);
    if data.is_empty() {
        return;
    }
    let sr = StringRef::from_bytes(data);
    let text = arena.get_str(sr).to_string();
    let bytes = text.as_bytes();
    let mut spans: Vec<(usize, usize, usize, usize)> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'{' {
            i += 1;
            continue;
        }
        let Some((content_start, content_end, total_len)) = scan_mdx_inline_expression(&bytes[i..])
        else {
            i += 1;
            continue;
        };
        spans.push((i, i + total_len, i + content_start, i + content_end));
        i += total_len;
    }
    if spans.is_empty() {
        return;
    }
    let node = arena.get_node(text_id);
    let base_start = node.start_offset;
    let base_line = node.start_line;
    let base_col = node.start_column;

    let mut new_children: Vec<u32> = Vec::new();
    let mut cursor = 0usize;
    for (span_start, span_end, content_start, content_end) in spans {
        if span_start > cursor {
            let seg = &text[cursor..span_start];
            let seg_sr = arena.alloc_string(seg);
            let tid = arena.alloc_node(MdastNodeType::Text as u8);
            arena.set_type_data(tid, &seg_sr.as_bytes());
            arena.set_position(
                tid,
                base_start + cursor as u32,
                base_start + span_start as u32,
                base_line,
                base_col + cursor as u32,
                base_line,
                base_col + span_start as u32,
            );
            new_children.push(tid);
        }
        let content = &text[content_start..content_end];
        let content_sr = arena.alloc_string(content);
        let eid = arena.alloc_node(MdastNodeType::MdxTextExpression as u8);
        arena.set_type_data(eid, &content_sr.as_bytes());
        arena.set_position(
            eid,
            base_start + span_start as u32,
            base_start + span_end as u32,
            base_line,
            base_col + span_start as u32,
            base_line,
            base_col + span_end as u32,
        );
        new_children.push(eid);
        cursor = span_end;
    }
    if cursor < text.len() {
        let seg = &text[cursor..];
        let seg_sr = arena.alloc_string(seg);
        let tid = arena.alloc_node(MdastNodeType::Text as u8);
        arena.set_type_data(tid, &seg_sr.as_bytes());
        arena.set_position(
            tid,
            base_start + cursor as u32,
            base_start + text.len() as u32,
            base_line,
            base_col + cursor as u32,
            base_line,
            base_col + text.len() as u32,
        );
        new_children.push(tid);
    }
    arena.replace_node_with_children(text_id, &new_children);
}

/// Split a `Text` node on `<Name>…</Name>` / `<Name/>` spans, producing
/// `mdxJsxTextElement` nodes for the matched spans. The inner content of a
/// matched open/close pair becomes a child `Text` node (no recursion — nested
/// JSX inside a directive label is rare enough that a single-level split
/// covers the conformance cases).
fn split_text_on_jsx_tags(arena: &mut Arena, text_id: u32) {
    use crate::mdx::{parse_jsx_tag, scan_mdx_inline_jsx};
    let data = arena.get_type_data(text_id);
    if data.is_empty() {
        return;
    }
    let sr = StringRef::from_bytes(data);
    let text = arena.get_str(sr).to_string();
    let bytes = text.as_bytes();

    #[derive(Clone)]
    enum Span {
        SelfClosing {
            start: usize,
            end: usize,
            name: alloc::string::String,
        },
        Paired {
            start: usize,
            open_end: usize,
            close_start: usize,
            end: usize,
            name: alloc::string::String,
        },
    }

    let mut spans: Vec<Span> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        let Some(tag_end) = scan_mdx_inline_jsx(&bytes[i..]) else {
            i += 1;
            continue;
        };
        let tag_raw = &text[i..i + tag_end];
        let jsx = parse_jsx_tag(tag_raw);
        if jsx.is_closing {
            i += tag_end;
            continue;
        }
        if jsx.is_self_closing {
            spans.push(Span::SelfClosing {
                start: i,
                end: i + tag_end,
                name: jsx.name.to_string(),
            });
            i += tag_end;
            continue;
        }
        // Opening tag — scan forward for a matching `</name>`.
        let name = jsx.name.to_string();
        let open_end = i + tag_end;
        let mut j = open_end;
        let mut close_span: Option<(usize, usize)> = None;
        while j < bytes.len() {
            if bytes[j] != b'<' {
                j += 1;
                continue;
            }
            let Some(inner_tag_end) = scan_mdx_inline_jsx(&bytes[j..]) else {
                j += 1;
                continue;
            };
            let inner_tag = &text[j..j + inner_tag_end];
            let inner_jsx = parse_jsx_tag(inner_tag);
            if inner_jsx.is_closing && inner_jsx.name.as_ref() == name.as_str() {
                close_span = Some((j, j + inner_tag_end));
                break;
            }
            j += inner_tag_end;
        }
        if let Some((close_start, close_end)) = close_span {
            spans.push(Span::Paired {
                start: i,
                open_end,
                close_start,
                end: close_end,
                name,
            });
            i = close_end;
        } else {
            i = open_end;
        }
    }

    if spans.is_empty() {
        return;
    }

    let node = arena.get_node(text_id);
    let base_start = node.start_offset;
    let base_line = node.start_line;
    let base_col = node.start_column;

    let push_text =
        |arena: &mut Arena, out: &mut Vec<u32>, segment: &str, seg_start: usize, seg_end: usize| {
            if segment.is_empty() {
                return;
            }
            let seg_sr = arena.alloc_string(segment);
            let tid = arena.alloc_node(MdastNodeType::Text as u8);
            arena.set_type_data(tid, &seg_sr.as_bytes());
            arena.set_position(
                tid,
                base_start + seg_start as u32,
                base_start + seg_end as u32,
                base_line,
                base_col + seg_start as u32,
                base_line,
                base_col + seg_end as u32,
            );
            out.push(tid);
        };

    let mut new_children: Vec<u32> = Vec::new();
    let mut cursor = 0usize;
    for span in spans {
        match span {
            Span::SelfClosing { start, end, name } => {
                push_text(
                    arena,
                    &mut new_children,
                    &text[cursor..start],
                    cursor,
                    start,
                );
                let name_sr = arena.alloc_string(&name);
                let jsx_data = satteri_ast::mdast::encode_mdx_jsx_element_data(name_sr, &[]);
                let jid = arena.alloc_node(MdastNodeType::MdxJsxTextElement as u8);
                arena.set_type_data(jid, &jsx_data);
                arena.set_position(
                    jid,
                    base_start + start as u32,
                    base_start + end as u32,
                    base_line,
                    base_col + start as u32,
                    base_line,
                    base_col + end as u32,
                );
                new_children.push(jid);
                cursor = end;
            }
            Span::Paired {
                start,
                open_end,
                close_start,
                end,
                name,
            } => {
                push_text(
                    arena,
                    &mut new_children,
                    &text[cursor..start],
                    cursor,
                    start,
                );
                let name_sr = arena.alloc_string(&name);
                let jsx_data = satteri_ast::mdast::encode_mdx_jsx_element_data(name_sr, &[]);
                let jid = arena.alloc_node(MdastNodeType::MdxJsxTextElement as u8);
                arena.set_type_data(jid, &jsx_data);
                arena.set_position(
                    jid,
                    base_start + start as u32,
                    base_start + end as u32,
                    base_line,
                    base_col + start as u32,
                    base_line,
                    base_col + end as u32,
                );
                // Inner text child.
                let inner = &text[open_end..close_start];
                if !inner.is_empty() {
                    let inner_sr = arena.alloc_string(inner);
                    let cid = arena.alloc_node(MdastNodeType::Text as u8);
                    arena.set_type_data(cid, &inner_sr.as_bytes());
                    arena.set_position(
                        cid,
                        base_start + open_end as u32,
                        base_start + close_start as u32,
                        base_line,
                        base_col + open_end as u32,
                        base_line,
                        base_col + close_start as u32,
                    );
                    arena.set_children(jid, &[cid]);
                }
                new_children.push(jid);
                cursor = end;
            }
        }
    }
    push_text(
        arena,
        &mut new_children,
        &text[cursor..],
        cursor,
        text.len(),
    );

    arena.replace_node_with_children(text_id, &new_children);
}

fn mdx_mark_and_unravel(arena: &mut Arena) {
    let len = arena.len() as u32;
    // Only paragraphs containing inline MDX nodes can be promoted; without
    // any in the arena the per-paragraph work below is guaranteed wasted.
    let has_inline_mdx = (0..len).any(|id| {
        matches!(
            MdastNodeType::from_u8(arena.get_node(id).node_type),
            Some(MdastNodeType::MdxJsxTextElement | MdastNodeType::MdxTextExpression),
        )
    });
    if !has_inline_mdx {
        return;
    }
    for id in 0..len {
        let node = arena.get_node(id);
        if node.node_type != MdastNodeType::Paragraph as u8 {
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

/// Pulldown-cmark sets the Link/Image node's `item.end` to the end of the
/// *shortcut* label (first `]`) for Collapsed references — the trailing `[]`
/// is "consumed" by the parser but not included in the span. Remark's mdast
/// includes the whole `[...][]`, so extend by 2 bytes when the source bytes
/// there actually are `[]`.
fn reference_end(
    source: &str,
    cursor: &mut satteri_arena::LineIndexCursor<'_>,
    end: u32,
    kind: u8,
) -> (u32, u32, u32) {
    let bytes = source.as_bytes();
    let mut out_end = end;
    if kind == 1 {
        let i = end as usize;
        if i + 1 < bytes.len() && bytes[i] == b'[' && bytes[i + 1] == b']' {
            out_end = end + 2;
        }
    }
    let (line, col) = cursor.offset_to_line_col(out_end);
    (out_end, line, col)
}

/// Extract the source text of the reference label — the bit between the
/// brackets that names the definition. Pulldown-cmark normalizes whitespace
/// when it stores `id` on the link, so using that clobbers the `label` field
/// remark preserves verbatim. Shortcut/Collapsed take the text from the
/// displayed brackets; Full takes the second pair (`[text][a  b]`).
///
/// `end` must already be the adjusted end from `reference_end` (so Collapsed
/// extends past `[]`).
fn extract_reference_label(source: &str, start: u32, end: u32, kind: u8, is_image: bool) -> &str {
    let bytes = source.as_bytes();
    let inner_start = if is_image {
        start as usize + 2
    } else {
        start as usize + 1
    };
    if kind == 2 {
        // Full `[text][label]`: walk back from `end` (past closing `]`) to
        // find the matching `[`.
        let close2 = end as usize - 1; // position of the second `]`
        let mut open2 = close2;
        while open2 > inner_start && bytes[open2 - 1] != b'[' {
            open2 -= 1;
        }
        return &source[open2..close2];
    }
    if kind == 1 {
        // Collapsed `[label][]`: `end` is past the trailing `]`, so drop the
        // last three bytes (`][]`) to get to the closing `]` of the label.
        let close1 = end as usize - 3;
        return &source[inner_start..close1];
    }
    // Shortcut `[label]`: `end` sits past the closing `]`.
    &source[inner_start..end as usize - 1]
}

/// Map a pulldown-cmark `LinkType` to an MDAST reference kind
/// (0 = shortcut, 1 = collapsed, 2 = full). Returns `None` for link types
/// that resolve to an inline `link`/`image` (Inline, Autolink, Email, WikiLink).
fn reference_kind(link_type: LinkType) -> Option<u8> {
    match link_type {
        LinkType::Reference | LinkType::ReferenceUnknown => Some(2),
        LinkType::Collapsed | LinkType::CollapsedUnknown => Some(1),
        LinkType::Shortcut | LinkType::ShortcutUnknown => Some(0),
        _ => None,
    }
}

/// mdast identifier normalization. Matches remark's pipeline exactly:
/// `micromark-util-normalize-identifier` collapses `[\t\n\r ]` runs, trims,
/// then case-folds via `toLowerCase().toUpperCase()` (Unicode-approximate);
/// `mdast-util-from-markdown` then lowercases. The triple-case dance
/// matters for chars like `ẞ` → `ss`, where a single lowercase would give
/// `ß` and break cross-references to a `[SS]` definition.
fn normalize_identifier(s: &str) -> String {
    if s.is_ascii() {
        // ASCII case folding is round-trip stable, so the
        // lower→upper→lower dance collapses to a single in-place lowercase.
        let mut out = String::with_capacity(s.len());
        let mut last_was_ws = false;
        for &b in s.as_bytes() {
            if matches!(b, b' ' | b'\t' | b'\n' | b'\r') {
                if !last_was_ws && !out.is_empty() {
                    out.push(' ');
                    last_was_ws = true;
                }
            } else {
                out.push(b.to_ascii_lowercase() as char);
                last_was_ws = false;
            }
        }
        if out.ends_with(' ') {
            out.pop();
        }
        return out;
    }
    let mut collapsed = String::with_capacity(s.len());
    let mut last_was_ws = false;
    for ch in s.chars() {
        if matches!(ch, ' ' | '\t' | '\n' | '\r') {
            if !last_was_ws {
                collapsed.push(' ');
                last_was_ws = true;
            }
        } else {
            collapsed.push(ch);
            last_was_ws = false;
        }
    }
    collapsed
        .trim()
        .to_lowercase()
        .to_uppercase()
        .to_lowercase()
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
