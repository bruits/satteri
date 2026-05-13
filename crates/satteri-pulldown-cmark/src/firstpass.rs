//! The first pass resolves all block structure, generating an AST. Within a block, items
//! are in a linear chain with potential inline markup identified.

use alloc::{string::String, vec::Vec};
use core::{cmp::max, ops::Range};

use unicase::UniCase;

use crate::{
    linklabel::{scan_link_label_rest, LinkLabel},
    mdx::*,
    parse::{
        scan_containers, Allocations, DirectiveAttrData, FootnoteDef, HeadingAttributes, Item,
        ItemBody, LinkDef, LINK_MAX_NESTED_PARENS,
    },
    scanners::*,
    strings::CowStr,
    tree::{Tree, TreeIndex},
    HeadingLevel, MetadataBlockKind, Options,
};

/// Runs the first pass, which resolves the block structure of the document,
/// and returns the resulting tree.
pub(crate) fn run_first_pass(
    text: &str,
    options: Options,
) -> (Tree<Item>, Allocations<'_>, Vec<(usize, String)>) {
    // This is a very naive heuristic for the number of nodes
    // we'll need.
    let start_capacity = max(128, text.len() / 32);
    let lookup_table = &create_lut(&options);
    let first_pass = FirstPass {
        text,
        tree: Tree::with_capacity(start_capacity),
        begin_list_item: None,
        last_line_blank: false,
        list_interrupted_paragraph: false,
        allocs: Allocations::new(),
        options,
        lookup_table,
        brace_context_next: 0,
        brace_context_stack: Vec::new(),
        mdx_errors: Vec::new(),
    };
    first_pass.run()
}

// Each level of brace nesting adds another entry to a hash table.
// To limit the amount of memory consumed, do not create a new brace
// context beyond some amount deep.
//
// There are actually two limits at play here: this one,
// and the one where the maximum amount of distinct contexts passes
// the 255 item limit imposed by using `u8`. When over 255 distinct
// contexts are created, it wraps around, while this one instead makes it
// saturate, which is a better behavior.
const MATH_BRACE_CONTEXT_MAX_NESTING: usize = 25;

/// State for the first parsing pass.
pub(crate) struct FirstPass<'a, 'b> {
    pub(crate) text: &'a str,
    pub(crate) tree: Tree<Item>,
    begin_list_item: Option<usize>,
    last_line_blank: bool,
    list_interrupted_paragraph: bool,
    pub(crate) allocs: Allocations<'a>,
    pub(crate) options: Options,
    lookup_table: &'b LookupTable,
    /// Math environment brace nesting.
    brace_context_stack: Vec<u8>,
    brace_context_next: usize,
    /// MDX errors collected during first pass.
    pub(crate) mdx_errors: Vec<(usize, String)>,
}

impl<'a, 'b> FirstPass<'a, 'b> {
    fn run(mut self) -> (Tree<Item>, Allocations<'a>, Vec<(usize, String)>) {
        let mut ix = 0;
        while ix < self.text.len() {
            ix = self.parse_block(ix);
        }
        while self.tree.spine_len() > 0 {
            self.pop(ix);
        }
        (self.tree, self.allocs, self.mdx_errors)
    }

    /// Returns offset after block.
    fn parse_block(&mut self, mut start_ix: usize) -> usize {
        let bytes = self.text.as_bytes();
        let mut line_start = LineStart::new(&bytes[start_ix..]);

        // math spans and their braces are tracked only within a single block
        self.brace_context_stack.clear();
        self.brace_context_next = 0;

        let i = scan_containers(&self.tree, &mut line_start, self.options);
        if i < self.tree.spine_len() {
            self.list_interrupted_paragraph = false;
        }
        for _ in i..self.tree.spine_len() {
            self.pop(start_ix);
        }

        // Before processing new containers: if we arrive here with a
        // non-blank line pending, the previous line was a non-trailing blank
        // that belongs to the currently-open list item. Mark the item as
        // spread (loose). Must happen before the new-containers loop, which
        // could open a deeper nested list item and shift the spine tip.
        if self.last_line_blank {
            let content_probe = start_ix + line_start.bytes_scanned();
            let has_content =
                content_probe < bytes.len() && scan_blank_line(&bytes[content_probe..]).is_none();
            if has_content {
                if let Some(up) = self.tree.peek_up() {
                    if let ItemBody::ListItem(_, _) = self.tree[up].item.body {
                        if self.tree[up].child.is_some() {
                            self.mark_enclosing_listitem_spread();
                        }
                    }
                }
            }
        }

        // Process new containers
        loop {
            let save = line_start.clone();
            let mut outer_indent = line_start.scan_space_upto(4);
            if outer_indent >= 4 {
                if self.options.contains(Options::ENABLE_MDX) {
                    // MDX has no indented code blocks.  Speculatively scan all
                    // remaining whitespace to look for a new container (list
                    // marker, blockquote, or — when enabled — a directive fence)
                    // at deeper indentation. If no new container is found we
                    // restore and break so that the indentation is preserved for
                    // content that belongs to an existing list item.
                    let extra = line_start.scan_space_upto(usize::MAX);
                    let mdx_ix = start_ix + line_start.bytes_scanned();
                    let has_directive = self.options.contains(Options::ENABLE_DIRECTIVE)
                        && scan_ch_repeat(&bytes[mdx_ix..], b':') > 1;
                    let has_container = scan_listitem(&bytes[mdx_ix..]).is_some()
                        || (mdx_ix < bytes.len() && bytes[mdx_ix] == b'>')
                        || has_directive;
                    if !has_container {
                        // Tables are detected later in parse_paragraph when
                        // the table opener (`|`) is the first non-whitespace
                        // char on the line. Don't break here if this line
                        // could be a table start — restore only the outer
                        // indent and let paragraph-level parsing continue
                        // so scan_paragraph_table sees it.
                        if self.options.contains(Options::ENABLE_TABLES)
                            && mdx_ix < bytes.len()
                            && bytes[mdx_ix] == b'|'
                        {
                            // keep the whitespace we consumed; break so we
                            // advance into paragraph parsing with the pipe
                            // as the first non-whitespace byte.
                            break;
                        }
                        line_start = save;
                        break;
                    }
                    // The deeper list marker's continuation indent must account
                    // for the whitespace we just consumed past the initial 4.
                    outer_indent += extra;
                } else {
                    line_start = save;
                    break;
                }
            }
            if self.options.contains(Options::ENABLE_FOOTNOTES) {
                // Footnote definitions
                let container_start = start_ix + line_start.bytes_scanned();
                if let Some(bytecount) = self.parse_footnote(container_start) {
                    start_ix = container_start + bytecount;
                    line_start = LineStart::new(&bytes[start_ix..]);
                    continue;
                }
            }
            let container_start = start_ix + line_start.bytes_scanned();
            if let Some((ch, index, indent)) = line_start.scan_list_marker_with_indent(outer_indent)
            {
                let after_marker_index = start_ix + line_start.bytes_scanned();
                let already_in_list = self
                    .tree
                    .peek_up()
                    .is_some_and(|ix| matches!(self.tree[ix].item.body, ItemBody::List(_, _, _)));
                let after_marker_blank = {
                    let rest = &bytes[after_marker_index..];
                    rest.is_empty() || scan_blank_line(rest).is_some()
                };
                if self.list_interrupted_paragraph && !already_in_list && after_marker_blank {
                    self.list_interrupted_paragraph = false;
                    line_start = save;
                    break;
                }
                self.continue_list(container_start, ch, index);
                self.tree.append(Item {
                    start: container_start,
                    end: after_marker_index, // will get updated later if item not empty
                    body: ItemBody::ListItem(indent, false),
                });
                self.tree.push();
                if let Some(n) = scan_blank_line(&bytes[after_marker_index..]) {
                    self.begin_list_item = Some(after_marker_index + n);
                    return after_marker_index + n;
                }
                if self.options.contains(Options::ENABLE_TASKLISTS) {
                    let saved_line_start = line_start.clone();
                    let task_list_marker =
                        line_start.scan_task_list_marker().map(|is_checked| Item {
                            start: after_marker_index,
                            end: start_ix + line_start.bytes_scanned(),
                            body: ItemBody::TaskListMarker(is_checked),
                        });
                    if let Some(task_list_marker) = task_list_marker {
                        let rest = &bytes[task_list_marker.end..];
                        let marker_ate_newline = matches!(
                            bytes.get(task_list_marker.end.wrapping_sub(1)),
                            Some(b'\n' | b'\r')
                        );
                        // Skip spaces/tabs on the rest of the marker line to
                        // find the first real content (if any).
                        let trailing_ws = rest
                            .iter()
                            .position(|&b| b != b' ' && b != b'\t')
                            .unwrap_or(rest.len());
                        let after_ws = &rest[trailing_ws..];
                        let rest_of_line_blank =
                            after_ws.is_empty() || matches!(after_ws.first(), Some(b'\n' | b'\r'));
                        // When the rest of the marker line is blank, see if the
                        // next line has lazy-continuation content we should
                        // attach to the task item's paragraph.
                        //
                        // `marker_ate_newline=true` means the scanner already
                        // consumed the newline as the trailing whitespace, so
                        // the marker line ended right after `]` and `after_ws`
                        // is on the next line — handle it the same as a blank
                        // marker tail.
                        let lazy_continuation_start = if marker_ate_newline {
                            let start = task_list_marker.end + trailing_ws;
                            (start < bytes.len()
                                && scan_blank_line(&bytes[start..]).is_none()
                                && !scan_paragraph_interrupt_no_table(
                                    &bytes[start..],
                                    true,
                                    self.options.contains(Options::ENABLE_FOOTNOTES),
                                    self.options.contains(Options::ENABLE_DEFINITION_LIST),
                                    self.options.contains(Options::ENABLE_MDX),
                                    self.options.contains(Options::ENABLE_MATH),
                                    &self.tree,
                                    self.tree.spine_len(),
                                ))
                            .then_some(start)
                        } else if rest_of_line_blank {
                            let newline_len = scan_eol(after_ws).unwrap_or(0);
                            let start = task_list_marker.end + trailing_ws + newline_len;
                            (newline_len > 0
                                && start < bytes.len()
                                && scan_blank_line(&bytes[start..]).is_none()
                                && !scan_paragraph_interrupt_no_table(
                                    &bytes[start..],
                                    true,
                                    self.options.contains(Options::ENABLE_FOOTNOTES),
                                    self.options.contains(Options::ENABLE_DEFINITION_LIST),
                                    self.options.contains(Options::ENABLE_MDX),
                                    self.options.contains(Options::ENABLE_MATH),
                                    &self.tree,
                                    self.tree.spine_len(),
                                ))
                            .then_some(start)
                        } else {
                            None
                        };
                        if let Some(new_start) = lazy_continuation_start {
                            return self.parse_paragraph(new_start, Some(task_list_marker));
                        } else if rest_of_line_blank || marker_ate_newline {
                            // No paragraph content found for the task item:
                            // either the next line is a paragraph interrupt
                            // (so it can't lazily continue the task item) or
                            // the next line is blank.
                            line_start = saved_line_start;
                        } else {
                            return self
                                .parse_paragraph(task_list_marker.end, Some(task_list_marker));
                        }
                    }
                }
            } else if let Some((indent, child, item)) = self
                .options
                .contains(Options::ENABLE_DEFINITION_LIST)
                .then(|| {
                    self.tree
                        .cur()
                        .map(|cur| (self.tree[cur].child, &mut self.tree[cur].item))
                })
                .flatten()
                .filter(|(_, item)| {
                    matches!(
                        item,
                        Item {
                            body: ItemBody::Paragraph
                                | ItemBody::TightParagraph
                                | ItemBody::MaybeDefinitionListTitle
                                | ItemBody::DefinitionListDefinition(_),
                            ..
                        }
                    )
                })
                .and_then(|item| {
                    Some((
                        line_start
                            .scan_definition_list_definition_marker_with_indent(outer_indent)?,
                        item.0,
                        item.1,
                    ))
                })
            {
                match item.body {
                    ItemBody::Paragraph | ItemBody::TightParagraph => {
                        item.body = ItemBody::DefinitionList(true);
                        let Item { start, end, .. } = *item;
                        let list_idx = self.tree.cur().unwrap();
                        let title_idx = self.tree.create_node(Item {
                            start,
                            end, // will get updated later if item not empty
                            body: ItemBody::DefinitionListTitle,
                        });
                        self.tree[title_idx].child = child;
                        self.tree[list_idx].child = Some(title_idx);
                        self.tree.push();
                    }
                    ItemBody::MaybeDefinitionListTitle => {
                        item.body = ItemBody::DefinitionListTitle;
                    }
                    ItemBody::DefinitionListDefinition(_) => {}
                    _ => unreachable!(),
                }
                let after_marker_index = start_ix + line_start.bytes_scanned();
                self.tree.append(Item {
                    start: container_start - outer_indent,
                    end: after_marker_index, // will get updated later if item not empty
                    body: ItemBody::DefinitionListDefinition(indent),
                });
                if let Some(ItemBody::DefinitionList(ref mut is_tight)) =
                    self.tree.peek_up().map(|cur| &mut self.tree[cur].item.body)
                {
                    if self.last_line_blank {
                        *is_tight = false;
                        self.last_line_blank = false;
                    }
                }
                self.tree.push();
                if let Some(n) = scan_blank_line(&bytes[after_marker_index..]) {
                    self.begin_list_item = Some(after_marker_index + n);
                    return after_marker_index + n;
                }
            } else if line_start.scan_blockquote_marker() {
                let kind = if self.options.contains(Options::ENABLE_GITHUB_ALERTS) {
                    line_start.scan_blockquote_tag()
                } else {
                    None
                };
                self.finish_list(start_ix);
                self.tree.append(Item {
                    start: container_start,
                    end: 0, // will get set later
                    body: ItemBody::BlockQuote(kind),
                });
                self.tree.push();
                if kind.is_some() {
                    // blockquote tag leaves us at the end of the line
                    // we need to scan through all the container syntax for the next line
                    // and break out if we can't re-scan all of them
                    let ix = start_ix + line_start.bytes_scanned();
                    let mut lazy_line_start = LineStart::new(&bytes[ix..]);
                    let tree_position =
                        scan_containers(&self.tree, &mut lazy_line_start, self.options);
                    let current_container = tree_position == self.tree.spine_len();
                    if !lazy_line_start.scan_space(4)
                        && self.scan_paragraph_interrupt(
                            &bytes[ix + lazy_line_start.bytes_scanned()..],
                            current_container,
                            tree_position,
                        )
                    {
                        return ix;
                    } else {
                        // blockquote tags act as if they were nested in a paragraph
                        // so you can lazily continue the imaginary paragraph off of them
                        line_start = lazy_line_start;
                        line_start.scan_all_space();
                        start_ix = ix;
                        break;
                    }
                }
            } else if self.options.contains(Options::ENABLE_DIRECTIVE)
                && scan_ch_repeat(&bytes[(start_ix + line_start.bytes_scanned())..], b':') > 1
            {
                let colon_start = start_ix + line_start.bytes_scanned();
                let colon_count = scan_ch_repeat(&bytes[colon_start..], b':');
                if colon_count >= 3 && self.tree.spine_len() <= u8::MAX as usize {
                    // Container directive (:::+)
                    let fence_length = core::cmp::min(colon_count, u8::MAX as usize);
                    let after_colons = colon_start + colon_count;
                    if let Some((dir_data, content_end)) =
                        parse_directive_after_colons(self.text, bytes, after_colons)
                    {
                        // Close any open list before opening a sibling directive,
                        // matching how blockquote handles the same transition.
                        self.finish_list(start_ix);
                        // For block directives, advance to end of line
                        let after = &bytes[content_end..];
                        let ws = scan_whitespace_no_nl(after);
                        let line_end = content_end + ws + scan_nextline(&after[ws..]);
                        let dir_ix = self.allocs.allocate_directive(dir_data);
                        self.tree.append(Item {
                            start: container_start,
                            end: 0,
                            body: ItemBody::ContainerDirective(fence_length as u8, dir_ix),
                        });
                        self.tree.push();
                        return line_end;
                    } else {
                        break;
                    }
                } else if colon_count == 2 {
                    // Leaf directive (::)
                    let after_colons = colon_start + 2;
                    if let Some((dir_data, line_end)) =
                        parse_directive_after_colons(self.text, bytes, after_colons)
                    {
                        // Verify only whitespace follows on the line
                        let remaining = &bytes[line_end..];
                        let ws = scan_whitespace_no_nl(remaining);
                        let at_eol = line_end + ws >= bytes.len()
                            || bytes[line_end + ws] == b'\n'
                            || bytes[line_end + ws] == b'\r';
                        if at_eol || line_end >= bytes.len() {
                            self.finish_list(start_ix);
                            let dir_ix = self.allocs.allocate_directive(dir_data);
                            self.tree.append(Item {
                                start: container_start,
                                end: line_end,
                                body: ItemBody::LeafDirective(dir_ix),
                            });
                            let next_line = line_end + scan_nextline(&bytes[line_end..]);
                            return next_line;
                        }
                    }
                    break;
                } else {
                    break;
                }
            } else {
                line_start = save;
                break;
            }
        }

        if self.options.contains(Options::ENABLE_DIRECTIVE) {
            let mut pop_count = None;
            let mut fence_line_end = start_ix;
            // Closing fence may be indented up to 3 spaces relative to the
            // current container cursor — matches remark-directive. Try
            // matching against each enclosing ContainerDirective; if the
            // speculative space consumption doesn't lead to a match, restore
            // line_start before continuing with normal block parsing.
            let fence_save = line_start.clone();
            let _ = line_start.scan_space_upto(3);
            let mut matched_length: Option<u8> = None;
            for (i, &node_ix) in self.tree.walk_spine().rev().enumerate() {
                match self.tree[node_ix].item.body {
                    ItemBody::ContainerDirective(length, ..) => {
                        let probe = line_start.clone();
                        if line_start.scan_closing_container_extensions_fence(length) {
                            let after_fence = start_ix + line_start.bytes_scanned();
                            fence_line_end = after_fence + scan_nextline(&bytes[after_fence..]);
                            pop_count = Some(i + 1);
                            matched_length = Some(length);
                            break;
                        }
                        line_start = probe;
                    }
                    ItemBody::List(..) | ItemBody::ListItem(..) => {}
                    _ => break,
                }
            }
            if pop_count.is_none() {
                line_start = fence_save;
            }

            if let Some(mut c) = pop_count {
                // remark-directive closes a contiguous run of same-length
                // container directives on a single closing `:::` — the fence
                // line is shared between the innermost open directive and
                // all of its ancestors that use the same fence length. Walk
                // further up the spine and bump the pop count accordingly.
                if let Some(length) = matched_length {
                    let spine: Vec<TreeIndex> = self.tree.walk_spine().copied().collect();
                    if c < spine.len() {
                        for &ancestor_ix in spine.iter().rev().skip(c) {
                            match self.tree[ancestor_ix].item.body {
                                ItemBody::ContainerDirective(other_length, ..)
                                    if other_length == length =>
                                {
                                    c += 1;
                                }
                                ItemBody::List(..) | ItemBody::ListItem(..) => {}
                                _ => break,
                            }
                        }
                    }
                }
                for _ in 0..c {
                    self.pop(fence_line_end);
                }
                return fence_line_end;
            }
        }
        let ix = start_ix + line_start.bytes_scanned();

        if let Some(n) = scan_blank_line(&bytes[ix..]) {
            if let Some(node_ix) = self.tree.peek_up() {
                match &mut self.tree[node_ix].item.body {
                    ItemBody::ContainerDirective(..) => {
                        // Blank lines inside a container directive propagate
                        // looseness to the enclosing list item — matching
                        // remark-directive's behavior where a directive with
                        // blank-line-separated content makes its ancestor list
                        // item spread.
                        self.mark_enclosing_listitem_spread();
                    }
                    ItemBody::BlockQuote(..) => (),
                    ItemBody::ListItem(indent, _) | ItemBody::DefinitionListDefinition(indent)
                        if self.begin_list_item.is_some() =>
                    {
                        self.last_line_blank = true;
                        if !self.options.contains(Options::ENABLE_MDX) {
                            // This is a blank list item.
                            // While the list itself can be continued no matter how many blank lines
                            // there are between this one and the next one, it cannot nest anything
                            // else, so its indentation should not be subtracted from the line start.
                            // In MDX mode we keep the indent because there are no indented code
                            // blocks, so continuation content at any depth should remain part of
                            // the list item.
                            *indent = 0;
                        }
                    }
                    _ => {
                        self.last_line_blank = true;
                    }
                }
            } else {
                self.last_line_blank = true;
            }
            return ix + n;
        }

        // Save `remaining_space` here to avoid needing to backtrack `line_start` for HTML blocks
        let remaining_space = line_start.remaining_space();
        let content_start_ix = start_ix + line_start.bytes_scanned();

        let mut indent = line_start.scan_space_upto(4);
        if indent == 4 {
            if self.options.contains(Options::ENABLE_MDX) {
                // MDX does not support indented code blocks. Track the full
                // leading whitespace as the indent so that deeply-indented
                // fenced code blocks inside containers (lists, JSX, directives)
                // strip the right amount from their content lines.
                indent += line_start.scan_space_upto(usize::MAX);
            } else {
                self.finish_list(start_ix);
                let ix = start_ix + line_start.bytes_scanned();
                let remaining_space = line_start.remaining_space();
                return self.parse_indented_code_block(content_start_ix, ix, remaining_space);
            }
        }

        let ix = start_ix + line_start.bytes_scanned();

        // Metadata blocks cannot be indented, and — matching remark-frontmatter
        // — only match at the very start of the document.
        if indent == 0 && ix == 0 && self.tree.spine_len() == 0 {
            if let Some((_n, metadata_block_ch)) = scan_metadata_block(
                &bytes[ix..],
                self.options
                    .contains(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS),
                self.options
                    .contains(Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS),
            ) {
                self.finish_list(start_ix);
                return self.parse_metadata_block(ix, metadata_block_ch);
            }
        }

        // MDX blocks, must be checked before HTML blocks since JSX looks like HTML.
        if self.options.contains(Options::ENABLE_MDX) {
            // MDX ESM: lines starting with `import` or `export`.
            // ESM is only valid at the document root (not inside containers).
            if indent == 0 && self.tree.spine_len() == 0 {
                if let Some(end_ix) = scan_mdx_esm(&bytes[ix..]) {
                    let mut final_end = end_ix;

                    // If the scanned ESM block is incomplete (e.g. an export
                    // spanning a blank line), retry across blank lines using
                    // oxc — matching the reference mdxjs behavior.
                    let candidate = self.text[ix..ix + final_end].trim_end();
                    if !candidate.is_empty() {
                        use crate::mdx::EsmParseResult;
                        let mut allocator = oxc_allocator::Allocator::default();
                        match crate::mdx::try_parse_esm(candidate, &mut allocator) {
                            EsmParseResult::Complete => {}
                            EsmParseResult::Incomplete => {
                                let mut pos = ix + final_end;
                                loop {
                                    let blank_start = pos;
                                    while pos < bytes.len()
                                        && (bytes[pos] == b'\n'
                                            || bytes[pos] == b'\r'
                                            || bytes[pos] == b' '
                                            || bytes[pos] == b'\t')
                                    {
                                        pos += 1;
                                    }
                                    if pos == blank_start || pos >= bytes.len() {
                                        break;
                                    }
                                    let chunk_start = pos;
                                    while pos < bytes.len() {
                                        let eol = memchr::memchr(b'\n', &bytes[pos..])
                                            .map(|i| pos + i + 1)
                                            .unwrap_or(bytes.len());
                                        pos = eol;
                                        if pos < bytes.len()
                                            && (bytes[pos] == b'\n' || bytes[pos] == b'\r')
                                        {
                                            break;
                                        }
                                    }
                                    if pos == chunk_start {
                                        break;
                                    }
                                    final_end = pos - ix;
                                    let candidate = self.text[ix..ix + final_end].trim_end();
                                    match crate::mdx::try_parse_esm(candidate, &mut allocator) {
                                        EsmParseResult::Complete => break,
                                        EsmParseResult::Incomplete => continue,
                                        EsmParseResult::Error => break,
                                    }
                                }
                            }
                            EsmParseResult::Error => {}
                        }
                    }

                    self.finish_list(start_ix);
                    return self.parse_mdx_esm(ix, ix + final_end);
                }
            }

            // MDX JSX flow: line starting with `<` followed by component name or fragment
            if bytes[ix] == b'<' {
                if let Some(end_ix) =
                    self.scan_mdx_flow_in_container(ix, |b, c| scan_mdx_jsx_block(b, c))
                {
                    self.finish_list(start_ix);
                    let result = self.parse_mdx_jsx_flow(ix, ix + end_ix);
                    // A blank line inside the consumed flow block means the
                    // enclosing list item is "loose" (spread=true). remark
                    // detects this naturally since its tokenizer reads line-
                    // by-line; we consume the whole block atomically, so we
                    // have to inspect the span here.
                    if contains_blank_line(&bytes[ix..ix + end_ix]) {
                        self.last_line_blank = true;
                        self.mark_enclosing_listitem_spread();
                    }
                    return result;
                }
            }

            // MDX expression flow: line starting with `{`
            if bytes[ix] == b'{' {
                if let Some(end_ix) =
                    self.scan_mdx_flow_in_container(ix, |b, c| scan_mdx_expression_block(b, c))
                {
                    self.finish_list(start_ix);
                    let result = self.parse_mdx_jsx_flow(ix, ix + end_ix);
                    if contains_blank_line(&bytes[ix..ix + end_ix]) {
                        self.last_line_blank = true;
                        self.mark_enclosing_listitem_spread();
                    }
                    return result;
                }
                // If the inline scanner also can't find a closing `}`, it's truly unclosed.
                // (If it CAN find one, the `{` will be handled as inline in a paragraph.)
                if scan_mdx_inline_expression(&bytes[ix..]).is_none() {
                    self.mdx_errors.push((
                        ix,
                        "Unexpected end of file in expression, expected a corresponding \
                         closing brace for `{`"
                            .to_string(),
                    ));
                }
            }
        }

        // HTML Blocks, completely disabled in MDX mode (all tags are JSX).
        if bytes[ix] == b'<' && !self.options.contains(Options::ENABLE_MDX) {
            // Types 1-5 are all detected by one function and all end with the same
            // pattern
            if let Some(html_end_tag) = get_html_end_tag(&bytes[(ix + 1)..]) {
                self.finish_list(start_ix);
                return self.parse_html_block_type_1_to_5(
                    content_start_ix,
                    html_end_tag,
                    remaining_space,
                    indent,
                );
            }

            // Detect type 6
            if starts_html_block_type_6(&bytes[(ix + 1)..]) {
                self.finish_list(start_ix);
                return self.parse_html_block_type_6_or_7(
                    content_start_ix,
                    remaining_space,
                    indent,
                );
            }

            // Detect type 7
            if let Some(_html_bytes) = scan_html_type_7(&bytes[ix..]) {
                self.finish_list(start_ix);
                return self.parse_html_block_type_6_or_7(
                    content_start_ix,
                    remaining_space,
                    indent,
                );
            }
        }

        if let Ok(n) = scan_hrule(&bytes[ix..]) {
            self.finish_list(start_ix);
            return self.parse_hrule(n, ix);
        }

        if let Some(atx_size) = scan_atx_heading(&bytes[ix..]) {
            self.finish_list(start_ix);
            return self.parse_atx_heading(ix, atx_size);
        }

        if let Some((n, fence_ch)) = scan_code_fence(&bytes[ix..]) {
            self.finish_list(start_ix);
            return self.parse_fenced_code_block(ix, indent, fence_ch, n);
        }

        if self.options.contains(Options::ENABLE_MATH) {
            if let Some(n) = scan_math_fence(&bytes[ix..]) {
                self.finish_list(start_ix);
                return self.parse_math_block(ix, indent, n);
            }
        }

        // parse refdef
        while let Some((bytecount, label, link_def)) =
            self.parse_refdef_total(start_ix + line_start.bytes_scanned())
        {
            self.allocs.refdefs.0.entry(label).or_insert(link_def);
            let container_start = start_ix + line_start.bytes_scanned();
            let mut ix = container_start + bytecount;
            // Refdefs act as if they were contained within a paragraph, for purposes of lazy
            // continuations. For example:
            //
            // ```
            // > [foo]: http://example.com
            // bar
            // ```
            //
            // is equivalent to
            //
            // ```
            // > bar
            //
            // [foo]: http://example.com
            // ```
            if let Some(nl) = scan_blank_line(&bytes[ix..]) {
                ix += nl;
            } else {
                self.finish_list(start_ix);
                return ix;
            }
            if let Some(lazy_line_start) = self.scan_next_line_or_lazy_continuation(&bytes[ix..]) {
                line_start = lazy_line_start;
                start_ix = ix;
            } else {
                self.finish_list(start_ix);
                return ix;
            }
        }

        let ix = start_ix + line_start.bytes_scanned();

        self.parse_paragraph(ix, None)
    }

