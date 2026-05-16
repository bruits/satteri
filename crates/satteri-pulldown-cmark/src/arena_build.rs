//! Direct arena builder: walks the pulldown-cmark internal tree and builds
//! a `satteri_arena::Arena` without going through the Event iterator.

use satteri_arena::{Arena, ArenaBuilder, LineIndex, Mdast, StringRef};
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

use crate::post_passes::MDX_EXPLICIT_JSX_DATA;

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
pub fn parse(source: &str, options: Options) -> (Arena<Mdast>, Vec<(usize, String)>) {
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
    let arena = Arena::<Mdast>::with_capacity(
        source_buf,
        estimated_nodes,
        estimated_nodes,
        estimated_nodes * 9,
    );
    let mut builder: ArenaBuilder<Mdast> = ArenaBuilder::from_arena(arena);

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
    let mut paragraph_open_depth: Vec<usize> = Vec::new();

    // Refdefs in source order. Each container close pass claims the defs whose
    // source range lies inside it; the rest get emitted at root. `emitted`
    // tracks which ones have already been placed.
    let mut refdefs_pending: Vec<PendingRefdef> = inner
        .allocs
        .refdefs_all
        .iter()
        .map(|(label, def)| PendingRefdef {
            label: label.as_ref().to_string(),
            dest: def.dest.to_string(),
            title: def.title.as_ref().map(|t| t.to_string()),
            span: def.span.clone(),
        })
        .collect();
    refdefs_pending.sort_by_key(|r| r.span.start);
    let mut refdef_emitted: Vec<bool> = vec![false; refdefs_pending.len()];

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
                let parent_body = inner.tree.peek_up().map(|p| inner.tree[p].item.body);
                let end = crate::firstpass::mdast_position_end(
                    &item,
                    source.as_bytes(),
                    parent_body.as_ref(),
                );
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
                    ItemBody::FencedCodeBlock(_) | ItemBody::IndentCodeBlock(_) => {
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
                            let code_start_column = builder.arena_ref().get_node(id).start_column;
                            let parent_body =
                                inner.tree.peek_up().map(|p| &inner.tree[p].item.body);
                            if let Some(ext) = crate::firstpass::extend_indented_code_block(
                                &inner.tree[ix].item,
                                source.as_bytes(),
                                parent_body,
                                code_start_column,
                                end,
                            ) {
                                code_end = ext.end_offset;
                                let (el, ec) = cursor.offset_to_line_col(code_end);
                                code_end_line = el;
                                code_end_col = ec;
                                if ext.extra_blank_lines > 0 {
                                    let id = builder.current_node_id();
                                    let mut data = builder.arena_ref().get_type_data(id).to_vec();
                                    if data.len() >= 24 {
                                        let mut extended = String::with_capacity(
                                            content.len() + ext.extra_blank_lines,
                                        );
                                        extended.push_str(&content);
                                        for _ in 0..ext.extra_blank_lines {
                                            extended.push('\n');
                                        }
                                        let sr2 = builder.alloc_string(&extended);
                                        data = builder.arena_ref().get_type_data(id).to_vec();
                                        data[16..24].copy_from_slice(&sr2.as_bytes());
                                        builder.set_data_current(&data);
                                    }
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
                    // HTML block close: write accumulated content. The bool
                    // says whether to trim a trailing newline — true for
                    // type 6/7 (always) and for type 1-5 that hit their
                    // closer pattern; false for type 1-5 that ran to EOF
                    // without a closer (then the trailing `\n` is content).
                    ItemBody::HtmlBlock(trim_trailing) => {
                        if let Some(content) = html_block_buf.take() {
                            let trimmed = if *trim_trailing {
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
                        let node = builder.arena_ref().get_node(id);
                        let orig_start_offset = node.start_offset;
                        // Pull in any refdefs whose source range falls inside
                        // this list item before we evaluate spread / position.
                        if emit_refdefs_in_container(
                            &mut builder,
                            &mut cursor,
                            source,
                            &refdefs_pending,
                            &mut refdef_emitted,
                            orig_start_offset as usize,
                            item.end,
                        ) {
                            builder.sort_current_pending_children_by_source_order();
                        }
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
                        let (mut cont_end, mut cont_end_line, mut cont_end_col) =
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
                        if let Some(extended) =
                            crate::firstpass::extend_list_item_to_next_sibling_content(
                                &inner.tree,
                                ix,
                                source.as_bytes(),
                                cont_end,
                            )
                        {
                            cont_end = extended;
                            let (el, ec) = cursor.offset_to_line_col(cont_end);
                            cont_end_line = el;
                            cont_end_col = ec;
                        }
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
                        let (mut cont_end, mut cont_end_line, mut cont_end_col) =
                            if let Some(last_child) = builder.last_sibling_id() {
                                let lc = builder.arena_ref().get_node(last_child);
                                (lc.end_offset, lc.end_line, lc.end_column)
                            } else {
                                (end, end_line, end_col)
                            };
                        if let Some(extended) =
                            crate::firstpass::extend_list_in_blockquote_through_marker_lines(
                                &inner.tree,
                                ix,
                                source.as_bytes(),
                                cont_end,
                            )
                        {
                            cont_end = extended;
                            let (el, ec) = cursor.offset_to_line_col(cont_end);
                            cont_end_line = el;
                            cont_end_col = ec;
                        }
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
                        // If this is a Paragraph close, drain any inline JSX
                        // that was opened inside the paragraph but never
                        // matched. micromark-mdx errors in this situation
                        // ("Expected a closing tag for `<X>` … before the
                        // end of `paragraph`"); we mirror the error rather
                        // than silently produce an invalid tree where the
                        // would-be paragraph-close ends up closing the
                        // dangling JSX node.
                        if matches!(item.body, ItemBody::Paragraph | ItemBody::TightParagraph) {
                            if let Some(opened_at) = paragraph_open_depth.pop() {
                                while builder.stack_depth() > opened_at {
                                    if let Some((name, offset)) = jsx_stack.pop() {
                                        let loc = byte_offset_to_line_col(source, offset as usize);
                                        mdx_errors.push((
                                            offset as usize,
                                            format!(
                                                "Expected a closing tag for `<{name}>` ({loc}) before the end of `paragraph`"
                                            ),
                                        ));
                                    }
                                    builder.close_node();
                                }
                            }
                        }
                        let id = builder.current_node_id();
                        let node = builder.arena_ref().get_node(id);
                        let orig_start = node.start_offset;
                        let orig_start_line = node.start_line;
                        let orig_start_col = node.start_column;
                        // Claim refdefs nested in this container before its
                        // children are finalized.
                        if matches!(
                            item.body,
                            ItemBody::BlockQuote(..)
                                | ItemBody::ContainerDirective(..)
                                | ItemBody::FootnoteDefinition(..)
                        ) && emit_refdefs_in_container(
                            &mut builder,
                            &mut cursor,
                            source,
                            &refdefs_pending,
                            &mut refdef_emitted,
                            orig_start as usize,
                            item.end,
                        ) {
                            builder.sort_current_pending_children_by_source_order();
                        }
                        let use_last_child = matches!(
                            item.body,
                            ItemBody::BlockQuote(..) | ItemBody::ContainerDirective(..)
                        );
                        let use_last_child_strict =
                            matches!(item.body, ItemBody::FootnoteDefinition(..));
                        let (mut cont_end, mut cont_end_line, mut cont_end_col) = if use_last_child
                        {
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
                        } else if use_last_child_strict {
                            // FootnoteDefinition: end at the last child's end
                            // (don't absorb trailing blank lines / whitespace
                            // beyond the content — matches remark, which
                            // trims trailing source-whitespace from the
                            // definition's span).
                            if let Some(last_child) = builder.last_sibling_id() {
                                let lc = builder.arena_ref().get_node(last_child);
                                (lc.end_offset, lc.end_line, lc.end_column)
                            } else {
                                (end, end_line, end_col)
                            }
                        } else {
                            (end, end_line, end_col)
                        };

                        if let Some(extended) =
                            crate::firstpass::extend_inner_blockquote_through_outer_markers(
                                &inner.tree,
                                ix,
                                source.as_bytes(),
                                cont_end,
                            )
                        {
                            cont_end = extended;
                            let (el, ec) = cursor.offset_to_line_col(cont_end);
                            cont_end_line = el;
                            cont_end_col = ec;
                        }
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
                        // mdx-js extends CommonMark's "alt = stripped visible
                        // content" rule by also concatenating the literal
                        // body of `{...}` expressions. e.g. `![{1+2}](u)` →
                        // alt = "1+2".
                        ItemBody::MdxTextExpression(cow_ix)
                        | ItemBody::MdxFlowExpression(cow_ix) => {
                            let cow = inner.allocs.take_cow(*cow_ix);
                            buf.push_str(&cow);
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
                        paragraph_open_depth.push(builder.stack_depth());
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
                    ItemBody::IndentCodeBlock(_) => {
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
                            let label_ref = match unescape_label_backslashes(label_src) {
                                Some(unescaped) => builder.alloc_string(&unescaped),
                                None => builder.alloc_string(label_src),
                            };
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
                            let label_ref = match unescape_label_backslashes(label_src) {
                                Some(unescaped) => builder.alloc_string(&unescaped),
                                None => builder.alloc_string(label_src),
                            };
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
                        // `identifier` is the normalized form (case-folded,
                        // whitespace-collapsed); `label` is the human-
                        // readable form with backslash escapes resolved.
                        // Mirrors mdast-util-from-markdown's
                        // footnoteDefinition handler (and our Definition
                        // emission in `emit_pending_refdef`).
                        let id_sr = builder.alloc_string(&normalize_identifier(&label_cow));
                        let label_sr = match unescape_label_backslashes(&label_cow) {
                            Some(unescaped) => builder.alloc_string(&unescaped),
                            None => builder.alloc_string(&label_cow),
                        };
                        builder.open_node(MdastNodeType::FootnoteDefinition as u8);
                        builder.set_position_current(
                            start, end, start_line, start_col, end_line, end_col,
                        );
                        builder.set_data_current(
                            &FootnoteDefinitionData {
                                identifier: id_sr,
                                label: label_sr,
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
                            let id = builder.current_node_id();
                            builder
                                .arena_mut()
                                .set_node_data(id, MDX_EXPLICIT_JSX_DATA.to_vec());
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
                        crate::post_passes::emit_text_merging(
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
                        crate::post_passes::emit_text_merging(
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
                        let slice = &source[start as usize..end as usize];
                        let sr = match normalize_inline_html_wrap(slice) {
                            Some(normalized) => builder.alloc_string(&normalized),
                            None => StringRef::new(start, end - start),
                        };
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
                        // Identifier is normalized (case-folded + whitespace-
                        // collapsed); label keeps the raw source form. Same
                        // pattern as FootnoteDefinition.
                        let id_sr = builder.alloc_string(&normalize_identifier(&cow));
                        let label_sr = builder.alloc_string(&cow);
                        let data = ReferenceData {
                            identifier: id_sr,
                            label: label_sr,
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

    // Root-level refdefs: anything not already emitted inside a container.
    // Interleaved with the other root children in source order.
    let mut emitted_any_at_root = false;
    for (i, rd) in refdefs_pending.iter().enumerate() {
        if refdef_emitted[i] {
            continue;
        }
        emit_pending_refdef(&mut builder, &mut cursor, source, rd);
        emitted_any_at_root = true;
    }
    if emitted_any_at_root {
        builder.sort_current_pending_children_by_source_order();
    }

    // Close root.
    builder.close_node();
    let mut arena = builder.finish();
    arena.parse_options = options.bits();

    // Source-level early exits: post-passes scan the arena to find
    // candidate nodes, but if the construct's trigger char(s) don't
    // appear in the source at all, no candidate can exist. The memchr
    // probes are conservative supersets (e.g. `@` matches both emails
    // and unrelated literal text); the actual passes still validate.
    let source_bytes = source.as_bytes();

    if options.contains(Options::ENABLE_MDX) && memchr::memchr2(b'<', b'{', source_bytes).is_some()
    {
        crate::post_passes::mdx_mark_and_unravel(&mut arena);
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
        if options.contains(Options::ENABLE_DIRECTIVE)
            && memchr::memmem::find(source_bytes, b"://").is_some()
        {
            crate::post_passes::merge_directive_port_splits(&mut arena);
        }
        if memchr::memchr3(b'h', b'w', b'@', source_bytes).is_some() {
            crate::post_passes::gfm_autolink_literal_pass(&mut arena);
        }
    }

    if options.contains(Options::ENABLE_DIRECTIVE) {
        // Directive labels are stored as a single Text node today. Remark
        // inline-parses them so constructs like `` `code` `` inside
        // `:::tip[Set a \`baseUrl\`]` end up as inlineCode. We mirror the
        // common case (backticks) here as a post-pass.
        if memchr::memchr(b'`', source_bytes).is_some() {
            crate::post_passes::directive_label_inline_code_pass(&mut arena);
        }
        if options.contains(Options::ENABLE_MDX)
            && memchr::memchr2(b'<', b'{', source_bytes).is_some()
        {
            // Same idea for JSX tags inside a directive label —
            // `:::note[The <code>x</code> property]`.
            crate::post_passes::directive_label_jsx_pass(&mut arena);
        }
    }

    // Precompute per-node code-point offsets so `to_raw_buffer` skips a
    // second `LineIndex` build + per-node `byte_to_cp_offset` lookup.
    // ASCII sources skip — `cp == byte` and `to_raw_buffer` won't touch
    // the cache. The cursor is already warm from the arena walk.
    if !source.is_ascii() {
        let mut cp_offsets = Vec::with_capacity(arena.nodes.len());
        for node in &arena.nodes {
            let pair = if node.start_line == 0 && node.start_offset == 0 {
                (0u32, 0u32)
            } else {
                (
                    cursor.byte_to_cp_offset(node.start_offset),
                    cursor.byte_to_cp_offset(node.end_offset),
                )
            };
            cp_offsets.push(pair);
        }
        arena.cp_offsets = cp_offsets;
    }

    (arena, mdx_errors)
}
struct PendingRefdef {
    label: String,
    dest: String,
    title: Option<String>,
    span: core::ops::Range<usize>,
}

fn emit_pending_refdef(
    builder: &mut ArenaBuilder<Mdast>,
    cursor: &mut satteri_arena::LineIndexCursor<'_, '_>,
    source: &str,
    rd: &PendingRefdef,
) {
    let start = rd.span.start as u32;
    let end = rd.span.end as u32;
    let (sl, sc) = cursor.offset_to_line_col(start);
    let (el, ec) = cursor.offset_to_line_col(end);
    let url_ref = builder.alloc_string(&rd.dest);
    let title_ref = match &rd.title {
        Some(t) => builder.alloc_string(t),
        None => StringRef::empty(),
    };
    let raw_label = extract_definition_label(source, start).unwrap_or(rd.label.as_str());
    let label_ref = match unescape_label_backslashes(raw_label) {
        Some(unescaped) => builder.alloc_string(&unescaped),
        None => builder.alloc_string(raw_label),
    };
    let identifier_ref = builder.alloc_string(&normalize_identifier(&rd.label));
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

/// Emit any not-yet-emitted refdefs whose source range falls inside the
/// container span `[container_start, container_end)`. Returns true if at
/// least one was emitted, so the caller knows it should re-sort the
/// container's pending children to keep source order.
fn emit_refdefs_in_container(
    builder: &mut ArenaBuilder<Mdast>,
    cursor: &mut satteri_arena::LineIndexCursor<'_, '_>,
    source: &str,
    pending: &[PendingRefdef],
    emitted: &mut [bool],
    container_start: usize,
    container_end: usize,
) -> bool {
    let mut any = false;
    for (i, rd) in pending.iter().enumerate() {
        if emitted[i] {
            continue;
        }
        if rd.span.start >= container_start && rd.span.start < container_end {
            emit_pending_refdef(builder, cursor, source, rd);
            emitted[i] = true;
            any = true;
        }
    }
    any
}

/// Normalize wrapped-line leading whitespace inside an inline HTML span:
/// micromark drops up to 3 columns of indent at the start of each continuation
/// line (tabs counted as 4-column stops, with any overflow re-emitted as
/// spaces). Returns `None` when the slice has no continuation line that would
/// change.
fn normalize_inline_html_wrap(src: &str) -> Option<String> {
    let bytes = src.as_bytes();
    let first_nl = bytes.iter().position(|&b| b == b'\n' || b == b'\r')?;
    let mut out = String::with_capacity(src.len());
    out.push_str(&src[..first_nl]);
    let mut i = first_nl;
    while i < bytes.len() {
        if bytes[i] == b'\r' {
            out.push('\r');
            i += 1;
            if i < bytes.len() && bytes[i] == b'\n' {
                out.push('\n');
                i += 1;
            }
        } else if bytes[i] == b'\n' {
            out.push('\n');
            i += 1;
        }
        let mut col = 0usize;
        while col < 3 && i < bytes.len() {
            match bytes[i] {
                b' ' => {
                    col += 1;
                    i += 1;
                }
                b'\t' => {
                    let tab_cols = 4 - (col % 4);
                    if col + tab_cols <= 3 {
                        col += tab_cols;
                        i += 1;
                    } else {
                        let leftover = col + tab_cols - 3;
                        for _ in 0..leftover {
                            out.push(' ');
                        }
                        i += 1;
                        col = 3;
                    }
                }
                _ => break,
            }
        }
        let line_start = i;
        while i < bytes.len() && bytes[i] != b'\n' && bytes[i] != b'\r' {
            i += 1;
        }
        out.push_str(&src[line_start..i]);
    }
    if out == src {
        None
    } else {
        Some(out)
    }
}

fn reference_end(
    source: &str,
    cursor: &mut satteri_arena::LineIndexCursor<'_, '_>,
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
/// Extract the verbatim text between `[` and the matching `]` for a link
/// reference definition, treating `\X` as a 2-byte escape (so an escaped `]`
/// inside the label doesn't terminate the scan).
/// Resolve `\X` escape sequences for ASCII punctuation in a definition or
/// reference label, matching remark's behaviour. Returns `None` when there's
/// nothing to change so the caller can keep using the source slice directly.
fn unescape_label_backslashes(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    if !bytes.contains(&b'\\') {
        return None;
    }
    let mut out = String::with_capacity(s.len());
    let mut last = 0;
    let mut i = 0;
    let mut changed = false;
    while i < bytes.len() {
        if bytes[i] == b'\\'
            && i + 1 < bytes.len()
            && crate::puncttable::is_ascii_punctuation(bytes[i + 1])
        {
            out.push_str(&s[last..i]);
            out.push(bytes[i + 1] as char);
            i += 2;
            last = i;
            changed = true;
        } else {
            i += 1;
        }
    }
    if !changed {
        return None;
    }
    out.push_str(&s[last..]);
    Some(out)
}

fn extract_definition_label(source: &str, start: u32) -> Option<&str> {
    let bytes = source.as_bytes();
    let open = start as usize;
    if open >= bytes.len() || bytes[open] != b'[' {
        return None;
    }
    let mut i = open + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if i + 1 < bytes.len() => i += 2,
            b']' => return Some(&source[open + 1..i]),
            _ => i += 1,
        }
    }
    None
}

fn extract_reference_label(source: &str, start: u32, end: u32, kind: u8, is_image: bool) -> &str {
    let bytes = source.as_bytes();
    let inner_start = if is_image {
        start as usize + 2
    } else {
        start as usize + 1
    };
    if kind == 2 {
        // Full `[text][label]`: walk back from `end` (past closing `]`) to
        // find the matching `[`. Skip escaped `\[` — a bracket is escaped
        // when preceded by an odd number of backslashes.
        let close2 = end as usize - 1; // position of the second `]`
        let mut open2 = close2;
        while open2 > inner_start {
            if bytes[open2 - 1] == b'[' {
                let mut bs = 0usize;
                let mut k = open2 - 1;
                while k > inner_start && bytes[k - 1] == b'\\' {
                    bs += 1;
                    k -= 1;
                }
                if bs.is_multiple_of(2) {
                    break;
                }
            }
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

fn encode_jsx_element_data(jsx: &JsxElementData<'_>, builder: &mut ArenaBuilder<Mdast>) -> Vec<u8> {
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

    encode_mdx_jsx_element_data(name_ref, &attr_tuples, true)
}