    /// footnote definitions and GFM quote markers can be "interrupted"
    /// like paragraphs, but otherwise can't have other blocks after them.
    ///
    /// Call this at the end of the line to parse that. If it succeeeds,
    /// this returns the LineStart for the new line.
    fn scan_next_line_or_lazy_continuation<'input>(
        &mut self,
        bytes: &'input [u8],
    ) -> Option<LineStart<'input>> {
        let mut line_start = LineStart::new(bytes);
        let tree_position = scan_containers(&self.tree, &mut line_start, self.options);
        let current_container = tree_position == self.tree.spine_len();
        if self.options.contains(Options::ENABLE_MDX) {
            // MDX: indented code blocks are disabled, so consume all leading
            // whitespace and always check for paragraph interrupts.  This
            // ensures deeply-indented code fences interrupt paragraphs.
            line_start.scan_all_space();
            if self.scan_paragraph_interrupt(
                &bytes[line_start.bytes_scanned()..],
                current_container,
                tree_position,
            ) || scan_blank_line(&bytes[line_start.bytes_scanned()..]).is_some()
            {
                None
            } else {
                Some(line_start)
            }
        } else if (!line_start.scan_space(4)
            && self.scan_paragraph_interrupt(
                &bytes[line_start.bytes_scanned()..],
                current_container,
                tree_position,
            ))
            || scan_blank_line(&bytes[line_start.bytes_scanned()..]).is_some()
        {
            None
        } else {
            line_start.scan_all_space();
            Some(line_start)
        }
    }

    /// Returns the offset of the first line after the table.
    /// Assumptions: current focus is a table element and the table header
    /// matches the separator line (same number of columns).
    fn parse_table(
        &mut self,
        table_cols: usize,
        head_start: usize,
        body_start: usize,
    ) -> Option<usize> {
        // filled empty cells are limited to protect against quadratic growth
        // https://github.com/raphlinus/pulldown-cmark/issues/832
        let mut missing_empty_cells = 0;
        // parse header. this shouldn't fail because we made sure the table header is ok
        let (_sep_start, thead_ix) =
            self.parse_table_row_inner(head_start, table_cols, &mut missing_empty_cells)?;
        self.tree[thead_ix].item.body = ItemBody::TableHead;

        // parse body
        let mut ix = body_start;
        while let Some((next_ix, _row_ix)) =
            self.parse_table_row(ix, table_cols, &mut missing_empty_cells)
        {
            ix = next_ix;
        }

        self.pop(ix);
        Some(ix)
    }

    /// Call this when containers are taken care of.
    /// Returns bytes scanned, row_ix
    fn parse_table_row_inner(
        &mut self,
        mut ix: usize,
        row_cells: usize,
        missing_empty_cells: &mut usize,
    ) -> Option<(usize, TreeIndex)> {
        let bytes = self.text.as_bytes();
        let mut cells = 0;
        let mut final_cell_ix = None;

        let old_cur = self.tree.cur();
        let row_ix = self.tree.append(Item {
            start: ix,
            end: 0, // set at end of this function
            body: ItemBody::TableRow,
        });
        self.tree.push();

        let mut first_iter = true;
        let mut saw_opening_pipe = false;
        loop {
            let cell_start = ix;
            let pipe_consumed = scan_ch(&bytes[ix..], b'|');
            ix += pipe_consumed;
            if first_iter && pipe_consumed > 0 {
                saw_opening_pipe = true;
            }
            first_iter = false;
            let _start_ix = ix;
            ix += scan_whitespace_no_nl(&bytes[ix..]);

            if let Some(eol_bytes) = scan_eol(&bytes[ix..]) {
                // A line with only an opening `|` and no content (e.g. stray
                // `              |` between table rows) emits a row with a
                // single empty cell in remark-gfm instead of terminating the
                // table. Mirror that here.
                if saw_opening_pipe && cells == 0 {
                    let empty_cell_ix = self.tree.append(Item {
                        start: cell_start,
                        end: ix,
                        body: ItemBody::TableCell,
                    });
                    final_cell_ix = Some(empty_cell_ix);
                    cells = 1;
                }
                ix += eol_bytes;
                break;
            }

            let cell_ix = self.tree.append(Item {
                start: cell_start,
                end: ix,
                body: ItemBody::TableCell,
            });
            self.tree.push();
            let (next_ix, _brk) = self.parse_line(ix, None, TableParseMode::Active);

            self.tree[cell_ix].item.end = next_ix;
            self.tree.pop();

            ix = next_ix;
            cells += 1;

            if cells == row_cells {
                final_cell_ix = Some(cell_ix);
            }
        }

        if let (Some(cur), 0) = (old_cur, cells) {
            self.pop(ix);
            self.tree[cur].next = None;
            return None;
        }

        // Don't fill missing cells in MDAST: GFM HTML rendering pads rows to
        // the header width, but `mdast-util-gfm-table` keeps the source cell
        // count (HAST padding happens downstream in `mdast-util-to-hast`).
        // Match remark-gfm's mdast shape.
        let _ = row_cells;
        let _ = missing_empty_cells;

        // Extend the last cell's end to include the trailing `|` and
        // whitespace, matching remark's convention.
        if let Some(cell_ix) = final_cell_ix {
            let row_end = ix;
            let mut cell_end = self.tree[cell_ix].item.end;
            let bytes = self.text.as_bytes();
            while cell_end < row_end
                && cell_end < bytes.len()
                && bytes[cell_end] != b'\n'
                && bytes[cell_end] != b'\r'
            {
                cell_end += 1;
            }
            self.tree[cell_ix].item.end = cell_end;
        }

        // drop excess cells
        if let Some(cell_ix) = final_cell_ix {
            self.tree[cell_ix].next = None;
        }

        self.pop(ix);

        Some((ix, row_ix))
    }

    /// Returns first offset after the row and the tree index of the row.
    fn parse_table_row(
        &mut self,
        mut ix: usize,
        row_cells: usize,
        missing_empty_cells: &mut usize,
    ) -> Option<(usize, TreeIndex)> {
        let bytes = self.text.as_bytes();
        let mut line_start = LineStart::new(&bytes[ix..]);
        let tree_position = scan_containers(&self.tree, &mut line_start, self.options);
        let current_container = tree_position == self.tree.spine_len();
        if !current_container {
            return None;
        }
        line_start.scan_all_space();
        ix += line_start.bytes_scanned();
        if scan_paragraph_interrupt_no_table(
            &bytes[ix..],
            current_container,
            self.options.contains(Options::ENABLE_FOOTNOTES),
            self.options.contains(Options::ENABLE_DEFINITION_LIST),
            self.options.contains(Options::ENABLE_MDX),
            self.options.contains(Options::ENABLE_MATH),
            &self.tree,
            tree_position,
        ) {
            return None;
        }

        let (ix, row_ix) = self.parse_table_row_inner(ix, row_cells, missing_empty_cells)?;
        Some((ix, row_ix))
    }

    /// Returns offset of line start after paragraph.
    fn parse_paragraph(&mut self, start_ix: usize, tasklist_marker: Option<Item>) -> usize {
        self.list_interrupted_paragraph = false;
        let body = if let Some(ItemBody::DefinitionList(_)) =
            self.tree.peek_up().map(|idx| self.tree[idx].item.body)
        {
            if self.tree.cur().is_none_or(|idx| {
                matches!(
                    &self.tree[idx].item.body,
                    ItemBody::DefinitionListDefinition(..)
                )
            }) {
                // blank lines between the previous definition and this one don't count
                self.last_line_blank = false;
                ItemBody::MaybeDefinitionListTitle
            } else {
                self.finish_list(start_ix);
                ItemBody::Paragraph
            }
        } else {
            self.finish_list(start_ix);
            ItemBody::Paragraph
        };
        let node_ix = self.tree.append(Item {
            start: start_ix,
            end: 0, // will get set later
            body,
        });
        self.tree.push();

        if let Some(item) = tasklist_marker {
            self.tree.append(item);
        }

        let bytes = self.text.as_bytes();
        let mut ix = start_ix;
        loop {
            let scan_mode = if self.options.contains(Options::ENABLE_TABLES) && ix == start_ix {
                TableParseMode::Scan
            } else {
                TableParseMode::Disabled
            };
            let (next_ix, brk) = self.parse_line(ix, None, scan_mode);

            // break out when we find a table
            if let Some(Item {
                body: ItemBody::Table(alignment_ix),
                ..
            }) = brk
            {
                let table_cols = self.allocs[alignment_ix].len();
                self.tree[node_ix].item.body = ItemBody::Table(alignment_ix);
                // this clears out any stuff we may have appended - but there may
                // be a cleaner way
                self.tree[node_ix].child = None;
                self.tree.pop();
                if body == ItemBody::MaybeDefinitionListTitle {
                    self.finish_list(ix);
                }
                self.tree.push();
                if let Some(ix) = self.parse_table(table_cols, ix, next_ix) {
                    return ix;
                }
            }

            ix = next_ix;
            let mut line_start = LineStart::new(&bytes[ix..]);
            let tree_position = scan_containers(&self.tree, &mut line_start, self.options);
            let current_container = tree_position == self.tree.spine_len();

            let trailing_backslash_pos = match brk {
                Some(Item {
                    start,
                    body: ItemBody::HardBreak(true),
                    ..
                }) if bytes[start] == b'\\' => Some(start),
                _ => None,
            };
            // In MDX mode, indented code blocks are disabled, so consume all
            // leading whitespace and always check for paragraph interrupts.
            let is_indented = if self.options.contains(Options::ENABLE_MDX) {
                line_start.scan_all_space();
                false
            } else {
                line_start.scan_space(4)
            };
            if !is_indented {
                let ix_new = ix + line_start.bytes_scanned();
                if current_container {
                    if let Some(ix_setext) =
                        self.parse_setext_heading(ix_new, node_ix, trailing_backslash_pos.is_some())
                    {
                        if let Some(pos) = trailing_backslash_pos {
                            self.tree.append_text(pos, pos + 1, false);
                        }
                        self.pop(ix_setext);
                        if body == ItemBody::MaybeDefinitionListTitle {
                            self.finish_list(ix);
                        }
                        return ix_setext;
                    }
                }
                // first check for non-empty lists, then for other interrupts
                let suffix = &bytes[ix_new..];
                if self.scan_paragraph_interrupt(suffix, current_container, tree_position) {
                    if let Some(pos) = trailing_backslash_pos {
                        self.tree.append_text(pos, pos + 1, false);
                    }
                    self.list_interrupted_paragraph =
                        scan_listitem(suffix).is_some() || scan_blockquote_start(suffix).is_some();
                    break;
                }
                if self.options.contains(Options::ENABLE_DIRECTIVE)
                    && !current_container
                    && line_start.scan_closing_container_extensions_fence(3)
                {
                    break;
                }
            }
            line_start.scan_all_space();
            if line_start.is_at_eol() {
                if let Some(pos) = trailing_backslash_pos {
                    self.tree.append_text(pos, pos + 1, false);
                }
                break;
            }

            if self.options.contains(Options::ENABLE_DIRECTIVE) {
                let mut closes = false;
                for &node_ix in self.tree.walk_spine().rev().skip(1) {
                    match self.tree[node_ix].item.body {
                        ItemBody::ContainerDirective(length, ..) => {
                            let probe = line_start.clone();
                            if line_start.scan_closing_container_extensions_fence(length) {
                                closes = true;
                                break;
                            }
                            line_start = probe;
                        }
                        // Walk past list/listItem — closing `:::` can sit at the
                        // list's indent level and still close an outer directive
                        // (matches remark-directive). A blockquote on the spine,
                        // by contrast, hides an inner `:::` as its own content.
                        ItemBody::List(..) | ItemBody::ListItem(..) => {}
                        _ => break,
                    }
                }

                if closes {
                    break;
                }
            }

            ix = next_ix + line_start.bytes_scanned();
            if let Some(item) = brk {
                self.tree.append(item);
            }
        }

        self.pop(ix);
        ix
    }

    /// Returns end ix of setext_heading on success.
    fn parse_setext_heading(
        &mut self,
        ix: usize,
        node_ix: TreeIndex,
        has_trailing_content: bool,
    ) -> Option<usize> {
        let bytes = self.text.as_bytes();
        let (n, level) = scan_setext_heading(&bytes[ix..])?;
        let mut attrs = None;

        if let Some(cur_ix) = self.tree.cur() {
            let parent_ix = self.tree.peek_up().unwrap();
            let header_start = self.tree[parent_ix].item.start;
            // Note that `self.tree[parent_ix].item.end` might be zero at this point.
            // Use the end position of the current node (i.e. the last known child
            // of the parent) instead.
            let header_end = self.tree[cur_ix].item.end;

            // extract the trailing attribute block
            let (content_end, attrs_) =
                self.extract_and_parse_heading_attribute_block(header_start, header_end);
            attrs = attrs_;

            // strip trailing whitespace
            let new_end = if has_trailing_content {
                content_end
            } else {
                let mut last_line_start = header_start;
                if attrs.is_some() {
                    loop {
                        let next_line_start =
                            last_line_start + scan_nextline(&bytes[last_line_start..content_end]);
                        if next_line_start >= content_end {
                            break;
                        }
                        let mut line_start = LineStart::new(&bytes[next_line_start..content_end]);
                        if scan_containers(&self.tree, &mut line_start, self.options)
                            != self.tree.spine_len()
                        {
                            break;
                        }
                        last_line_start = next_line_start + line_start.bytes_scanned();
                    }
                }
                let trailing_ws = scan_rev_while(
                    &bytes[last_line_start..content_end],
                    is_ascii_whitespace_no_nl,
                );
                content_end - trailing_ws
            };

            if attrs.is_some() {
                // remove trailing block attributes
                self.tree.truncate_siblings(new_end);
            }

            if let Some(cur_ix) = self.tree.cur() {
                self.tree[cur_ix].item.end = new_end;
            }
        }

        self.tree[node_ix].item.body = ItemBody::Heading(
            level,
            attrs.map(|attrs| self.allocs.allocate_heading(attrs)),
        );

        Some(ix + n)
    }

    /// Parse a line of input, appending text and items to tree.
    ///
    /// Returns: index after line and an item representing the break.
    fn parse_line(
        &mut self,
        start: usize,
        end: Option<usize>,
        mode: TableParseMode,
    ) -> (usize, Option<Item>) {
        let bytes = self.text.as_bytes();
        let bytes = match end {
            Some(end) => &bytes[..end],
            None => bytes,
        };
        let bytes_len = bytes.len();
        let mut pipes = 0;
        let mut last_pipe_ix = start;
        let mut begin_text = start;
        let mut backslash_escaped = false;

        let (final_ix, brk) = iterate_special_bytes(self.lookup_table, bytes, start, |ix, byte| {
            match byte {
                b'\n' | b'\r' => {
                    if let TableParseMode::Active = mode {
                        return LoopInstruction::BreakAtWith(ix, None);
                    }

                    let mut i = ix;
                    let eol_bytes = scan_eol(&bytes[ix..]).unwrap();

                    let end_ix = ix + eol_bytes;
                    let trailing_backslashes = scan_rev_while(&bytes[..ix], |b| b == b'\\');

                    // GFM table detection: check if the next line is a valid
                    // table delimiter. Runs before hard-break so that inputs
                    // like `foo\\\n|-` resolve to a table (keeping the trailing
                    // backslash in the header cell) rather than a paragraph
                    // with a hard break. Headers without pipes are allowed
                    // (`scan_table_head` still requires a pipe in the
                    // delimiter, so setext headings are unaffected). We skip
                    // delimiters that would also be a valid list-item marker,
                    // since block-level lists take precedence over tables.
                    if mode == TableParseMode::Scan {
                        let next_line_ix = ix + eol_bytes;
                        let mut line_start = LineStart::new(&bytes[next_line_ix..]);
                        if scan_containers(&self.tree, &mut line_start, self.options)
                            == self.tree.spine_len()
                        {
                            // In MDX, the delimiter row of a table nested inside
                            // a list item may have extra leading whitespace
                            // beyond the container continuation. No indented
                            // code blocks, so consume it before scan_table_head.
                            if self.options.contains(Options::ENABLE_MDX) {
                                line_start.scan_all_space();
                            }
                            let table_head_ix = next_line_ix + line_start.bytes_scanned();
                            let delim = &bytes[table_head_ix..];
                            let delim_is_list_item = scan_listitem(delim).is_some();
                            let (table_head_bytes, alignment) = if delim_is_list_item {
                                (0, vec![])
                            } else {
                                scan_table_head(delim)
                            };

                            if table_head_bytes > 0 {
                                let header_count =
                                    count_header_cols(bytes, pipes, start, last_pipe_ix);

                                if alignment.len() == header_count {
                                    let alignment_ix = self.allocs.allocate_alignment(alignment);
                                    let end_ix = table_head_ix + table_head_bytes;
                                    return LoopInstruction::BreakAtWith(
                                        end_ix,
                                        Some(Item {
                                            start: i,
                                            end: end_ix, // must update later
                                            body: ItemBody::Table(alignment_ix),
                                        }),
                                    );
                                }
                            }
                        }
                    }

                    if trailing_backslashes % 2 == 1 && end_ix < bytes_len {
                        i -= 1;
                        self.tree.append_text(begin_text, i, backslash_escaped);
                        backslash_escaped = false;
                        return LoopInstruction::BreakAtWith(
                            end_ix,
                            Some(Item {
                                start: i,
                                end: end_ix,
                                body: ItemBody::HardBreak(true),
                            }),
                        );
                    }

                    let trailing_spaces = scan_rev_while(&bytes[..ix], |c| c == b' ');
                    let has_tab_before_spaces = trailing_spaces > 0
                        && ix > trailing_spaces
                        && bytes[ix - trailing_spaces - 1] == b'\t';
                    if trailing_spaces >= 2 && !has_tab_before_spaces {
                        i -= trailing_spaces;
                        self.tree.append_text(begin_text, i, backslash_escaped);
                        backslash_escaped = false;
                        return LoopInstruction::BreakAtWith(
                            end_ix,
                            Some(Item {
                                start: i,
                                end: end_ix,
                                body: ItemBody::HardBreak(false),
                            }),
                        );
                    }

                    let trailing_whitespace =
                        scan_rev_while(&bytes[..ix], is_ascii_whitespace_no_nl);
                    self.tree
                        .append_text(begin_text, ix - trailing_whitespace, backslash_escaped);
                    backslash_escaped = false;

                    LoopInstruction::BreakAtWith(
                        end_ix,
                        Some(Item {
                            start: i,
                            end: end_ix,
                            body: ItemBody::SoftBreak,
                        }),
                    )
                }
                b'\\' if bytes.get(ix + 1).copied().is_some_and(is_ascii_punctuation) => {
                    self.tree.append_text(begin_text, ix, backslash_escaped);
                    if bytes[ix + 1] == b'`' {
                        let count = 1 + scan_ch_repeat(&bytes[(ix + 2)..], b'`');
                        self.tree.append(Item {
                            start: ix + 1,
                            end: ix + count + 1,
                            body: ItemBody::MaybeCode(count, true),
                        });
                        begin_text = ix + 1 + count;
                        backslash_escaped = false;
                        LoopInstruction::ContinueAndSkip(count)
                    } else if bytes[ix + 1] == b'|' && TableParseMode::Active == mode {
                        // Yeah, it's super weird that backslash escaped pipes in tables aren't "real"
                        // backslash escapes.
                        //
                        // This tree structure is intended for the benefit of inline analysis, and it
                        // is supposed to operate as-if backslash escaped pipes were stripped out in a
                        // separate pass.
                        begin_text = ix + 1;
                        backslash_escaped = false;
                        LoopInstruction::ContinueAndSkip(1)
                    } else if ix + 2 < bytes_len
                        && bytes[ix + 1] == b'\\'
                        && bytes[ix + 2] == b'|'
                        && TableParseMode::Active == mode
                    {
                        // To parse `\\|`, discard the backslashes and parse the `|` that follows it.
                        begin_text = ix + 2;
                        backslash_escaped = true;
                        LoopInstruction::ContinueAndSkip(2)
                    } else if bytes[ix + 1] == b'$' && self.options.contains(Options::ENABLE_MATH) {
                        // In math context, \$ should still produce a MaybeMath
                        // delimiter so it can close a math span. The backslash
                        // only prevents opening.
                        begin_text = ix + 1;
                        backslash_escaped = true;
                        LoopInstruction::ContinueAndSkip(0)
                    } else {
                        begin_text = ix + 1;
                        backslash_escaped = true;
                        LoopInstruction::ContinueAndSkip(1)
                    }
                }
                c @ b'*' | c @ b'_' | c @ b'~' | c @ b'^' => {
                    let string_suffix = &self.text[ix..];
                    let count = 1 + scan_ch_repeat(&string_suffix.as_bytes()[1..], c);
                    let can_open = delim_run_can_open(
                        &self.text[start..],
                        string_suffix,
                        count,
                        ix - start,
                        mode,
                        self.options,
                    );
                    let can_close = delim_run_can_close(
                        &self.text[start..],
                        string_suffix,
                        count,
                        ix - start,
                        mode,
                        self.options,
                    );
                    let is_valid_seq = c != b'~'
                        || count == 2
                        || (count == 1
                            && (self.options.contains(Options::ENABLE_STRIKETHROUGH)
                                || self.options.contains(Options::ENABLE_SUBSCRIPT)));

                    if (can_open || can_close) && is_valid_seq {
                        self.tree.append_text(begin_text, ix, backslash_escaped);
                        backslash_escaped = false;
                        for i in 0..count {
                            self.tree.append(Item {
                                start: ix + i,
                                end: ix + i + 1,
                                body: ItemBody::MaybeEmphasis(count - i, can_open, can_close),
                            });
                        }
                        begin_text = ix + count;
                    }
                    LoopInstruction::ContinueAndSkip(count - 1)
                }
                b'$' => {
                    let brace_context =
                        if self.brace_context_stack.len() > MATH_BRACE_CONTEXT_MAX_NESTING {
                            self.brace_context_next as u8
                        } else {
                            self.brace_context_stack.last().copied().unwrap_or_else(|| {
                                self.brace_context_stack.push(!0);
                                !0
                            })
                        };

                    self.tree.append_text(begin_text, ix, backslash_escaped);
                    self.tree.append(Item {
                        start: ix,
                        end: ix + 1,
                        body: ItemBody::MaybeMath(backslash_escaped, brace_context),
                    });
                    begin_text = ix + 1;
                    backslash_escaped = false;
                    LoopInstruction::ContinueAndSkip(0)
                }
                b'{' if self.options.contains(Options::ENABLE_MDX) => {
                    // If `{` sits inside a pair of matching backtick runs on
                    // the current line, it's part of a code span's text —
                    // code spans take priority over MDX expressions in
                    // remark. Skip inline-expression detection so the `{` is
                    // consumed as literal text (the enclosing code span will
                    // pick it up when backtick pairing resolves).
                    if is_inside_code_span_on_line(bytes, ix) {
                        LoopInstruction::ContinueAndSkip(0)
                    } else {
                        // MDX inline expression: try to scan balanced braces.
                        let scan_result = if self.tree.spine_len() > 0 {
                            let check = self.make_container_line_check();
                            scan_mdx_inline_expression_in_container(&bytes[ix..], &check)
                        } else {
                            scan_mdx_inline_expression(&bytes[ix..])
                        };
                        if let Some((content_start, content_end, total_len)) = scan_result {
                            self.tree.append_text(begin_text, ix, backslash_escaped);
                            backslash_escaped = false;
                            let content = &self.text[ix + content_start..ix + content_end];
                            let cow_ix = self.allocs.allocate_cow(content.into());
                            self.tree.append(Item {
                                start: ix,
                                end: ix + total_len,
                                body: ItemBody::MdxTextExpression(cow_ix),
                            });
                            begin_text = ix + total_len;
                            LoopInstruction::ContinueAndSkip(total_len - 1)
                        } else {
                            // Unclosed expression.
                            self.mdx_errors.push((
                                ix,
                                "Unexpected end of file in expression, expected a corresponding \
                             closing brace for `{`"
                                    .to_string(),
                            ));
                            LoopInstruction::ContinueAndSkip(0)
                        }
                    }
                }
                b'{' => {
                    if self.brace_context_stack.len() == MATH_BRACE_CONTEXT_MAX_NESTING {
                        self.brace_context_stack.push(self.brace_context_next as u8);
                        self.brace_context_next = MATH_BRACE_CONTEXT_MAX_NESTING;
                    } else if self.brace_context_stack.len() > MATH_BRACE_CONTEXT_MAX_NESTING {
                        // When we reach the limit of nesting, switch from actually matching
                        // braces to just counting them.
                        self.brace_context_next += 1;
                    } else if !self.brace_context_stack.is_empty() {
                        // Store nothing if no math environment has been reached yet.
                        self.brace_context_stack.push(self.brace_context_next as u8);
                        self.brace_context_next += 1;
                    }
                    LoopInstruction::ContinueAndSkip(0)
                }
                b'}' => {
                    if let &mut [ref mut top_level_context] = &mut self.brace_context_stack[..] {
                        // Unbalanced Braces
                        //
                        // The initial, root top-level brace context is -1, but this is changed whenever an unbalanced
                        // close brace is encountered:
                        //
                        //     This is not a math environment: $}$
                        //     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^|^
                        //     -1                               |-2
                        //
                        // To ensure this can't get parsed as math, each side of the unbalanced
                        // brace is an irreversibly separate brace context. As long as the math
                        // environment itself contains balanced braces, they should share a top level context.
                        //
                        //     Math environment contains 2+2: $}$2+2$
                        //                                       ^^^ this is a math environment
                        *top_level_context = top_level_context.wrapping_sub(1);
                    } else if self.brace_context_stack.len() > MATH_BRACE_CONTEXT_MAX_NESTING {
                        // When we exceed 25 levels of nesting, switch from accurately balancing braces
                        // to just counting them. When we dip back below the limit, switch back.
                        if self.brace_context_next <= MATH_BRACE_CONTEXT_MAX_NESTING {
                            self.brace_context_stack.pop();
                        } else {
                            self.brace_context_next -= 1;
                        }
                    } else {
                        self.brace_context_stack.pop();
                    }
                    LoopInstruction::ContinueAndSkip(0)
                }
                b'`' => {
                    self.tree.append_text(begin_text, ix, backslash_escaped);
                    backslash_escaped = false;
                    let count = 1 + scan_ch_repeat(&bytes[(ix + 1)..], b'`');
                    self.tree.append(Item {
                        start: ix,
                        end: ix + count,
                        body: ItemBody::MaybeCode(count, false),
                    });
                    begin_text = ix + count;
                    LoopInstruction::ContinueAndSkip(count - 1)
                }
                b'<' if bytes.get(ix + 1) != Some(&b'\\') => {
                    // Note: could detect some non-HTML cases and early escape here, but not
                    // clear that's a win.
                    self.tree.append_text(begin_text, ix, backslash_escaped);
                    backslash_escaped = false;
                    self.tree.append(Item {
                        start: ix,
                        end: ix + 1,
                        body: ItemBody::MaybeHtml,
                    });
                    begin_text = ix + 1;
                    LoopInstruction::ContinueAndSkip(0)
                }
                b'!' if bytes.get(ix + 1) == Some(&b'[') => {
                    self.tree.append_text(begin_text, ix, backslash_escaped);
                    backslash_escaped = false;
                    self.tree.append(Item {
                        start: ix,
                        end: ix + 2,
                        body: ItemBody::MaybeImage,
                    });
                    begin_text = ix + 2;
                    LoopInstruction::ContinueAndSkip(1)
                }
                b'[' => {
                    self.tree.append_text(begin_text, ix, backslash_escaped);
                    backslash_escaped = false;
                    self.tree.append(Item {
                        start: ix,
                        end: ix + 1,
                        body: ItemBody::MaybeLinkOpen,
                    });
                    begin_text = ix + 1;
                    LoopInstruction::ContinueAndSkip(0)
                }
                b']' => {
                    self.tree.append_text(begin_text, ix, backslash_escaped);
                    backslash_escaped = false;
                    self.tree.append(Item {
                        start: ix,
                        end: ix + 1,
                        body: ItemBody::MaybeLinkClose(true),
                    });
                    begin_text = ix + 1;
                    LoopInstruction::ContinueAndSkip(0)
                }
                b'&' => match scan_entity(&bytes[ix..]) {
                    (n, Some(value)) => {
                        self.tree.append_text(begin_text, ix, backslash_escaped);
                        backslash_escaped = false;
                        self.tree.append(Item {
                            start: ix,
                            end: ix + n,
                            body: ItemBody::SynthesizeText(self.allocs.allocate_cow(value)),
                        });
                        begin_text = ix + n;
                        LoopInstruction::ContinueAndSkip(n - 1)
                    }
                    _ => LoopInstruction::ContinueAndSkip(0),
                },
                b':' if self.options.contains(Options::ENABLE_DIRECTIVE) => {
                    // Text directive: :name[label]{attrs}
                    // Must not be preceded by another colon (to avoid ::, :::)
                    if ix > 0 && bytes[ix - 1] == b':' {
                        LoopInstruction::ContinueAndSkip(0)
                    } else if let Some((dir_data, end_pos)) =
                        parse_directive_after_colons(self.text, bytes, ix + 1)
                    {
                        // :name: (followed by colon) is NOT a directive (emoji compat)
                        if end_pos < bytes.len() && bytes[end_pos] == b':' {
                            let name_end = ix + 1 + dir_data.name.len();
                            if name_end == end_pos {
                                // bare :name: with no label/attrs
                                return LoopInstruction::ContinueAndSkip(0);
                            }
                        }
                        self.tree.append_text(begin_text, ix, backslash_escaped);
                        backslash_escaped = false;
                        let dir_ix = self.allocs.allocate_directive(dir_data);
                        let consumed = end_pos - ix;
                        self.tree.append(Item {
                            start: ix,
                            end: end_pos,
                            body: ItemBody::TextDirective(dir_ix),
                        });
                        begin_text = end_pos;
                        LoopInstruction::ContinueAndSkip(consumed - 1)
                    } else {
                        LoopInstruction::ContinueAndSkip(0)
                    }
                }
                b'|' => {
                    if ix != 0 && bytes[ix - 1] == b'\\' {
                        LoopInstruction::ContinueAndSkip(0)
                    } else if let TableParseMode::Active = mode {
                        LoopInstruction::BreakAtWith(ix, None)
                    } else {
                        last_pipe_ix = ix;
                        pipes += 1;
                        LoopInstruction::ContinueAndSkip(0)
                    }
                }
                b'.' if matches!(bytes.get(ix + 1..), Some(&[b'.', b'.', ..])) => {
                    self.tree.append_text(begin_text, ix, backslash_escaped);
                    backslash_escaped = false;
                    self.tree.append(Item {
                        start: ix,
                        end: ix + 3,
                        body: ItemBody::SynthesizeChar('…'),
                    });
                    begin_text = ix + 3;
                    LoopInstruction::ContinueAndSkip(2)
                }
                b'-' => {
                    let count = 1 + scan_ch_repeat(&bytes[(ix + 1)..], b'-');
                    if count == 1 {
                        LoopInstruction::ContinueAndSkip(0)
                    } else {
                        let itembody = if count == 2 {
                            ItemBody::SynthesizeChar('–')
                        } else if count == 3 {
                            ItemBody::SynthesizeChar('—')
                        } else {
                            let (ems, ens) = match count % 6 {
                                0 | 3 => (count / 3, 0),
                                2 | 4 => (0, count / 2),
                                1 => (count / 3 - 1, 2),
                                _ => (count / 3, 1),
                            };
                            // – and — are 3 bytes each in utf8
                            let mut buf = String::with_capacity(3 * (ems + ens));
                            for _ in 0..ems {
                                buf.push('—');
                            }
                            for _ in 0..ens {
                                buf.push('–');
                            }
                            ItemBody::SynthesizeText(self.allocs.allocate_cow(buf.into()))
                        };

                        self.tree.append_text(begin_text, ix, backslash_escaped);
                        backslash_escaped = false;
                        self.tree.append(Item {
                            start: ix,
                            end: ix + count,
                            body: itembody,
                        });
                        begin_text = ix + count;
                        LoopInstruction::ContinueAndSkip(count - 1)
                    }
                }
                c @ b'\'' | c @ b'"' => {
                    let string_suffix = &self.text[ix..];
                    let can_open = delim_run_can_open(
                        &self.text[start..],
                        string_suffix,
                        1,
                        ix - start,
                        mode,
                        self.options,
                    );
                    let can_close = delim_run_can_close(
                        &self.text[start..],
                        string_suffix,
                        1,
                        ix - start,
                        mode,
                        self.options,
                    );

                    self.tree.append_text(begin_text, ix, backslash_escaped);
                    backslash_escaped = false;
                    self.tree.append(Item {
                        start: ix,
                        end: ix + 1,
                        body: ItemBody::MaybeSmartQuote(c, can_open, can_close),
                    });
                    begin_text = ix + 1;

                    LoopInstruction::ContinueAndSkip(0)
                }
                _ => LoopInstruction::ContinueAndSkip(0),
            }
        });

        if brk.is_none() {
            let trailing_whitespace =
                scan_rev_while(&bytes[begin_text..final_ix], is_ascii_whitespace_no_nl);
            // need to close text at eof
            self.tree.append_text(
                begin_text,
                final_ix - trailing_whitespace,
                backslash_escaped,
            );
        }
        (final_ix, brk)
    }

    /// When start_ix is at the beginning of an HTML block of type 1 to 5,
    /// this will find the end of the block, adding the block itself to the
    /// tree and also keeping track of the lines of HTML within the block.
    ///
    /// The html_end_tag is the tag that must be found on a line to end the block.
    fn parse_html_block_type_1_to_5(
        &mut self,
        start_ix: usize,
        html_end_tag: &str,
        mut remaining_space: usize,
        mut indent: usize,
    ) -> usize {
        self.tree.append(Item {
            start: start_ix,
            end: 0, // set later
            body: ItemBody::HtmlBlock(false),
        });
        self.tree.push();

        let bytes = self.text.as_bytes();
        let mut ix = start_ix;
        let end_ix;
        loop {
            let line_start_ix = ix;
            ix += scan_nextline(&bytes[ix..]);
            self.append_html_line(remaining_space.max(indent), line_start_ix, ix);

            let mut line_start = LineStart::new(&bytes[ix..]);
            let n_containers = scan_containers(&self.tree, &mut line_start, self.options);
            if n_containers < self.tree.spine_len() {
                end_ix = ix;
                break;
            }

            if self.text[line_start_ix..ix].contains(html_end_tag) {
                end_ix = ix;
                break;
            }

            let next_line_ix = ix + line_start.bytes_scanned();
            if next_line_ix == self.text.len() {
                end_ix = next_line_ix;
                break;
            }
            ix = next_line_ix;
            remaining_space = line_start.remaining_space();
            indent = 0;
        }
        self.pop(end_ix);
        ix
    }

    /// When start_ix is at the beginning of an HTML block of type 6 or 7,
    /// this will consume lines until there is a blank line and keep track of
    /// the HTML within the block.
    fn parse_html_block_type_6_or_7(
        &mut self,
        start_ix: usize,
        mut remaining_space: usize,
        mut indent: usize,
    ) -> usize {
        self.tree.append(Item {
            start: start_ix,
            end: 0, // set later
            body: ItemBody::HtmlBlock(true),
        });
        self.tree.push();

        let bytes = self.text.as_bytes();
        let mut ix = start_ix;
        let end_ix;
        loop {
            let line_start_ix = ix;
            ix += scan_nextline(&bytes[ix..]);
            self.append_html_line(remaining_space.max(indent), line_start_ix, ix);

            let mut line_start = LineStart::new(&bytes[ix..]);
            let n_containers = scan_containers(&self.tree, &mut line_start, self.options);
            if n_containers < self.tree.spine_len() || line_start.is_at_eol() {
                end_ix = ix;
                break;
            }

            let next_line_ix = ix + line_start.bytes_scanned();
            if next_line_ix == self.text.len() || scan_blank_line(&bytes[next_line_ix..]).is_some()
            {
                end_ix = next_line_ix;
                break;
            }
            ix = next_line_ix;
            remaining_space = line_start.remaining_space();
            indent = 0;
        }
        self.pop(end_ix);
        ix
    }

    fn parse_indented_code_block(
        &mut self,
        line_start_ix: usize,
        start_ix: usize,
        mut remaining_space: usize,
    ) -> usize {
        self.tree.append(Item {
            start: line_start_ix,
            end: 0, // will get set later
            body: ItemBody::IndentCodeBlock,
        });
        self.tree.push();
        let bytes = self.text.as_bytes();
        let mut last_nonblank_child = None;
        let mut last_nonblank_ix = 0;
        let mut end_ix = 0;
        self.last_line_blank = false;

        let mut ix = start_ix;
        loop {
            let line_start_ix = ix;
            ix += scan_nextline(&bytes[ix..]);
            self.append_code_text(remaining_space, line_start_ix, ix);
            // TODO(spec clarification): should we synthesize newline at EOF?

            if !self.last_line_blank {
                last_nonblank_child = self.tree.cur();
                last_nonblank_ix = ix;
                end_ix = ix;
            }

            let mut line_start = LineStart::new(&bytes[ix..]);
            let n_containers = scan_containers(&self.tree, &mut line_start, self.options);
            if n_containers < self.tree.spine_len()
                || !(line_start.scan_space(4) || line_start.is_at_eol())
            {
                break;
            }
            let next_line_ix = ix + line_start.bytes_scanned();
            if next_line_ix == self.text.len() {
                break;
            }
            ix = next_line_ix;
            remaining_space = line_start.remaining_space();
            self.last_line_blank = scan_blank_line(&bytes[ix..]).is_some();
        }

        // Trim trailing blank lines.
        if let Some(child) = last_nonblank_child {
            self.tree[child].next = None;
            self.tree[child].item.end = last_nonblank_ix;
        }
        self.pop(end_ix);
        self.list_interrupted_paragraph = true;
        ix
    }

    fn parse_fenced_code_block(
        &mut self,
        start_ix: usize,
        indent: usize,
        fence_ch: u8,
        n_fence_char: usize,
    ) -> usize {
        let bytes = self.text.as_bytes();
        let mut info_start = start_ix + n_fence_char;
        info_start += scan_whitespace_no_nl(&bytes[info_start..]);
        // TODO: info strings are typically very short. wouldn't it be faster
        // to just do a forward scan here?
        let mut ix = info_start + scan_nextline(&bytes[info_start..]);
        // Strip only the line terminator (\n, \r, \r\n) — remark/mdast preserves
        // trailing spaces in the fence info string so they end up in `meta`.
        let info_end = ix - scan_rev_while(&bytes[info_start..ix], |b| b == b'\n' || b == b'\r');
        let info_string = unescape(&self.text[info_start..info_end], self.tree.is_in_table());
        self.tree.append(Item {
            start: start_ix,
            end: 0, // will get set later
            body: ItemBody::FencedCodeBlock(self.allocs.allocate_cow(info_string)),
        });
        self.tree.push();
        loop {
            let mut line_start = LineStart::new(&bytes[ix..]);
            let n_containers = scan_containers(&self.tree, &mut line_start, self.options);
            if n_containers < self.tree.spine_len() {
                // this line will get parsed again as not being part of the code
                // if it's blank, it should be parsed as a blank line
                self.pop(ix);
                return ix;
            }
            if self.options.contains(Options::ENABLE_MDX) {
                // MDX: check for closing fence after consuming all leading whitespace.
                // Use a separate scanner so the content line_start only strips the
                // fence indent (preserving code indentation).
                let mut close_line_start = line_start.clone();
                close_line_start.scan_all_space();
                let close_ix = ix + close_line_start.bytes_scanned();
                if let Some(n) = scan_closing_code_fence(&bytes[close_ix..], fence_ch, n_fence_char)
                {
                    ix = close_ix + n;
                    self.pop(ix);
                    return ix + scan_blank_line(&bytes[ix..]).unwrap_or(0);
                }
                // For content lines, only strip up to `indent` spaces (the fence's
                // own indentation), preserving any deeper indentation as code content.
                line_start.scan_space(indent);
            } else {
                line_start.scan_space(indent);
                let mut close_line_start = line_start.clone();
                if !close_line_start.scan_space(4 - indent) {
                    let close_ix = ix + close_line_start.bytes_scanned();
                    if let Some(n) =
                        scan_closing_code_fence(&bytes[close_ix..], fence_ch, n_fence_char)
                    {
                        ix = close_ix + n;
                        self.pop(ix);
                        // try to read trailing whitespace or it will register as a completely blank line
                        return ix + scan_blank_line(&bytes[ix..]).unwrap_or(0);
                    }
                }
            }
            let remaining_space = line_start.remaining_space();
            ix += line_start.bytes_scanned();
            let next_ix = ix + scan_nextline(&bytes[ix..]);
            self.append_code_text(remaining_space, ix, next_ix);
            ix = next_ix;
        }
    }

    fn parse_math_block(&mut self, start_ix: usize, indent: usize, n_fence_char: usize) -> usize {
        let bytes = self.text.as_bytes();
        let mut meta_start = start_ix + n_fence_char;
        meta_start += scan_whitespace_no_nl(&bytes[meta_start..]);
        let mut ix = meta_start + scan_nextline(&bytes[meta_start..]);
        let meta_end = ix - scan_rev_while(&bytes[meta_start..ix], is_ascii_whitespace);
        let meta_string = if meta_start < meta_end {
            unescape(&self.text[meta_start..meta_end], self.tree.is_in_table())
        } else {
            "".into()
        };
        self.tree.append(Item {
            start: start_ix,
            end: 0,
            body: ItemBody::MathBlock(self.allocs.allocate_cow(meta_string)),
        });
        self.tree.push();
        loop {
            let mut line_start = LineStart::new(&bytes[ix..]);
            let n_containers = scan_containers(&self.tree, &mut line_start, self.options);
            if n_containers < self.tree.spine_len() {
                self.pop(ix);
                return ix;
            }
            line_start.scan_space(indent);
            let mut close_line_start = line_start.clone();
            if !close_line_start.scan_space(4 - indent) {
                let close_ix = ix + close_line_start.bytes_scanned();
                if let Some(n) = scan_closing_math_fence(&bytes[close_ix..], n_fence_char) {
                    ix = close_ix + n;
                    self.pop(ix);
                    return ix + scan_blank_line(&bytes[ix..]).unwrap_or(0);
                }
            }
            let remaining_space = line_start.remaining_space();
            ix += line_start.bytes_scanned();
            let next_ix = ix + scan_nextline(&bytes[ix..]);
            self.append_code_text(remaining_space, ix, next_ix);
            ix = next_ix;
        }
    }

    fn parse_metadata_block(&mut self, start_ix: usize, metadata_block_ch: u8) -> usize {
        let bytes = self.text.as_bytes();
        let metadata_block_kind = match metadata_block_ch {
            b'-' => MetadataBlockKind::YamlStyle,
            b'+' => MetadataBlockKind::PlusesStyle,
            _ => panic!("Erroneous metadata block character when parsing metadata block"),
        };
        // 3 delimiter characters
        let mut ix = start_ix + 3 + scan_nextline(&bytes[start_ix + 3..]);
        self.tree.append(Item {
            start: start_ix,
            end: 0, // will get set later
            body: ItemBody::MetadataBlock(metadata_block_kind),
        });
        self.tree.push();
        loop {
            let mut line_start = LineStart::new(&bytes[ix..]);
            let n_containers = scan_containers(&self.tree, &mut line_start, self.options);
            if n_containers < self.tree.spine_len() {
                break;
            }
            if let (_, 0) = calc_indent(&bytes[ix..], 4) {
                if let Some(n) = scan_closing_metadata_block(&bytes[ix..], metadata_block_ch) {
                    ix += n;
                    break;
                }
            }
            let remaining_space = line_start.remaining_space();
            ix += line_start.bytes_scanned();
            let next_ix = ix + scan_nextline(&bytes[ix..]);
            // Metadata blocks preserve CRLF — remark-frontmatter keeps the
            // original line endings in the yaml/toml value. (The `append_code_text`
            // path normalizes to LF, which is correct for code blocks.)
            if remaining_space > 0 {
                let cow_ix = self.allocs.allocate_cow("   "[..remaining_space].into());
                self.tree.append(Item {
                    start: ix,
                    end: ix,
                    body: ItemBody::SynthesizeText(cow_ix),
                });
            }
            self.tree.append_text(ix, next_ix, false);
            ix = next_ix;
        }

        self.pop(ix);

        // try to read trailing whitespace or it will register as a completely blank line
        ix + scan_blank_line(&bytes[ix..]).unwrap_or(0)
    }

    fn append_code_text(&mut self, remaining_space: usize, start: usize, end: usize) {
        if remaining_space > 0 {
            let cow_ix = self.allocs.allocate_cow("   "[..remaining_space].into());
            self.tree.append(Item {
                start,
                end: start,
                body: ItemBody::SynthesizeText(cow_ix),
            });
        }
        // remark preserves CRLF verbatim in code-block / html / yaml values,
        // so we don't normalize line endings here.
        self.tree.append_text(start, end, false);
    }

    /// Appends a line of HTML to the tree.
    fn append_html_line(&mut self, remaining_space: usize, start: usize, end: usize) {
        if remaining_space > 0 {
            let cow_ix = self.allocs.allocate_cow("   "[..remaining_space].into());
            self.tree.append(Item {
                start,
                end: start,
                body: ItemBody::SynthesizeText(cow_ix),
            });
        }
        // remark preserves CRLF verbatim in html blocks — emit the raw range.
        self.tree.append(Item {
            start,
            end,
            body: ItemBody::Html,
        });
    }

    /// Pop a container, setting its end.
    fn pop(&mut self, ix: usize) {
        let cur_ix = self.tree.pop().unwrap();
        self.tree[cur_ix].item.end = ix;
        if let ItemBody::DefinitionList(_) = self.tree[cur_ix].item.body {
            fixup_end_of_definition_list(&mut self.tree, cur_ix);
            self.begin_list_item = None;
        }
        if let ItemBody::List(true, _, _) | ItemBody::DefinitionList(true) =
            self.tree[cur_ix].item.body
        {
            surgerize_tight_list(&mut self.tree, cur_ix);
            self.begin_list_item = None;
        }
    }

    /// Close a list if it's open. Also set loose if last line was blank
    /// and end current list if it's a lone, empty item
    fn finish_list(&mut self, ix: usize) {
        self.finish_empty_list_item();
        if let Some(node_ix) = self.tree.peek_up() {
            if let ItemBody::List(_, _, _) | ItemBody::DefinitionList(_) =
                self.tree[node_ix].item.body
            {
                self.pop(ix);
            }
        }
        if self.last_line_blank {
            if let Some(node_ix) = self.tree.peek_grandparent() {
                if let ItemBody::List(ref mut is_tight, _, _)
                | ItemBody::DefinitionList(ref mut is_tight) = self.tree[node_ix].item.body
                {
                    *is_tight = false;
                }
            }
            self.last_line_blank = false;
        }
    }

    fn mark_enclosing_listitem_spread(&mut self) {
        let spine: Vec<TreeIndex> = self.tree.walk_spine().copied().collect();
        for node_ix in spine.into_iter().rev() {
            if let ItemBody::ListItem(indent, _) = self.tree[node_ix].item.body {
                self.tree[node_ix].item.body = ItemBody::ListItem(indent, true);
                return;
            }
        }
    }

    fn finish_empty_list_item(&mut self) {
        if let Some(begin_list_item) = self.begin_list_item {
            if self.last_line_blank {
                // A list item can begin with at most one blank line.
                if let Some(node_ix) = self.tree.peek_up() {
                    if let ItemBody::ListItem(_, _) | ItemBody::DefinitionListDefinition(_) =
                        self.tree[node_ix].item.body
                    {
                        self.pop(begin_list_item);
                    }
                }
            }
        }
        self.begin_list_item = None;
    }

    /// Continue an existing list or start a new one if there's not an open
    /// list that matches.
    fn continue_list(&mut self, start: usize, ch: u8, index: u64) {
        self.finish_empty_list_item();
        if let Some(node_ix) = self.tree.peek_up() {
            if let ItemBody::List(ref mut is_tight, existing_ch, _) = self.tree[node_ix].item.body {
                if existing_ch == ch {
                    if self.last_line_blank {
                        *is_tight = false;
                        self.last_line_blank = false;
                    }
                    return;
                }
            }
            // TODO: this is not the best choice for end; maybe get end from last list item.
            self.finish_list(start);
        }
        self.tree.append(Item {
            start,
            end: 0, // will get set later
            body: ItemBody::List(true, ch, index),
        });
        self.tree.push();
        self.last_line_blank = false;
    }

    /// Parse a thematic break.
    ///
    /// Returns index of start of next line.
    fn parse_hrule(&mut self, hrule_size: usize, ix: usize) -> usize {
        self.tree.append(Item {
            start: ix,
            end: ix + hrule_size,
            body: ItemBody::Rule,
        });
        ix + hrule_size
    }

    /// Parse an ATX heading.
    ///
    /// Returns index of start of next line.
    fn parse_atx_heading(&mut self, start: usize, atx_level: HeadingLevel) -> usize {
        let mut ix = start;
        let heading_ix = self.tree.append(Item {
            start,
            end: 0,                    // set later
            body: ItemBody::default(), // set later
        });
        ix += atx_level as usize;
        // next char is space or eol (guaranteed by scan_atx_heading)
        let bytes = self.text.as_bytes();
        if let Some(eol_bytes) = scan_eol(&bytes[ix..]) {
            self.tree[heading_ix].item.end = ix + eol_bytes;
            self.tree[heading_ix].item.body = ItemBody::Heading(atx_level, None);
            return ix + eol_bytes;
        }
        // skip leading spaces
        let skip_spaces = scan_whitespace_no_nl(&bytes[ix..]);
        ix += skip_spaces;

        // now handle the header text
        let header_start = ix;
        let header_node_idx = self.tree.push(); // so that we can set the endpoint later

        // trim the trailing attribute block before parsing the entire line, if necessary.
        // When MDX is enabled, `{...}` in headings should be treated as MDX expressions,
        // not heading attribute blocks. MDX expressions and heading attributes use the
        // same `{...}` syntax and would conflict.
        let (end, content_end, attrs) = if self.options.contains(Options::ENABLE_HEADING_ATTRIBUTES)
            && !self.options.contains(Options::ENABLE_MDX)
        {
            // the start of the next line is the end of the header since the
            // header cannot have line breaks
            let header_end = header_start + scan_nextline(&bytes[header_start..]);
            let (content_end, attrs) =
                self.extract_and_parse_heading_attribute_block(header_start, header_end);
            self.parse_line(ix, Some(content_end), TableParseMode::Disabled);
            (header_end, content_end, attrs)
        } else {
            let (line_ix, line_brk) = self.parse_line(ix, None, TableParseMode::Disabled);
            ix = line_ix;
            // Backslash at end is actually hard line break
            if let Some(Item {
                start,
                end,
                body: ItemBody::HardBreak(true),
            }) = line_brk
            {
                self.tree.append_text(start, end, false);
            }
            (ix, ix, None)
        };
        self.tree[header_node_idx].item.end = end;

        // remove trailing matter from header text
        let mut empty_text_node = false;
        if let Some(cur_ix) = self.tree.cur() {
            // remove closing of the ATX heading
            let header_text = &bytes[header_start..content_end];
            let mut limit = header_text
                .iter()
                .rposition(|&b| !(b == b'\n' || b == b'\r' || b == b' ' || b == b'\t'))
                .map_or(0, |i| i + 1);
            let closer = header_text[..limit]
                .iter()
                .rposition(|&b| b != b'#')
                .map_or(0, |i| i + 1);
            if closer == 0 {
                limit = closer;
            } else {
                let spaces = scan_rev_while(&header_text[..closer], |b| b == b' ' || b == b'\t');
                if spaces > 0 {
                    limit = closer - spaces;
                }
            }
            // if text is only spaces, then remove them
            self.tree[cur_ix].item.end = limit + header_start;

            // limit = 0 when text is empty after removing spaces
            if limit == 0 {
                empty_text_node = true;
            }
        }

        if empty_text_node {
            self.tree.remove_node();
        } else {
            self.tree.pop();
        }
        self.tree[heading_ix].item.body = ItemBody::Heading(
            atx_level,
            attrs.map(|attrs| self.allocs.allocate_heading(attrs)),
        );

        end
    }

    /// Returns the number of bytes scanned on success.
    fn parse_footnote(&mut self, start: usize) -> Option<usize> {
        let bytes = &self.text.as_bytes()[start..];
        if !bytes.starts_with(b"[^") {
            return None;
        }
        // GitHub doesn't allow footnote definition labels to contain line
        // breaks. It actually does allow this for link definitions under
        // certain circumstances, but for this it's simpler to avoid it.
        let (mut i, label) =
            scan_link_label_rest(&self.text[start + 2..], &|_| None, self.tree.is_in_table())?;
        if label.bytes().any(|b| b == b'\r' || b == b'\n') {
            return None;
        }
        i += 2;
        if bytes.get(i) != Some(&b':') {
            return None;
        }
        i += 1;
        self.finish_list(start);
        if let Some(node_ix) = self.tree.peek_up() {
            if let ItemBody::FootnoteDefinition(..) = self.tree[node_ix].item.body {
                // finish previous footnote if it's still open
                self.pop(start);
            }
        }
        i += scan_whitespace_no_nl(&bytes[i..]);
        self.allocs
            .footdefs
            .0
            .insert(UniCase::new(label.clone()), FootnoteDef { use_count: 0 });
        self.tree.append(Item {
            start,
            end: 0, // will get set later
            // TODO: check whether the label here is strictly necessary
            body: ItemBody::FootnoteDefinition(self.allocs.allocate_cow(label)),
        });
        self.tree.push();
        Some(i)
    }

    /// Tries to parse a reference label, which can be interrupted by new blocks.
    /// On success, returns the number of bytes of the label and the label itself.
    fn parse_refdef_label(&self, start: usize) -> Option<(usize, CowStr<'a>)> {
        scan_link_label_rest(
            &self.text[start..],
            &|bytes| {
                let mut line_start = LineStart::new(bytes);
                let tree_position = scan_containers(&self.tree, &mut line_start, self.options);
                let current_container = tree_position == self.tree.spine_len();
                if line_start.scan_space(4) {
                    return Some(line_start.bytes_scanned());
                }
                let bytes_scanned = line_start.bytes_scanned();
                let suffix = &bytes[bytes_scanned..];
                if self.scan_paragraph_interrupt(suffix, current_container, tree_position)
                    || (current_container && scan_setext_heading(suffix).is_some())
                {
                    None
                } else {
                    Some(bytes_scanned)
                }
            },
            self.tree.is_in_table(),
        )
    }

    /// Returns number of bytes scanned, label and definition on success.
    fn parse_refdef_total(&mut self, start: usize) -> Option<(usize, LinkLabel<'a>, LinkDef<'a>)> {
        let bytes = &self.text.as_bytes()[start..];
        if bytes.first() != Some(&b'[') {
            return None;
        }
        let (mut i, label) = self.parse_refdef_label(start + 1)?;
        i += 1;
        if bytes.get(i) != Some(&b':') {
            return None;
        }
        i += 1;
        let (bytecount, link_def) = self.scan_refdef(start, start + i)?;
        Some((bytecount + i, UniCase::new(label), link_def))
    }

    /// Returns number of bytes and number of newlines
    fn scan_refdef_space(&self, bytes: &[u8], mut i: usize) -> Option<(usize, usize)> {
        let mut newlines = 0;
        loop {
            let whitespaces = scan_whitespace_no_nl(&bytes[i..]);
            i += whitespaces;
            if let Some(eol_bytes) = scan_eol(&bytes[i..]) {
                i += eol_bytes;
                newlines += 1;
                if newlines > 1 {
                    return None;
                }
            } else {
                break;
            }
            let mut line_start = LineStart::new(&bytes[i..]);
            let tree_position = scan_containers(&self.tree, &mut line_start, self.options);
            let current_container = tree_position == self.tree.spine_len();
            if !line_start.scan_space(4) {
                let suffix = &bytes[i + line_start.bytes_scanned()..];
                if self.scan_paragraph_interrupt(suffix, current_container, tree_position)
                    || scan_setext_heading(suffix).is_some()
                {
                    return None;
                }
            }
            i += line_start.bytes_scanned();
        }
        Some((i, newlines))
    }

    // returns (bytelength, title_str)
    fn scan_refdef_title<'t>(&self, text: &'t str) -> Option<(usize, CowStr<'t>)> {
        let bytes = text.as_bytes();
        let closing_delim = match bytes.first()? {
            b'\'' => b'\'',
            b'"' => b'"',
            b'(' => b')',
            _ => return None,
        };
        let mut bytecount = 1;
        let mut linestart = 1;

        let mut linebuf = None;

        while let Some(&c) = bytes.get(bytecount) {
            match c {
                b'(' if closing_delim == b')' => {
                    // https://spec.commonmark.org/0.30/#link-title
                    // a sequence of zero or more characters between matching parentheses ((...)),
                    // including a ( or ) character only if it is backslash-escaped.
                    return None;
                }
                b'\n' | b'\r' => {
                    // push text to line buffer
                    // this is used to strip the block formatting:
                    //
                    // > [first]: http://example.com "
                    // > second"
                    //
                    // should get turned into `<a href="example.com" title=" second">first</a>`
                    let linebuf = if let Some(linebuf) = &mut linebuf {
                        linebuf
                    } else {
                        linebuf = Some(String::new());
                        linebuf.as_mut().unwrap()
                    };
                    linebuf.push_str(&text[linestart..bytecount]);
                    linebuf.push('\n'); // normalize line breaks
                                        // skip line break
                    bytecount += 1;
                    if c == b'\r' && bytes.get(bytecount) == Some(&b'\n') {
                        bytecount += 1;
                    }
                    let mut line_start = LineStart::new(&bytes[bytecount..]);
                    let tree_position = scan_containers(&self.tree, &mut line_start, self.options);
                    let current_container = tree_position == self.tree.spine_len();
                    if !line_start.scan_space(4) {
                        let suffix = &bytes[bytecount + line_start.bytes_scanned()..];
                        if self.scan_paragraph_interrupt(suffix, current_container, tree_position)
                            || scan_setext_heading(suffix).is_some()
                        {
                            return None;
                        }
                    }
                    line_start.scan_all_space();
                    bytecount += line_start.bytes_scanned();
                    linestart = bytecount;
                    if scan_blank_line(&bytes[bytecount..]).is_some() {
                        // blank line - not allowed
                        return None;
                    }
                }
                b'\\' => {
                    bytecount += 1;
                    if let Some(c) = bytes.get(bytecount) {
                        if c != &b'\r' && c != &b'\n' {
                            bytecount += 1;
                        }
                    }
                }
                c if c == closing_delim => {
                    let cow = if let Some(mut linebuf) = linebuf {
                        linebuf.push_str(&text[linestart..bytecount]);
                        CowStr::from(linebuf)
                    } else {
                        CowStr::from(&text[linestart..bytecount])
                    };
                    return Some((bytecount + 1, cow));
                }
                _ => {
                    bytecount += 1;
                }
            }
        }
        None
    }

    /// Returns # of bytes and definition.
    /// Assumes the label of the reference including colon has already been scanned.
    fn scan_refdef(&self, span_start: usize, start: usize) -> Option<(usize, LinkDef<'a>)> {
        let bytes = self.text.as_bytes();

        // whitespace between label and url (including up to one newline)
        let (mut i, _newlines) = self.scan_refdef_space(bytes, start)?;

        // scan link dest
        let (dest_length, dest) = scan_link_dest(self.text, i, LINK_MAX_NESTED_PARENS)?;
        if dest_length == 0 {
            return None;
        }
        let dest = unescape(dest, self.tree.is_in_table());
        i += dest_length;

        // no title
        let mut backup = (
            i - start,
            LinkDef {
                dest,
                title: None,
                span: span_start..i,
            },
        );

        // scan whitespace between dest and label
        let (mut i, newlines) =
            if let Some((new_i, mut newlines)) = self.scan_refdef_space(bytes, i) {
                if i == self.text.len() {
                    newlines += 1;
                }
                if new_i == i && newlines == 0 {
                    return None;
                }
                if newlines > 1 {
                    return Some(backup);
                };
                (new_i, newlines)
            } else {
                return Some(backup);
            };

        // scan title
        // if this fails but newline == 1, return also a refdef without title
        if let Some((title_length, title)) = self.scan_refdef_title(&self.text[i..]) {
            i += title_length;
            if scan_blank_line(&bytes[i..]).is_some() {
                backup.0 = i - start;
                backup.1.span = span_start..i;
                backup.1.title = Some(unescape(title, self.tree.is_in_table()));
                return Some(backup);
            }
        }
        if newlines > 0 {
            Some(backup)
        } else {
            None
        }
    }

    /// Checks whether we should break a paragraph on the given input.
    fn scan_paragraph_interrupt(
        &self,
        bytes: &[u8],
        current_container: bool,
        tree_position: usize,
    ) -> bool {
        // The free `scan_paragraph_interrupt_no_table` helper checks MDX JSX
        // flow elements with a `None` container check, which misfires inside
        // a blockquote or similar container where the `>` continuation prefix
        // would be mis-read as a JSX close `>`. Re-check that specific case
        // with the spine's container prefix so multi-line JSX in blockquotes
        // correctly interrupts paragraphs.
        if self.options.contains(Options::ENABLE_MDX)
            && bytes.starts_with(b"<")
            && self
                .scan_mdx_flow_in_container_bytes(bytes, |b, c| scan_mdx_jsx_block(b, c))
                .is_some()
        {
            return true;
        }
        if scan_paragraph_interrupt_no_table(
            bytes,
            current_container,
            self.options.contains(Options::ENABLE_FOOTNOTES),
            self.options.contains(Options::ENABLE_DEFINITION_LIST),
            self.options.contains(Options::ENABLE_MDX),
            self.options.contains(Options::ENABLE_MATH),
            &self.tree,
            tree_position,
        ) {
            return true;
        }
        // pulldown-cmark allows heavy tables, that have a `|` on the header row,
        // to interrupt paragraphs.
        //
        // ```markdown
        // This is a table
        // | a | b | c |
        // |---|---|---|
        // | d | e | f |
        //
        // This is not a table
        //  a | b | c
        // ---|---|---
        //  d | e | f
        // ```
        if !self.options.contains(Options::ENABLE_TABLES) || !bytes.starts_with(b"|") {
            return false;
        }

        // Checking if something's a valid table or not requires looking at two lines.
        // First line, count unescaped pipes.
        let mut pipes = 0;
        let mut next_line_ix = 0;
        let mut bsesc = false;
        let mut last_pipe_ix = 0;
        for (i, &byte) in bytes.iter().enumerate() {
            match byte {
                b'\\' => {
                    bsesc = true;
                    continue;
                }
                b'|' if !bsesc => {
                    pipes += 1;
                    last_pipe_ix = i;
                }
                b'\r' | b'\n' => {
                    next_line_ix = i + scan_eol(&bytes[i..]).unwrap();
                    break;
                }
                _ => {}
            }
            bsesc = false;
        }

        // scan_eol can't return 0, so this can't be zero
        if next_line_ix == 0 {
            return false;
        }

        // Scan the table head. The part that looks like:
        //
        //     |---|---|---|
        //
        // Also scan any containing items, since it's on its own line, and
        // might be nested inside a block quote or something
        //
        //     > Table: First
        //     > | first col | second col |
        //     > |-----------|------------|
        //     ^
        //     | need to skip over the `>` when checking for the table
        let mut line_start = LineStart::new(&bytes[next_line_ix..]);
        if scan_containers(&self.tree, &mut line_start, self.options) != self.tree.spine_len() {
            return false;
        }
        // In MDX, a table nested inside a list item may be indented beyond the
        // list's continuation column (no indented code blocks to conflict
        // with). Consume any extra whitespace so scan_table_head sees the
        // delimiter row flush-left.
        if self.options.contains(Options::ENABLE_MDX) {
            line_start.scan_all_space();
        }
        let table_head_ix = next_line_ix + line_start.bytes_scanned();
        let (table_head_bytes, alignment) = scan_table_head(&bytes[table_head_ix..]);

        if table_head_bytes == 0 {
            return false;
        }

        // computing header count from number of pipes
        let header_count = count_header_cols(bytes, pipes, 0, last_pipe_ix);

        // make sure they match the number of columns we find in separator line
        alignment.len() == header_count
    }

    /// Extracts and parses a heading attribute block if exists.
    ///
    /// Returns `(end_offset_of_heading_content, (id, classes))`.
    ///
    /// If `header_end` is less than or equal to `header_start`, the given
    /// input is considered as empty.
    fn extract_and_parse_heading_attribute_block(
        &mut self,
        header_start: usize,
        header_end: usize,
    ) -> (usize, Option<HeadingAttributes<'a>>) {
        if !self.options.contains(Options::ENABLE_HEADING_ATTRIBUTES)
            || self.options.contains(Options::ENABLE_MDX)
        {
            return (header_end, None);
        }

        // extract the trailing attribute block
        let header_bytes = &self.text.as_bytes()[header_start..header_end];
        let (content_len, attr_block_range_rel) =
            extract_attribute_block_content_from_header_text(header_bytes);
        let attrs = attr_block_range_rel.and_then(|r| {
            parse_inside_attribute_block(
                &self.text[(header_start + r.start)..(header_start + r.end)],
            )
        });
        let content_end = if attrs.is_some() {
            header_start + content_len
        } else {
            header_end
        };
        (content_end, attrs)
    }
}

/// Scanning modes for `Parser`'s `parse_line` method.
#[derive(PartialEq, Eq, Copy, Clone)]
enum TableParseMode {
    /// Inside a paragraph, scanning for table headers.
    Scan,
    /// Inside a table.
    Active,
    /// Inside a paragraph, not scanning for table headers.
    Disabled,
}

/// Computes the number of header columns in a table line by computing the number of dividing pipes
/// that aren't followed or preceded by whitespace.
fn count_header_cols(
    bytes: &[u8],
    mut pipes: usize,
    mut start: usize,
    last_pipe_ix: usize,
) -> usize {
    // A header with no pipes is a single-column "light" table row.
    if pipes == 0 {
        return 1;
    }
    // was first pipe preceded by whitespace? if so, subtract one
    start += scan_whitespace_no_nl(&bytes[start..]);
    if bytes[start] == b'|' {
        pipes -= 1;
    }

    // was last pipe followed by whitespace? if so, sub one
    if scan_blank_line(&bytes[(last_pipe_ix + 1)..]).is_some() {
        pipes
    } else {
        pipes + 1
    }
}

/// Checks whether we should break a paragraph on the given input.
///
/// Use `FirstPass::scan_paragraph_interrupt` in any context that allows
/// tables to interrupt the paragraph.
#[allow(clippy::too_many_arguments)]
fn scan_paragraph_interrupt_no_table(
    bytes: &[u8],
    current_container: bool,
    has_footnote: bool,
    definition_list: bool,
    mdx: bool,
    math: bool,
    tree: &Tree<Item>,
    tree_position: usize,
) -> bool {
    scan_eol(bytes).is_some()
        || scan_hrule(bytes).is_ok()
        || scan_atx_heading(bytes).is_some()
        || scan_code_fence(bytes).is_some()
        || (math && scan_math_fence(bytes).is_some())
        || scan_interrupting_container_extensions_fence(bytes)
        || scan_blockquote_start(bytes).is_some()
        || scan_listitem(bytes).is_some_and(|(ix, delim, index, _)| {
            ! current_container ||
            tree.is_in_table() ||
            // we don't allow interruption by either empty lists or
            // numbered lists starting at an index other than 1
            (delim == b'*' || delim == b'-' || delim == b'+' || index == 1)
                && (scan_blank_line(&bytes[ix..]).is_none())
        })
        || bytes.starts_with(b"<")
            && (get_html_end_tag(&bytes[1..]).is_some() || starts_html_block_type_6(&bytes[1..]))
        // MDX JSX flow elements also interrupt paragraphs
        || (mdx && bytes.starts_with(b"<") && scan_mdx_jsx_block(bytes, None).is_some())
        // MDX flow expressions (`{ ... }` spanning the full line) also interrupt
        || (mdx && bytes.starts_with(b"{") && scan_mdx_expression_block(bytes, None).is_some())
        || definition_list
            && ((current_container
                && tree.peek_up().is_some_and(|cur| {
                    matches!(
                        tree[cur].item.body,
                        ItemBody::Paragraph
                            | ItemBody::TightParagraph
                            | ItemBody::MaybeDefinitionListTitle
                    )
                }))
                || tree.walk_spine().nth(tree_position).is_some_and(|cur| {
                    matches!(tree[*cur].item.body, ItemBody::DefinitionListDefinition(_))
                }))
            && bytes.starts_with(b":")
        || (has_footnote
            && bytes.starts_with(b"[^")
            && scan_link_label_rest(
                core::str::from_utf8(&bytes[2..]).unwrap(),
                &|_| None,
                tree.is_in_table(),
            )
            .is_some_and(|(len, _)| bytes.get(2 + len) == Some(&b':')))
}

/// True if `bytes` contains at least one blank line (a line consisting
/// entirely of spaces/tabs, or an empty line). Used to propagate "loose"
/// state up to the enclosing list when a multi-line flow block (JSX or
/// expression) is consumed atomically.
/// Return `true` if `pos` sits between two backtick runs of matching length
/// on the current line. Used to give code spans priority over MDX inline
/// expressions — `` `{foo}` `` is a code span containing the text `{foo}`,
/// not an inline expression.
fn is_inside_code_span_on_line(bytes: &[u8], pos: usize) -> bool {
    // Scan backward for the start of the current line.
    let mut line_start = pos;
    while line_start > 0 && bytes[line_start - 1] != b'\n' && bytes[line_start - 1] != b'\r' {
        line_start -= 1;
    }
    // Scan forward for the end of the current line.
    let mut line_end = pos;
    while line_end < bytes.len() && bytes[line_end] != b'\n' && bytes[line_end] != b'\r' {
        line_end += 1;
    }
    // Collect backtick runs on this line, skipping backslash-escaped ones.
    let mut runs: Vec<(usize, usize)> = Vec::new(); // (start, count)
    let mut i = line_start;
    while i < line_end {
        if bytes[i] == b'\\' && i + 1 < line_end {
            i += 2;
            continue;
        }
        if bytes[i] == b'`' {
            let start = i;
            while i < line_end && bytes[i] == b'`' {
                i += 1;
            }
            runs.push((start, i - start));
        } else {
            i += 1;
        }
    }
    if runs.len() < 2 {
        return false;
    }
    // First-fit pair the runs: each open matches the next run of the same
    // length, and once matched both are consumed.
    let mut paired = vec![false; runs.len()];
    for a in 0..runs.len() {
        if paired[a] {
            continue;
        }
        for b in (a + 1)..runs.len() {
            if paired[b] {
                continue;
            }
            if runs[b].1 == runs[a].1 {
                paired[a] = true;
                paired[b] = true;
                let span_start = runs[a].0 + runs[a].1;
                let span_end = runs[b].0;
                if pos >= span_start && pos < span_end {
                    return true;
                }
                break;
            }
        }
    }
    false
}

fn contains_blank_line(bytes: &[u8]) -> bool {
    let mut i = 0;
    let mut at_line_start = true;
    let mut line_has_non_ws = false;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\n' || b == b'\r' {
            if !line_has_non_ws && !at_line_start {
                // A line that contained only whitespace.
                return true;
            }
            // Skip \r\n as one terminator.
            if b == b'\r' && i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                i += 2;
            } else {
                i += 1;
            }
            // If the next line is *also* a terminator, it's a blank line.
            if i < bytes.len() && (bytes[i] == b'\n' || bytes[i] == b'\r') {
                return true;
            }
            at_line_start = true;
            line_has_non_ws = false;
            continue;
        }
        at_line_start = false;
        if b != b' ' && b != b'\t' {
            line_has_non_ws = true;
        }
        i += 1;
    }
    false
}

fn is_directive_name_start_ascii(c: u8) -> bool {
    c.is_ascii_alphanumeric()
}

fn is_directive_name_char_ascii(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'-' || c == b'_'
}

/// True when the char at `ix` is a valid directive-name character.
/// Non-ASCII characters are accepted when they are ID_Continue (letters,
/// marks, digits), which matches micromark-extension-directive's behavior of
/// terminating names on unicode punctuation (CJK full-stop, etc.) while
/// accepting CJK letters.
fn scan_directive_name_char(bytes: &[u8], ix: usize) -> Option<usize> {
    if ix >= bytes.len() {
        return None;
    }
    let b = bytes[ix];
    if b < 0x80 {
        return if is_directive_name_char_ascii(b) {
            Some(1)
        } else {
            None
        };
    }
    let rest = core::str::from_utf8(&bytes[ix..]).ok()?;
    let ch = rest.chars().next()?;
    if unicode_id_start::is_id_continue(ch) {
        Some(ch.len_utf8())
    } else {
        None
    }
}

fn scan_directive_name_start(bytes: &[u8]) -> Option<usize> {
    if bytes.is_empty() {
        return None;
    }
    let b = bytes[0];
    if b < 0x80 {
        return if is_directive_name_start_ascii(b) {
            Some(1)
        } else {
            None
        };
    }
    let rest = core::str::from_utf8(bytes).ok()?;
    let ch = rest.chars().next()?;
    if unicode_id_start::is_id_start(ch) {
        Some(ch.len_utf8())
    } else {
        None
    }
}

/// Parse a directive name per remark-directive spec.
/// Returns (name, bytes_consumed) or None.
fn scan_directive_name(bytes: &[u8]) -> Option<(usize, usize)> {
    let first = scan_directive_name_start(bytes)?;
    let mut len = first;
    while let Some(n) = scan_directive_name_char(bytes, len) {
        len += n;
    }
    if len == 0 {
        return None;
    }
    let last = bytes[len - 1];
    if last == b'-' || last == b'_' {
        return None;
    }
    Some((0, len))
}

/// Parse a directive label `[content]`. Returns (label_start, label_end, total_consumed).
/// label_start/label_end are byte offsets within `bytes` of the inner content.
fn scan_directive_label(bytes: &[u8]) -> Option<(usize, usize, usize)> {
    if bytes.is_empty() || bytes[0] != b'[' {
        return None;
    }
    let mut depth = 1i32;
    let mut i = 1;
    let label_start = 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if i + 1 < bytes.len() => {
                i += 2;
                continue;
            }
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some((label_start, i, i + 1));
                }
            }
            b'\n' | b'\r' => return None,
            _ => {}
        }
        i += 1;
    }
    None
}

/// Parse directive attributes `{...}`. Returns (attrs, total_consumed).
fn scan_directive_attributes(bytes: &[u8]) -> Option<(Vec<(CowStr<'_>, CowStr<'_>)>, usize)> {
    if bytes.is_empty() || bytes[0] != b'{' {
        return None;
    }
    let mut i = 1;
    let end = loop {
        if i >= bytes.len() {
            return None;
        }
        match bytes[i] {
            b'}' => break i,
            b'\n' | b'\r' => return None,
            b'\\' if i + 1 < bytes.len() => i += 2,
            _ => i += 1,
        }
    };
    let inner = &bytes[1..end];
    let attrs = parse_directive_attrs_inner(inner);
    Some((attrs, end + 1))
}

fn parse_directive_attrs_inner(bytes: &[u8]) -> Vec<(CowStr<'_>, CowStr<'_>)> {
    let mut attrs: Vec<(CowStr<'_>, CowStr<'_>)> = Vec::new();
    let mut classes: Vec<&str> = Vec::new();
    let mut id: Option<&str> = None;
    let text = core::str::from_utf8(bytes).unwrap_or("");
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b' ' | b'\t' => {
                i += 1;
            }
            b'#' => {
                i += 1;
                let start = i;
                while i < bytes.len() && !is_attr_shortcut_terminator(bytes[i]) {
                    i += 1;
                }
                if i > start {
                    id = Some(&text[start..i]);
                }
            }
            b'.' => {
                i += 1;
                let start = i;
                while i < bytes.len() && !is_attr_shortcut_terminator(bytes[i]) {
                    i += 1;
                }
                if i > start {
                    classes.push(&text[start..i]);
                }
            }
            _ => {
                let name_start = i;
                while i < bytes.len()
                    && (bytes[i].is_ascii_alphanumeric()
                        || bytes[i] == b'-'
                        || bytes[i] == b'.'
                        || bytes[i] == b':'
                        || bytes[i] == b'_')
                {
                    i += 1;
                }
                if i == name_start {
                    i += 1;
                    continue;
                }
                let name = &text[name_start..i];
                if i < bytes.len() && bytes[i] == b'=' {
                    i += 1;
                    if i < bytes.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
                        let quote = bytes[i];
                        i += 1;
                        let val_start = i;
                        while i < bytes.len() && bytes[i] != quote {
                            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                                i += 1;
                            }
                            i += 1;
                        }
                        let val = &text[val_start..i];
                        if i < bytes.len() {
                            i += 1; // skip closing quote
                        }
                        attrs.push((name.into(), val.into()));
                    } else {
                        let val_start = i;
                        while i < bytes.len()
                            && bytes[i] != b' '
                            && bytes[i] != b'\t'
                            && bytes[i] != b'}'
                        {
                            i += 1;
                        }
                        attrs.push((name.into(), text[val_start..i].into()));
                    }
                } else {
                    attrs.push((name.into(), "".into()));
                }
            }
        }
    }

    if let Some(id_val) = id {
        attrs.push(("id".into(), id_val.into()));
    }
    if !classes.is_empty() {
        attrs.push(("class".into(), classes.join(" ").into()));
    }
    attrs
}

fn is_attr_shortcut_terminator(c: u8) -> bool {
    matches!(c, b'#' | b'.' | b'}' | b' ' | b'\t')
}

/// Parse name[label]{attrs} after the colon(s). Returns (DirectiveAttrData, end_position).
/// The end_position is right after the last parsed component (name, label, or attrs).
fn parse_directive_after_colons<'a>(
    text: &'a str,
    bytes: &'a [u8],
    start: usize,
) -> Option<(DirectiveAttrData<'a>, usize)> {
    let remaining = &bytes[start..];
    let (_, name_len) = scan_directive_name(remaining)?;
    let name: CowStr<'a> = text[start..start + name_len].into();
    let mut pos = start + name_len;

    let mut label_start = 0usize;
    let mut label_end = 0usize;

    // Label (no space allowed between name and [)
    if pos < bytes.len() && bytes[pos] == b'[' {
        if let Some((ls, le, consumed)) = scan_directive_label(&bytes[pos..]) {
            label_start = pos + ls;
            label_end = pos + le;
            pos += consumed;
        }
    }

    // Attributes (no space allowed between label/name and {)
    let mut attributes = Vec::new();
    if pos < bytes.len() && bytes[pos] == b'{' {
        if let Some((attrs, consumed)) = scan_directive_attributes(&bytes[pos..]) {
            attributes = attrs;
            pos += consumed;
        }
    }

    Some((
        DirectiveAttrData {
            name,
            attributes,
            label_start,
            label_end,
        },
        pos,
    ))
}

/// Assumes `text_bytes` is preceded by `<`.
fn get_html_end_tag(text_bytes: &[u8]) -> Option<&'static str> {
    static BEGIN_TAGS: &[&[u8]; 4] = &[b"pre", b"style", b"script", b"textarea"];
    static ST_BEGIN_TAGS: &[&[u8]; 3] = &[b"!--", b"?", b"![CDATA["];

    for (beg_tag, end_tag) in BEGIN_TAGS
        .iter()
        .zip(["</pre>", "</style>", "</script>", "</textarea>"].iter())
    {
        let tag_len = beg_tag.len();

        if text_bytes.len() < tag_len {
            // begin tags are increasing in size
            break;
        }

        if !text_bytes[..tag_len].eq_ignore_ascii_case(beg_tag) {
            continue;
        }

        // Must either be the end of the line...
        if text_bytes.len() == tag_len {
            return Some(end_tag);
        }

        // ...or be followed by whitespace, newline, or '>'.
        let s = text_bytes[tag_len];
        if is_ascii_whitespace(s) || s == b'>' {
            return Some(end_tag);
        }
    }

    for (beg_tag, end_tag) in ST_BEGIN_TAGS.iter().zip(["-->", "?>", "]]>"].iter()) {
        if text_bytes.starts_with(beg_tag) {
            return Some(end_tag);
        }
    }

    if text_bytes.len() > 1 && text_bytes[0] == b'!' && text_bytes[1].is_ascii_alphabetic() {
        Some(">")
    } else {
        None
    }
}

fn surgerize_tight_list(tree: &mut Tree<Item>, list_ix: TreeIndex) {
    let mut list_item = tree[list_ix].child;
    while let Some(listitem_ix) = list_item {
        let mut node_ix = tree[listitem_ix].child;
        while let Some(node) = node_ix {
            if let ItemBody::Paragraph = tree[node].item.body {
                tree[node].item.body = ItemBody::TightParagraph;
            }
            node_ix = tree[node].next;
        }

        list_item = tree[listitem_ix].next;
    }
}

fn fixup_end_of_definition_list(tree: &mut Tree<Item>, list_ix: TreeIndex) {
    let mut list_item = tree[list_ix].child;
    let mut previous_list_item = None;
    while let Some(listitem_ix) = list_item {
        match &mut tree[listitem_ix].item.body {
            ItemBody::DefinitionListTitle | ItemBody::DefinitionListDefinition(_) => {
                previous_list_item = list_item;
                list_item = tree[listitem_ix].next;
            }
            body @ ItemBody::MaybeDefinitionListTitle => {
                *body = ItemBody::Paragraph;
                break;
            }
            _ => break,
        }
    }
    if let Some(previous_list_item) = previous_list_item {
        tree.truncate_to_parent(previous_list_item);
    }
}

/// Determines whether the delimiter run starting at given index is
/// left-flanking, as defined by the commonmark spec (and isn't intraword
/// for _ delims).
/// suffix is &s[ix..], which is passed in as an optimization, since taking
/// a string subslice is O(n).
fn delim_run_can_open(
    s: &str,
    suffix: &str,
    run_len: usize,
    ix: usize,
    mode: TableParseMode,
    options: Options,
) -> bool {
    let next_char = if let Some(c) = suffix[run_len..].chars().next() {
        c
    } else {
        return false;
    };
    if next_char.is_whitespace() {
        return false;
    }
    if ix == 0 {
        return true;
    }
    if mode == TableParseMode::Active {
        if s.as_bytes()[..ix].ends_with(b"|") && !s.as_bytes()[..ix].ends_with(br"\|") {
            return true;
        }
        if next_char == '|' {
            return false;
        }
    }
    let delim = suffix.bytes().next().unwrap();
    if delim == b'*' && (next_char == '*' || next_char == '_' || next_char == '~') {
        return true;
    }
    if (delim == b'*' || delim == b'^') && !is_punctuation(next_char) {
        return true;
    }
    if delim == b'~' && run_len > 1 {
        return true;
    }
    let prev_char = s[..ix].chars().last().unwrap();
    if delim == b'~'
        && (prev_char == '~' || options.contains(Options::ENABLE_SUBSCRIPT))
        && !is_punctuation(next_char)
    {
        return true;
    }
    if delim == b'~' && options.contains(Options::ENABLE_STRIKETHROUGH) && run_len == 1 {
        return !is_punctuation(next_char)
            || (is_punctuation(next_char)
                && (prev_char.is_whitespace() || is_punctuation(prev_char)));
    }

    prev_char.is_whitespace()
        || is_punctuation(prev_char) && (delim != b'\'' || ![']', ')'].contains(&prev_char))
}

fn delim_run_can_close(
    s: &str,
    suffix: &str,
    run_len: usize,
    ix: usize,
    mode: TableParseMode,
    options: Options,
) -> bool {
    if ix == 0 {
        return false;
    }
    let prev_char = s[..ix].chars().last().unwrap();
    if prev_char.is_whitespace() {
        return false;
    }
    let next_char = if let Some(c) = suffix[run_len..].chars().next() {
        c
    } else {
        return true;
    };
    if mode == TableParseMode::Active {
        if s.as_bytes()[..ix].ends_with(b"|") && !s.as_bytes()[..ix].ends_with(br"\|") {
            return false;
        }
        if next_char == '|' {
            return true;
        }
    }
    let delim = suffix.bytes().next().unwrap();
    if delim == b'*' && (prev_char == '*' || prev_char == '_' || prev_char == '~') {
        return true;
    }
    if (delim == b'*' || delim == b'^') && !is_punctuation(prev_char) {
        return true;
    }
    if delim == b'~' && run_len > 1 && !is_punctuation(prev_char) {
        return true;
    }
    if delim == b'~' && (prev_char == '~' || options.contains(Options::ENABLE_SUBSCRIPT)) {
        return true;
    }
    if delim == b'~' && options.contains(Options::ENABLE_STRIKETHROUGH) && run_len == 1 {
        return !is_punctuation(prev_char)
            || (is_punctuation(prev_char)
                && (next_char.is_whitespace() || is_punctuation(next_char)));
    }

    next_char.is_whitespace() || is_punctuation(next_char)
}

fn create_lut(options: &Options) -> LookupTable {
    #[cfg(all(target_arch = "x86_64", feature = "simd"))]
    {
        LookupTable {
            simd: simd::compute_lookup(options),
            scalar: special_bytes(options),
        }
    }
    #[cfg(not(all(target_arch = "x86_64", feature = "simd")))]
    {
        special_bytes(options)
    }
}

fn special_bytes(options: &Options) -> [bool; 256] {
    let mut bytes = [false; 256];
    let standard_bytes = [
        b'\n', b'\r', b'*', b'_', b'&', b'\\', b'[', b']', b'<', b'!', b'`',
    ];

    for &byte in &standard_bytes {
        bytes[byte as usize] = true;
    }
    if options.contains(Options::ENABLE_TABLES) {
        bytes[b'|' as usize] = true;
    }
    if options.contains(Options::ENABLE_STRIKETHROUGH)
        || options.contains(Options::ENABLE_SUBSCRIPT)
    {
        bytes[b'~' as usize] = true;
    }
    if options.contains(Options::ENABLE_SUPERSCRIPT) {
        bytes[b'^' as usize] = true;
    }
    if options.contains(Options::ENABLE_MATH) {
        bytes[b'$' as usize] = true;
        bytes[b'{' as usize] = true;
        bytes[b'}' as usize] = true;
    }
    if options.contains(Options::ENABLE_MDX) {
        bytes[b'{' as usize] = true;
        bytes[b'}' as usize] = true;
    }
    if options.has_smart_ellipses() {
        bytes[b'.' as usize] = true;
    }
    if options.has_smart_dashes() {
        bytes[b'-' as usize] = true;
    }
    if options.has_smart_quotes() {
        bytes[b'"' as usize] = true;
        bytes[b'\'' as usize] = true;
    }
    if options.contains(Options::ENABLE_DIRECTIVE) {
        bytes[b':' as usize] = true;
    }

    bytes
}

enum LoopInstruction<T> {
    /// Continue looking for more special bytes, but skip next few bytes.
    ContinueAndSkip(usize),
    /// Break looping immediately, returning with the given index and value.
    BreakAtWith(usize, T),
}

#[cfg(all(target_arch = "x86_64", feature = "simd"))]
struct LookupTable {
    simd: [u8; 16],
    scalar: [bool; 256],
}

#[cfg(not(all(target_arch = "x86_64", feature = "simd")))]
type LookupTable = [bool; 256];

/// This function walks the byte slices from the given index and
/// calls the callback function on all bytes (and their indices) that are in the following set:
/// `` ` ``, `\`, `&`, `*`, `_`, `~`, `!`, `<`, `[`, `]`, `|`, `\r`, `\n`
/// It is guaranteed not call the callback on other bytes.
/// Whenever `callback(ix, byte)` returns a `ContinueAndSkip(n)` value, the callback
/// will not be called with an index that is less than `ix + n + 1`.
/// When the callback returns a `BreakAtWith(end_ix, opt+val)`, no more callbacks will be
/// called and the function returns immediately with the return value `(end_ix, opt_val)`.
/// If `BreakAtWith(..)` is never returned, this function will return the first
/// index that is outside the byteslice bound and a `None` value.
fn iterate_special_bytes<F, T>(
    lut: &LookupTable,
    bytes: &[u8],
    ix: usize,
    callback: F,
) -> (usize, Option<T>)
where
    F: FnMut(usize, u8) -> LoopInstruction<Option<T>>,
{
    #[cfg(all(target_arch = "x86_64", feature = "simd"))]
    {
        simd::iterate_special_bytes(lut, bytes, ix, callback)
    }
    #[cfg(not(all(target_arch = "x86_64", feature = "simd")))]
    {
        scalar_iterate_special_bytes(lut, bytes, ix, callback)
    }
}

fn scalar_iterate_special_bytes<F, T>(
    lut: &[bool; 256],
    bytes: &[u8],
    mut ix: usize,
    mut callback: F,
) -> (usize, Option<T>)
where
    F: FnMut(usize, u8) -> LoopInstruction<Option<T>>,
{
    while ix < bytes.len() {
        let b = bytes[ix];
        if lut[b as usize] {
            match callback(ix, b) {
                LoopInstruction::ContinueAndSkip(skip) => {
                    ix += skip;
                }
                LoopInstruction::BreakAtWith(ix, val) => {
                    return (ix, val);
                }
            }
        }
        ix += 1;
    }

    (ix, None)
}

/// Split the usual heading content range and the content inside the trailing attribute block.
///
/// Returns `(leading_content_len, Option<trailing_attr_block_range>)`.
///
/// Note that `trailing_attr_block_range` will be empty range when the block
/// is `{}`, since the range is content inside the wrapping `{` and `}`.
///
/// The closing `}` of an attribute block can have trailing whitespaces.
/// They are automatically trimmed when the attribute block is being searched.
///
/// However, this method does not trim the trailing whitespaces of heading content.
/// It is callers' responsibility to trim them if necessary.
fn extract_attribute_block_content_from_header_text(
    heading: &[u8],
) -> (usize, Option<Range<usize>>) {
    let heading_len = heading.len();
    let mut ix = heading_len;
    ix -= scan_rev_while(heading, |b| {
        b == b'\n' || b == b'\r' || b == b' ' || b == b'\t'
    });
    if ix == 0 {
        return (heading_len, None);
    }

    let attr_block_close = ix - 1;
    if heading.get(attr_block_close) != Some(&b'}') {
        // The last character is not `}`. No attribute blocks found.
        return (heading_len, None);
    }
    // move cursor before the closing right brace (`}`)
    ix -= 1;

    ix -= scan_rev_while(&heading[..ix], |b| {
        // Characters to be excluded:
        //  * `{` and `}`: special characters to open and close an attribute block.
        //  * `\\`: a special character to escape many characters and disable some syntaxes.
        //      + Handling of this escape character differs among markdown processors.
        //      + Escaped characters will be separate text node from neighbors, so
        //        it is not easy to handle unescaped string and trim the trailing block.
        //  * `<` and `>`: special characters to start and end HTML tag.
        //      + No known processors converts `{#<i>foo</i>}` into
        //        `id="&lt;i&gt;foo&lt;/&gt;"` as of this writing, so hopefully
        //        this restriction won't cause compatibility issues.
        //  * `\n` and `\r`: a newline character.
        //      + Setext heading can have multiple lines. However it is hard to support
        //        attribute blocks that have newline inside, since the parsing proceeds line by
        //        line and lines will be separate nodes even they are logically a single text.
        !matches!(b, b'{' | b'}' | b'<' | b'>' | b'\\' | b'\n' | b'\r')
    });
    if ix == 0 {
        // `{` is not found. No attribute blocks available.
        return (heading_len, None);
    }
    let attr_block_open = ix - 1;
    if heading[attr_block_open] != b'{' {
        // `{` is not found. No attribute blocks available.
        return (heading_len, None);
    }

    (attr_block_open, Some(ix..attr_block_close))
}

/// Parses an attribute block content, such as `.class1 #id .class2`.
///
/// Returns `(id, classes)`.
///
/// It is callers' responsibility to find opening and closing characters of the attribute
/// block. Usually [`extract_attribute_block_content_from_header_text`] function does it for you.
///
/// Note that this parsing requires explicit whitespace separators between
/// attributes. This is intentional design with the reasons below:
///
/// * to keep conversion simple and easy to understand for any possible input,
/// * to avoid adding less obvious conversion rule that can reduce compatibility
///   with other implementations more, and
/// * to follow the major design of implementations with the support for the
///   attribute blocks extension (as of this writing).
///
/// See also: [`Options::ENABLE_HEADING_ATTRIBUTES`].
///
/// [`Options::ENABLE_HEADING_ATTRIBUTES`]: `crate::Options::ENABLE_HEADING_ATTRIBUTES`
fn parse_inside_attribute_block(inside_attr_block: &str) -> Option<HeadingAttributes<'_>> {
    let mut id = None;
    let mut classes = Vec::new();
    let mut attrs = Vec::new();

    for attr in inside_attr_block.split_ascii_whitespace() {
        // iterator returned by `str::split_ascii_whitespace` never emits empty
        // strings, so taking first byte won't panic.
        if attr.len() > 1 {
            let first_byte = attr.as_bytes()[0];
            if first_byte == b'#' {
                id = Some(attr[1..].into());
            } else if first_byte == b'.' {
                classes.push(attr[1..].into());
            } else {
                let split = attr.split_once('=');
                if let Some((key, value)) = split {
                    attrs.push((key.into(), Some(value.into())));
                } else {
                    attrs.push((attr.into(), None));
                }
            }
        }
    }

    if id.is_none() && classes.is_empty() && attrs.is_empty() {
        return None;
    }
    Some(HeadingAttributes { id, classes, attrs })
}

#[cfg(all(target_arch = "x86_64", feature = "simd"))]
mod simd {
    //! SIMD byte scanning logic.
    //!
    //! This module provides functions that allow walking through byteslices, calling
    //! provided callback functions on special bytes and their indices using SIMD.
    //! The byteset is defined in `compute_lookup`.
    //!
    //! The idea is to load in a chunk of 16 bytes and perform a lookup into a set of
    //! bytes on all the bytes in this chunk simultaneously. We produce a 16 bit bitmask
    //! from this and call the callback on every index corresponding to a 1 in this mask
    //! before moving on to the next chunk. This allows us to move quickly when there
    //! are no or few matches.
    //!
    //! The table lookup is inspired by this [great overview]. However, since all of the
    //! bytes we're interested in are ASCII, we don't quite need the full generality of
    //! the universal algorithm and are hence able to skip a few instructions.
    //!
    //! [great overview]: http://0x80.pl/articles/simd-byte-lookup.html

    use core::arch::x86_64::*;

    use super::{LookupTable, LoopInstruction};
    use crate::Options;

    const VECTOR_SIZE: usize = core::mem::size_of::<__m128i>();

    /// Generates a lookup table containing the bitmaps for our
    /// special marker bytes. This is effectively a 128 element 2d bitvector,
    /// that can be indexed by a four bit row index (the lower nibble)
    /// and a three bit column index (upper nibble).
    pub(super) fn compute_lookup(options: &Options) -> [u8; 16] {
        let mut lookup = [0u8; 16];
        let standard_bytes = [
            b'\n', b'\r', b'*', b'_', b'&', b'\\', b'[', b']', b'<', b'!', b'`',
        ];

        for &byte in &standard_bytes {
            add_lookup_byte(&mut lookup, byte);
        }
        if options.contains(Options::ENABLE_TABLES) {
            add_lookup_byte(&mut lookup, b'|');
        }
        if options.contains(Options::ENABLE_STRIKETHROUGH)
            || options.contains(Options::ENABLE_SUBSCRIPT)
        {
            add_lookup_byte(&mut lookup, b'~');
        }
        if options.contains(Options::ENABLE_SUPERSCRIPT) {
            add_lookup_byte(&mut lookup, b'^');
        }
        if options.contains(Options::ENABLE_MATH) {
            add_lookup_byte(&mut lookup, b'$');
            add_lookup_byte(&mut lookup, b'{');
            add_lookup_byte(&mut lookup, b'}');
        }
        if options.has_smart_ellipses() {
            add_lookup_byte(&mut lookup, b'.');
        }
        if options.has_smart_dashes() {
            add_lookup_byte(&mut lookup, b'-');
        }
        if options.has_smart_quotes() {
            add_lookup_byte(&mut lookup, b'"');
            add_lookup_byte(&mut lookup, b'\'');
        }

        lookup
    }

    fn add_lookup_byte(lookup: &mut [u8; 16], byte: u8) {
        lookup[(byte & 0x0f) as usize] |= 1 << (byte >> 4);
    }

    /// Computes a bit mask for the given byteslice starting from the given index,
    /// where the 16 least significant bits indicate (by value of 1) whether or not
    /// there is a special character at that byte position. The least significant bit
    /// corresponds to `bytes[ix]` and the most significant bit corresponds to
    /// `bytes[ix + 15]`.
    /// It is only safe to call this function when `bytes.len() >= ix + VECTOR_SIZE`.
    #[target_feature(enable = "ssse3")]
    #[inline]
    unsafe fn compute_mask(lut: &[u8; 16], bytes: &[u8], ix: usize) -> i32 {
        debug_assert!(bytes.len() >= ix + VECTOR_SIZE);

        let bitmap = _mm_loadu_si128(lut.as_ptr() as *const __m128i);
        // Small lookup table to compute single bit bitshifts
        // for 16 bytes at once.
        let bitmask_lookup =
            _mm_setr_epi8(1, 2, 4, 8, 16, 32, 64, -128, -1, -1, -1, -1, -1, -1, -1, -1);

        // Load input from memory.
        let raw_ptr = bytes.as_ptr().add(ix) as *const __m128i;
        let input = _mm_loadu_si128(raw_ptr);
        // Compute the bitmap using the bottom nibble as an index
        // into the lookup table. Note that non-ascii bytes will have
        // their most significant bit set and will map to lookup[0].
        let bitset = _mm_shuffle_epi8(bitmap, input);
        // Compute the high nibbles of the input using a 16-bit rightshift of four
        // and a mask to prevent most-significant bit issues.
        let higher_nibbles = _mm_and_si128(_mm_srli_epi16(input, 4), _mm_set1_epi8(0x0f));
        // Create a bitmask for the bitmap by perform a left shift of the value
        // of the higher nibble. Bytes with their most significant set are mapped
        // to -1 (all ones).
        let bitmask = _mm_shuffle_epi8(bitmask_lookup, higher_nibbles);
        // Test the bit of the bitmap by AND'ing the bitmap and the mask together.
        let tmp = _mm_and_si128(bitset, bitmask);
        // Check whether the result was not null. NEQ is not a SIMD intrinsic,
        // but comparing to the bitmask is logically equivalent. This also prevents us
        // from matching any non-ASCII bytes since none of the bitmaps were all ones
        // (-1).
        let result = _mm_cmpeq_epi8(tmp, bitmask);

        // Return the resulting bitmask.
        _mm_movemask_epi8(result)
    }

    /// Calls callback on byte indices and their value.
    /// Breaks when callback returns LoopInstruction::BreakAtWith(ix, val). And skips the
    /// number of bytes in callback return value otherwise.
    /// Returns the final index and a possible break value.
    pub(super) fn iterate_special_bytes<F, T>(
        lut: &LookupTable,
        bytes: &[u8],
        ix: usize,
        callback: F,
    ) -> (usize, Option<T>)
    where
        F: FnMut(usize, u8) -> LoopInstruction<Option<T>>,
    {
        if is_x86_feature_detected!("ssse3") && bytes.len() >= VECTOR_SIZE {
            unsafe { simd_iterate_special_bytes(&lut.simd, bytes, ix, callback) }
        } else {
            super::scalar_iterate_special_bytes(&lut.scalar, bytes, ix, callback)
        }
    }

    /// Calls the callback function for every 1 in the given bitmask with
    /// the index `offset + ix`, where `ix` is the position of the 1 in the mask.
    /// Returns `Ok(ix)` to continue from index `ix`, `Err((end_ix, opt_val)` to break with
    /// final index `end_ix` and optional value `opt_val`.
    unsafe fn process_mask<F, T>(
        mut mask: i32,
        bytes: &[u8],
        mut offset: usize,
        callback: &mut F,
    ) -> Result<usize, (usize, Option<T>)>
    where
        F: FnMut(usize, u8) -> LoopInstruction<Option<T>>,
    {
        while mask != 0 {
            let mask_ix = mask.trailing_zeros() as usize;
            offset += mask_ix;
            match callback(offset, *bytes.get_unchecked(offset)) {
                LoopInstruction::ContinueAndSkip(skip) => {
                    offset += skip + 1;
                    let shift = skip + 1 + mask_ix;
                    if shift >= 32 {
                        break;
                    }
                    mask >>= shift;
                }
                LoopInstruction::BreakAtWith(ix, val) => return Err((ix, val)),
            }
        }
        Ok(offset)
    }

    #[target_feature(enable = "ssse3")]
    /// Important: only call this function when `bytes.len() >= 16`. Doing
    /// so otherwise may exhibit undefined behaviour.
    unsafe fn simd_iterate_special_bytes<F, T>(
        lut: &[u8; 16],
        bytes: &[u8],
        mut ix: usize,
        mut callback: F,
    ) -> (usize, Option<T>)
    where
        F: FnMut(usize, u8) -> LoopInstruction<Option<T>>,
    {
        debug_assert!(bytes.len() >= VECTOR_SIZE);
        let upperbound = bytes.len() - VECTOR_SIZE;

        while ix < upperbound {
            let mask = compute_mask(lut, bytes, ix);
            let block_start = ix;
            ix = match process_mask(mask, bytes, ix, &mut callback) {
                Ok(ix) => core::cmp::max(ix, VECTOR_SIZE + block_start),
                Err((end_ix, val)) => return (end_ix, val),
            };
        }

        if bytes.len() > ix {
            // shift off the bytes at start we have already scanned
            let mask = compute_mask(lut, bytes, upperbound) >> (ix - upperbound);
            if let Err((end_ix, val)) = process_mask(mask, bytes, ix, &mut callback) {
                return (end_ix, val);
            }
        }

        (bytes.len(), None)
    }

    #[cfg(test)]
    mod simd_test {
        use super::{super::create_lut, iterate_special_bytes, LoopInstruction};
        use crate::Options;

        fn check_expected_indices(bytes: &[u8], expected: &[usize], skip: usize) {
            let mut opts = Options::empty();
            opts.insert(Options::ENABLE_MATH);
            opts.insert(Options::ENABLE_TABLES);
            opts.insert(Options::ENABLE_FOOTNOTES);
            opts.insert(Options::ENABLE_STRIKETHROUGH);
            opts.insert(Options::ENABLE_SUPERSCRIPT);
            opts.insert(Options::ENABLE_TASKLISTS);

            let lut = create_lut(&opts);
            let mut indices = vec![];

            iterate_special_bytes::<_, i32>(&lut, bytes, 0, |ix, _byte_ty| {
                indices.push(ix);
                LoopInstruction::ContinueAndSkip(skip)
            });

            assert_eq!(&indices[..], expected);
        }

        #[test]
        fn simple_no_match() {
            check_expected_indices("abcdef0123456789".as_bytes(), &[], 0);
        }

        #[test]
        fn simple_match() {
            check_expected_indices("*bcd&f0123456789".as_bytes(), &[0, 4], 0);
        }

        #[test]
        fn single_open_fish() {
            check_expected_indices("<".as_bytes(), &[0], 0);
        }

        #[test]
        fn long_match() {
            check_expected_indices("0123456789abcde~*bcd&f0".as_bytes(), &[15, 16, 20], 0);
        }

        #[test]
        fn border_skip() {
            check_expected_indices("0123456789abcde~~~~d&f0".as_bytes(), &[15, 20], 3);
        }

        #[test]
        fn exhaustive_search() {
            let chars = [
                b'\n', b'\r', b'*', b'_', b'~', b'^', b'|', b'&', b'\\', b'[', b']', b'<', b'!',
                b'`', b'$', b'{', b'}',
            ];

            for &c in &chars {
                for i in 0u8..=255 {
                    if !chars.contains(&i) {
                        // full match
                        let mut buf = [i; 18];
                        buf[3] = c;
                        buf[6] = c;

                        check_expected_indices(&buf[..], &[3, 6], 0);
                    }
                }
            }
        }
    }
}
