//! The first pass resolves all block structure, generating an AST. Within a block, items
//! are in a linear chain with potential inline markup identified.

use alloc::{string::String, vec::Vec};
use core::{cmp::max, ops::Range};

use unicase::UniCase;

#[cfg(feature = "mdx")]
use crate::mdx::*;
use crate::{
    linklabel::{scan_link_label_rest, LinkLabel},
    parse::{
        scan_containers, Allocations, DirectiveAttrData, FootnoteDef, HeadingAttributes, Item,
        ItemBody, LinkDef, LINK_MAX_NESTED_PARENS,
    },
    post_passes::{scan_autolink_literal, scan_email_autolink},
    scanners::*,
    strings::CowStr,
    tree::{Tree, TreeIndex},
    HeadingLevel, LinkType, MetadataBlockKind, Options,
};

pub(crate) fn run_first_pass(
    text: &str,
    options: Options,
) -> (Tree<Item>, Allocations<'_>, Vec<(usize, String)>) {
    let start_capacity = max(128, text.len() / 32);
    let lookup_table = &create_lut(&options);
    let first_pass = FirstPass {
        text,
        tree: Tree::with_capacity(start_capacity),
        begin_list_item: None,
        last_line_blank: false,
        list_interrupted_paragraph: false,
        refdef_interrupted_paragraph: false,
        allocs: Allocations::new(),
        options,
        lookup_table,
        brace_context_next: 0,
        brace_context_stack: Vec::new(),
        mdx_errors: Vec::new(),
        #[cfg(feature = "mdx")]
        mdx_expr_allocator: oxc_allocator::Allocator::default(),
        pending_lazy_blockquote_close: false,
        doc_start: 0,
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
    // Narrower variant set after a link reference definition: the def's
    // residual paragraph state interrupts only ordered-list markers
    // whose index ≠ 1. Unlike `list_interrupted_paragraph`, it must not
    // suppress blank-after-marker list items, which mdx-js/micromark
    // still accept after a refdef (`[r]: /x\n\n-\n- b` → list with
    // empty + filled items, not paragraph + list).
    refdef_interrupted_paragraph: bool,
    pub(crate) allocs: Allocations<'a>,
    pub(crate) options: Options,
    lookup_table: &'b LookupTable,
    /// Math environment brace nesting.
    brace_context_stack: Vec<u8>,
    brace_context_next: usize,
    /// MDX errors collected during first pass.
    pub(crate) mdx_errors: Vec<(usize, String)>,
    /// Mirrors micromark's `lazy[line]` for indented-code-after-blockquote-
    /// close. Set when `parse_block` pops a blockquote due to failed
    /// continuation; consumed (and cleared) by the next indented code block,
    /// which becomes one-line-only — matching `furtherStart`'s lazy-line
    /// rejection. Without this, `>\n    bar\n    baz` would merge `bar`
    /// and `baz` into one code block instead of splitting.
    pending_lazy_blockquote_close: bool,
    /// Reusable bump allocator for oxc parses (expression-body validation,
    /// ESM completeness checks). Avoids `Allocator::default()` heap alloc
    /// on every expression — the allocator is `reset()` between parses.
    #[cfg(feature = "mdx")]
    pub(crate) mdx_expr_allocator: oxc_allocator::Allocator,
    /// Byte offset where the document's parseable content begins — 3 when
    /// a UTF-8 BOM is stripped, 0 otherwise. Used to gate
    /// "must-be-at-doc-start" constructs (frontmatter) without anchoring
    /// them at literal byte 0.
    pub(crate) doc_start: usize,
}

impl<'a, 'b> FirstPass<'a, 'b> {
    fn run(mut self) -> (Tree<Item>, Allocations<'a>, Vec<(usize, String)>) {
        // Skip a leading UTF-8 BOM (`U+FEFF` = EF BB BF). micromark treats it
        // as a zero-width prefix, not part of the first block — without this,
        // `\u{FEFF}# Title` becomes a paragraph because the `#` isn't at the
        // line start. Positions are byte offsets into the original source so
        // skipping bytes is fine; downstream nodes just won't reference [0..3).
        let mut ix = 0;
        if self.text.as_bytes().starts_with(b"\xEF\xBB\xBF") {
            ix = 3;
            self.doc_start = 3;
        }
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
            self.refdef_interrupted_paragraph = false;
            // If any popped container was a blockquote AND the current line
            // is non-blank, the next indented code block on this line is
            // "lazy" wrt the blockquote — micromark would limit it to one
            // line. Skip the marker on blank lines: a blank line that pops
            // the blockquote is a *proper* close, after which subsequent
            // indented code blocks merge normally (e.g. `> Foo\n\n    bar
            // \n    baz` keeps `bar\nbaz` as one code block).
            let probe_ix = start_ix + line_start.bytes_scanned();
            let line_is_blank = scan_blank_line(&bytes[probe_ix..]).is_some();
            if !line_is_blank
                && !self.pending_lazy_blockquote_close
                && self.tree.walk_spine().skip(i).any(|&ix| {
                    matches!(
                        self.tree[ix].item.body,
                        ItemBody::BlockQuote(..) | ItemBody::ListItem(..)
                    )
                })
            {
                self.pending_lazy_blockquote_close = true;
            }
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
            if let Some((ch, index, indent)) = line_start.scan_list_marker_with_indent_and_clamp(
                outer_indent,
                !self.options.contains(Options::ENABLE_MDX),
            ) {
                let after_marker_index = start_ix + line_start.bytes_scanned();
                let already_in_list = self
                    .tree
                    .peek_up()
                    .is_some_and(|ix| matches!(self.tree[ix].item.body, ItemBody::List(_, _, _)));
                let after_marker_blank = {
                    let rest = &bytes[after_marker_index..];
                    rest.is_empty() || scan_blank_line(rest).is_some()
                };
                // An empty list marker can't interrupt a paragraph — including
                // the residual paragraph state left by a refdef. After
                // `[a]: u\n>*`, the `*` inside the new blockquote should be
                // paragraph text, not an empty list item, because micromark's
                // paragraph token is still considered "interrupted".
                if (self.list_interrupted_paragraph || self.refdef_interrupted_paragraph)
                    && !already_in_list
                    && after_marker_blank
                {
                    self.list_interrupted_paragraph = false;
                    self.refdef_interrupted_paragraph = false;
                    line_start = save;
                    break;
                }
                // micromark's list construct rejects start != 1 when
                // `self.interrupt` is set (mirrors `currentConstruct` lingering
                // after the previous block). After an indented code block,
                // `2. b` becomes a paragraph rather than starting a fresh
                // ordered list (`    code\n\n2. b` → `[code, paragraph]`).
                // Index-1 starts are always allowed.
                //
                // The check is TEXTUAL: only an exact `1.` or `1)` (single
                // digit, no leading zero) opens a list across a paragraph
                // boundary. `01.`, `001.`, `10.` all stay paragraph text —
                // micromark scans the literal marker bytes, not the numeric
                // value.
                //
                // This applies even when *nested* inside an existing list —
                // after `[ref]: /uri\n1. - 2. foo` the `2.` inside the inner
                // unordered list-item also fails the interrupt check, so it
                // stays as paragraph text instead of opening a new ordered
                // list. (Index-1 markers and any markers with a clear flag
                // are unaffected.)
                let is_textual_one = (ch == b'.' || ch == b')')
                    && bytes.get(container_start) == Some(&b'1')
                    && bytes
                        .get(container_start + 1)
                        .is_some_and(|&b| b == b'.' || b == b')');
                if (self.list_interrupted_paragraph || self.refdef_interrupted_paragraph)
                    && (ch == b'.' || ch == b')')
                    && !is_textual_one
                {
                    self.list_interrupted_paragraph = false;
                    self.refdef_interrupted_paragraph = false;
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
                                    self.options.contains(Options::ENABLE_MATH_MULTI_DOLLAR),
                                    self.options.contains(Options::ENABLE_DIRECTIVE),
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
                                    self.options.contains(Options::ENABLE_MATH_MULTI_DOLLAR),
                                    self.options.contains(Options::ENABLE_DIRECTIVE),
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
                    if let Some((mut dir_data, content_end)) =
                        parse_directive_after_colons(self.text, bytes, after_colons)
                    {
                        // initialSize per micromark-extension-directive: cols of
                        // leading whitespace before `:::` *after* outer-container
                        // prefix stripping. Drives the dedent for multi-line MDX
                        // expression attribute values nested in the body.
                        dir_data.initial_size = outer_indent.min(u8::MAX as usize) as u8;
                        // Close any open list before opening a sibling directive,
                        // matching how blockquote handles the same transition.
                        self.finish_list(start_ix);
                        // For block directives, advance to end of line
                        let after = &bytes[content_end..];
                        let ws = scan_whitespace_no_nl(after);
                        let line_end = content_end + ws + scan_nextline(&after[ws..]);
                        // `[label]` offsets (0/0 when no brackets) — capture
                        // before `dir_data` is moved into the allocator.
                        let label_start = dir_data.label_start;
                        let label_end = dir_data.label_end;
                        let dir_ix = self.allocs.allocate_directive(dir_data);
                        self.tree.append(Item {
                            start: container_start,
                            end: 0,
                            body: ItemBody::ContainerDirective(fence_length as u8, dir_ix),
                        });
                        self.tree.push();
                        // Emit the label as a real inline-tokenized child so the
                        // normal inline pass resolves emphasis/strong/links/code.
                        if label_start != 0 || label_end != 0 {
                            self.append_container_directive_label(label_start, label_end);
                        }
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
                            let label_start = dir_data.label_start;
                            let label_end = dir_data.label_end;
                            let dir_ix = self.allocs.allocate_directive(dir_data);
                            self.tree.append(Item {
                                start: container_start,
                                end: line_end,
                                body: ItemBody::LeafDirective(dir_ix),
                            });
                            // The label is the directive's inline content: tokenize
                            // it directly as children (no label paragraph).
                            if label_start < label_end {
                                self.tree.push();
                                self.parse_line(
                                    label_start,
                                    Some(label_end),
                                    TableParseMode::Disabled,
                                );
                                self.tree.pop();
                            }
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
            // A blank line breaks the "lazy-immediately-after-close"
            // restriction: micromark's `furtherStart` only rejects lazy
            // lines that directly follow the implicit close, not ones with
            // a blank line in between. Clearing here lets a subsequent
            // indented code block parse as a normal multi-line block.
            self.pending_lazy_blockquote_close = false;
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
                    ItemBody::BlockQuote(..) => {
                        // Blank `>` line inside a blockquote separates whatever
                        // closed the outer paragraph from the next block within
                        // the blockquote. Clear `list_interrupted_paragraph`
                        // so an empty `-` marker on the following blockquote
                        // line opens a fresh list (matches mdx-js/micromark:
                        // `_\n>\n>-` → paragraph + blockquote(list(empty))).
                        self.list_interrupted_paragraph = false;
                    }
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
                // If finish_list will pop an EMPTY list item (marker
                // followed by blank, no content), the subsequent indented
                // code block becomes "lazy" — mirrors micromark's
                // `furtherStart` restriction. Without this,
                // `*\n\n      bar\n      baz` would merge into one code
                // block; remark separates them. The `begin_list_item`
                // flag distinguishes the empty case (set on blank-after-
                // marker open) from list items whose first content is
                // still being processed (`-             bbb` has 12+
                // spaces of post-marker indent that's an indented code
                // block, but the list item is NOT empty in source).
                // Only fire when a real blank line separates the marker
                // from the indented content (`*\n\n    foo` → split). For
                // the directly-following case (`-\n             bbb` →
                // indented code stays IN the list item), don't fire.
                // begin_list_item records the byte right after the
                // marker's line ending; if start_ix > that, a blank line
                // sat between.
                let empty_listitem_will_close =
                    self.begin_list_item.is_some_and(|bli| start_ix > bli)
                        && self.tree.peek_up().is_some_and(|ix| {
                            matches!(self.tree[ix].item.body, ItemBody::ListItem(..))
                                && self.tree[ix].child.is_none()
                        });
                self.finish_list(start_ix);
                if empty_listitem_will_close {
                    self.pending_lazy_blockquote_close = true;
                }
                let ix = start_ix + line_start.bytes_scanned();
                let remaining_space = line_start.remaining_space();
                return self.parse_indented_code_block(content_start_ix, ix, remaining_space);
            }
        }

        let ix = start_ix + line_start.bytes_scanned();

        // Metadata blocks cannot be indented, and — matching remark-frontmatter
        // — only match at the very start of the document. `doc_start` is 0
        // normally, or 3 when a UTF-8 BOM was stripped.
        if indent == 0 && ix == self.doc_start && self.tree.spine_len() == 0 {
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
        #[cfg(feature = "mdx")]
        if self.options.contains(Options::ENABLE_MDX) {
            // MDX ESM: lines starting with `import` or `export`.
            // ESM is only valid at the document root — but a still-open
            // List (parent of a closed-empty ListItem) doesn't count as a
            // container that would forbid ESM. The list will close before
            // the ESM block.
            let at_root_for_esm = indent == 0
                && self
                    .tree
                    .walk_spine()
                    .all(|&ix| matches!(self.tree[ix].item.body, ItemBody::List(..)));
            if at_root_for_esm {
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
            // Only the part of `indent` consumed from leftover tab-expansion
            // spaces needs to be synthesized — those don't have raw bytes in
            // [content_start_ix..]. Indent consumed from actual space bytes is
            // already present in the source range. `scan_space_inner` drains
            // `spaces_remaining` first, so the tab-leftover portion is
            // `min(saved_remaining_space, indent)`.
            let synth = remaining_space.min(indent);

            // Types 1-5 are all detected by one function and all end with the same
            // pattern
            if let Some(html_end_tag) = get_html_end_tag(&bytes[(ix + 1)..]) {
                self.finish_list(start_ix);
                return self.parse_html_block_type_1_to_5(content_start_ix, html_end_tag, synth, 0);
            }

            // Detect type 6
            if starts_html_block_type_6(&bytes[(ix + 1)..]) {
                self.finish_list(start_ix);
                return self.parse_html_block_type_6_or_7(content_start_ix, synth, 0);
            }

            // Detect type 7
            if let Some(_html_bytes) = scan_html_type_7(&bytes[ix..]) {
                self.finish_list(start_ix);
                return self.parse_html_block_type_6_or_7(content_start_ix, synth, 0);
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

        if self.options.contains(Options::ENABLE_MATH_MULTI_DOLLAR) {
            if let Some(n) = scan_math_fence(&bytes[ix..]) {
                self.finish_list(start_ix);
                return self.parse_math_block(ix, indent, n);
            }
        }

        // parse refdef
        while let Some((bytecount, label, link_def)) =
            self.parse_refdef_total(start_ix + line_start.bytes_scanned())
        {
            self.allocs
                .refdefs_all
                .push((label.clone(), link_def.clone()));
            self.allocs.refdefs.0.entry(label).or_insert(link_def);
            let container_start = start_ix + line_start.bytes_scanned();
            let mut ix = container_start + bytecount;
            // A blank line between the refdef and the next interrupting block
            // means micromark has already closed the refdef's residual paragraph
            // before that next block opens — so the new block isn't "interrupting"
            // an open paragraph and `refdef_interrupted_paragraph` must not fire.
            let refdef_terminated_by_blank;
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
                // `scan_blank_line` happily consumes the refdef's own terminator
                // newline. A SECOND blank line at the new position is what
                // distinguishes "refdef\nnext-block" (still in paragraph)
                // from "refdef\n\nnext-block" (paragraph closed).
                let after_terminator = ix + nl;
                refdef_terminated_by_blank = scan_blank_line(&bytes[after_terminator..]).is_some();
                ix = after_terminator;
            } else {
                self.finish_list(start_ix);
                // Refdefs share micromark's paragraph token: the def's
                // residual paragraph state lingers after the def's bytes
                // are consumed. When the next line opens a new block,
                // ordered-list markers with start != 1 are rejected.
                // Use the narrower `refdef_interrupted_paragraph` flag so
                // blank-after-marker bullets like `-\n` are still
                // recognized as list items.
                self.refdef_interrupted_paragraph = true;
                return ix;
            }
            if let Some(lazy_line_start) = self.scan_next_line_or_lazy_continuation(&bytes[ix..]) {
                line_start = lazy_line_start;
                start_ix = ix;
            } else {
                self.finish_list(start_ix);
                // Only carry the interrupt state forward when the refdef wasn't
                // already closed by a blank line. `[a]: u\n\n>*` should open a
                // real list inside the blockquote; `[a]: u\n>*` should not.
                if !refdef_terminated_by_blank {
                    self.refdef_interrupted_paragraph = true;
                }
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

        // `mdast-util-gfm-table` keeps the source cell count exactly — neither
        // truncated to the header width nor padded with empties. HAST padding
        // happens downstream in `mdast-util-to-hast`, and overflow cells are
        // discarded there too.
        let _ = row_cells;
        let _ = missing_empty_cells;

        // Extend the final cell's end to include any trailing `|` and
        // whitespace, matching remark's convention. With overflow preserved,
        // the final cell is the last parsed cell, not the header-column cell.
        let last_cell_ix = {
            let mut walker = self.tree[row_ix].child;
            let mut last = None;
            while let Some(c) = walker {
                last = Some(c);
                walker = self.tree[c].next;
            }
            last
        };
        if let Some(cell_ix) = last_cell_ix {
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
        let _ = final_cell_ix;

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
        // A line with ≥4 leading spaces (after container prefixes) is an
        // indented code block, which interrupts table continuation. Only
        // accept up to 3 spaces of leading indentation before the row.
        // MDX disables indented code blocks, so a table nested inside JSX
        // flow can have body rows at any indent — match the head's MDX
        // allowance (see scan_table_head paragraph_interrupt).
        if self.options.contains(Options::ENABLE_MDX) {
            line_start.scan_all_space();
        } else {
            let _ = line_start.scan_space_upto(3);
            if !line_start.is_at_eol() && line_start.scan_space(1) {
                return None;
            }
        }
        ix += line_start.bytes_scanned();
        if scan_paragraph_interrupt_no_table(
            &bytes[ix..],
            current_container,
            self.options.contains(Options::ENABLE_FOOTNOTES),
            self.options.contains(Options::ENABLE_DEFINITION_LIST),
            self.options.contains(Options::ENABLE_MDX),
            self.options.contains(Options::ENABLE_MATH_MULTI_DOLLAR),
            self.options.contains(Options::ENABLE_DIRECTIVE),
            &self.tree,
            tree_position,
        ) {
            return None;
        }

        let (ix, row_ix) = self.parse_table_row_inner(ix, row_cells, missing_empty_cells)?;
        Some((ix, row_ix))
    }

    /// Returns offset of line start after paragraph.
    /// Append a container directive's `[label]` as a `DirectiveLabel` child
    /// holding inline-tokenized content. The label paragraph spans the brackets
    /// (`[` … `]`); its children come from the normal inline pass run over the
    /// inner content, so emphasis/strong/links/code resolve like anywhere else.
    /// Must be called with the directive itself on the spine.
    fn append_container_directive_label(&mut self, label_start: usize, label_end: usize) {
        let bracket_offset = label_start.saturating_sub(1);
        let bracket_end = label_end + 1;
        self.tree.append(Item {
            start: bracket_offset,
            end: bracket_end,
            body: ItemBody::DirectiveLabel,
        });
        self.tree.push();
        if label_start < label_end {
            self.parse_line(label_start, Some(label_end), TableParseMode::Disabled);
        }
        self.tree.pop();
    }

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
                    // MDX-only suppression: if the previous line opened an
                    // inline JSX tag whose body spans into this line, the
                    // tag is still open and the line's would-be interrupt
                    // is actually part of the JSX (typically the closing
                    // `>`). Don't break the paragraph here — let the inline
                    // resolver pick up the JSX.
                    if !(self.options.contains(Options::ENABLE_MDX)
                        && prev_line_has_open_inline_jsx(bytes, ix))
                    {
                        if let Some(pos) = trailing_backslash_pos {
                            self.tree.append_text(pos, pos + 1, false);
                        }
                        self.list_interrupted_paragraph = scan_listitem(suffix).is_some()
                            || scan_blockquote_start(suffix).is_some();
                        // Type-7 HTML block on a lazy-continuation line is
                        // a special case in remark/micromark: the new HTML
                        // block opens INSIDE the still-open container (the
                        // line is fed through the container's child flow
                        // parser, where type-7 is allowed). Open it inline
                        // for BlockQuote and ListItem parents so the HTML
                        // becomes a sibling of the open paragraph instead
                        // of popping the container.
                        //
                        // Exception: if the html line is at EOF without a
                        // trailing newline, micromark's lazy handling treats
                        // it as an incomplete line and the html ends up at
                        // root (e.g. `- a\n<a>` → `[list, html]`, vs
                        // `- a\n<a>\n` → `[list[item[para,html]]]`).
                        if !self.options.contains(Options::ENABLE_MDX)
                            && !current_container
                            && suffix.starts_with(b"<")
                            && scan_html_type_7(suffix).is_some()
                            && !starts_html_block_type_6(&suffix[1..])
                            && get_html_end_tag(&suffix[1..]).is_none()
                        {
                            // Find end of the html line. If there's no
                            // newline anywhere in `suffix`, the line is at
                            // EOF without a trailing newline — fall through
                            // to the normal break path so containers pop.
                            let line_terminated = suffix.iter().any(|&b| b == b'\n' || b == b'\r');
                            // `tree_position` is the count of spine items
                            // that matched container prefixes. The container
                            // we're STILL inside is at spine index
                            // `tree_position` (the first one that did NOT
                            // match), since lazy continuation keeps the
                            // unmatched container open.
                            let parent_is_container =
                                self.tree.walk_spine().nth(tree_position).is_some_and(|ix| {
                                    matches!(
                                        self.tree[*ix].item.body,
                                        ItemBody::BlockQuote(..) | ItemBody::ListItem(..)
                                    )
                                });
                            if parent_is_container && line_terminated {
                                self.pop(ix);
                                return self.parse_html_block_type_6_or_7(
                                    ix_new,
                                    line_start.remaining_space(),
                                    0,
                                );
                            }
                        }
                        break;
                    }
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

        // remark/micromark quirk: when a setext heading directly follows a
        // run of link reference definitions (no blank line separating
        // any of them), the heading's position extends back to the start
        // of the FIRST definition in that adjacent run. Micromark opens
        // one paragraph token spanning the whole chain; each definition
        // splits off as its own node but the heading inherits the
        // chain's original start.
        let bytes = self.text.as_bytes();
        let is_adjacent = |prev_end: usize, next_start: usize| -> bool {
            if next_start <= prev_end {
                return false;
            }
            // Allow exactly one newline (the def's line terminator)
            // plus any amount of leading whitespace on the next line.
            // Indented-code can't interrupt the def's still-open
            // paragraph, so even 4+ space indents are valid paragraph
            // continuations and qualify for chain-back.
            let mut newlines = 0;
            for &b in &bytes[prev_end..next_start] {
                if b == b'\n' {
                    if newlines > 0 {
                        return false;
                    }
                    newlines += 1;
                } else if b == b'\r' {
                    if newlines > 0 {
                        return false;
                    }
                } else if b == b' ' || b == b'\t' {
                    if newlines == 0 {
                        return false;
                    }
                    // OK: whitespace after the line break is the
                    // next line's leading indent.
                } else {
                    return false;
                }
            }
            newlines == 1
        };
        let original_start = self.tree[node_ix].item.start;
        // Don't chain back when the heading's *first* content line is
        // itself shaped like a setext underline (≤3 leading spaces, then
        // only `-` or `=`, then optional trailing whitespace). Micromark
        // tries to interpret such a line as an underline for the def's
        // residual paragraph, which fails and resets the paragraph
        // token — so the heading no longer inherits the def's start.
        // Other content-line shapes (plain text, `*`, `+`, …) leave the
        // paragraph token intact and the chain-back applies.
        let first_line_end = bytes[original_start..ix]
            .iter()
            .position(|&b| b == b'\n')
            .map(|p| original_start + p)
            .unwrap_or(ix);
        let first_line = &bytes[original_start..first_line_end];
        let is_setext_underline_shape = {
            let mut p = 0;
            while p < first_line.len() && first_line[p] == b' ' && p < 3 {
                p += 1;
            }
            if p < first_line.len() && (first_line[p] == b'-' || first_line[p] == b'=') {
                let c = first_line[p];
                let mut q = p + 1;
                while q < first_line.len() && first_line[q] == c {
                    q += 1;
                }
                while q < first_line.len() && (first_line[q] == b' ' || first_line[q] == b'\t') {
                    q += 1;
                }
                q == first_line.len()
            } else {
                false
            }
        };
        if !is_setext_underline_shape {
            let mut cur_start = original_start;
            for def in self.allocs.refdefs_all.iter().rev() {
                let def_end = def.1.span.end;
                let def_start = def.1.span.start;
                if is_adjacent(def_end, cur_start) {
                    cur_start = def_start;
                } else {
                    break;
                }
            }
            if cur_start < original_start {
                self.tree[node_ix].item.start = cur_start;
            }
        }

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
        // End offset of the most-recently-emitted non-text item whose
        // bytes can contain a `\` that is NOT a text-context backslash
        // (e.g. an inline GFM autolink Link whose URL ended in `\`).
        // Used by the `\n` hardbreak check to skip those `\`s.
        let mut last_inline_emission_end: usize = start;

        let (final_ix, brk) = iterate_special_bytes(self.lookup_table, bytes, start, |ix, byte| {
            match byte {
                b'\n' | b'\r' => {
                    if let TableParseMode::Active = mode {
                        return LoopInstruction::BreakAtWith(ix, None);
                    }

                    let mut i = ix;
                    let eol_bytes = scan_eol(&bytes[ix..]).unwrap();

                    let end_ix = ix + eol_bytes;
                    // CommonMark hardbreak: an odd number of trailing source
                    // `\` chars before `\n`. Bytes inside an inline-emitted
                    // Link (e.g. GFM literal autolink consumed a `\` as URL
                    // content) are NOT text-context backslashes — stop the
                    // scan at `last_inline_emission_end` so those don't
                    // count.
                    let trailing_backslashes = {
                        let mut p = ix;
                        while p > last_inline_emission_end && bytes[p - 1] == b'\\' {
                            p -= 1;
                        }
                        ix - p
                    };

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
                            // Allow up to 3 spaces of leading indent: a line
                            // like ` - …` is a valid bullet (and lists win
                            // over table delimiter recognition).
                            let leading_spaces =
                                delim.iter().take(3).take_while(|&&b| b == b' ').count();
                            let delim_is_list_item =
                                scan_listitem(&delim[leading_spaces..]).is_some();
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
                    } else if bytes[ix + 1] == b'$' && self.options.has_math() {
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
                    // GFM precedence: at `_` (an email-local atext char),
                    // try the email-literal construct first. micromark's
                    // text registry binds `_` to `emailAutolink` and walks
                    // forward through atext to find `@`. If a valid email
                    // tokenizes here, it wins over the attentionSequence —
                    // skipping the MaybeEmphasis emission keeps `_-_@…`
                    // from forming an emphasis pair that hides the email.
                    if c == b'_' && self.options.contains(Options::ENABLE_GFM) {
                        // Run the cheap structural scan first — most `_`
                        // in prose can't reach an `@` through atext chars
                        // and we want to bail before paying for the
                        // paragraph-scan predicates.
                        let paragraph_floor = self
                            .tree
                            .peek_up()
                            .map(|nix| self.tree[nix].item.start)
                            .unwrap_or(start);
                        if let Some((email_start, email_end, full_url)) =
                            scan_email_forward_from_atext(bytes, ix, begin_text, paragraph_floor)
                        {
                            if !has_unbalanced_bracket_from(bytes, paragraph_floor, ix)
                                && !is_inside_code_span(bytes, ix)
                                && !is_inside_link_destination(bytes, ix)
                            {
                                let link_ix = self.allocs.allocate_link(
                                    LinkType::Email,
                                    full_url
                                        .strip_prefix("mailto:")
                                        .map(str::to_owned)
                                        .unwrap_or(full_url)
                                        .into(),
                                    "".into(),
                                    "".into(),
                                );
                                self.tree
                                    .append_text(begin_text, email_start, backslash_escaped);
                                backslash_escaped = false;
                                let link_node_ix = self.tree.append(Item {
                                    start: email_start,
                                    end: email_end,
                                    body: ItemBody::Link(link_ix),
                                });
                                let text_child = self.tree.create_node(Item {
                                    start: email_start,
                                    end: email_end,
                                    body: ItemBody::Text {
                                        backslash_escaped: false,
                                    },
                                });
                                self.tree[link_node_ix].child = Some(text_child);
                                begin_text = email_end;
                                last_inline_emission_end = email_end;
                                let skip = email_end.saturating_sub(ix + 1);
                                return LoopInstruction::ContinueAndSkip(skip);
                            }
                        }
                    }
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

                    // `backslash_escaped` applies to the `$` itself only when
                    // the escape sits directly before it (`\$`, pending text
                    // run empty). For `\\$` or `\X$` the escape is consumed by
                    // the earlier char and must not bleed into the delimiter.
                    let dollar_escaped = backslash_escaped && begin_text == ix;
                    self.tree.append_text(begin_text, ix, backslash_escaped);
                    self.tree.append(Item {
                        start: ix,
                        end: ix + 1,
                        body: ItemBody::MaybeMath(dollar_escaped, brace_context),
                    });
                    begin_text = ix + 1;
                    backslash_escaped = false;
                    LoopInstruction::ContinueAndSkip(0)
                }
                #[cfg(feature = "mdx")]
                b'{' if self.options.contains(Options::ENABLE_MDX) => {
                    // If `{` sits inside a pair of matching backtick runs on
                    // the current line, it's part of a code span's text —
                    // code spans take priority over MDX expressions in
                    // remark. Skip inline-expression detection so the `{` is
                    // consumed as literal text (the enclosing code span will
                    // pick it up when backtick pairing resolves).
                    //
                    // Same treatment for `{` inside a CommonMark link URL
                    // `[...](...)`: mdx-js does not evaluate expressions in
                    // URLs (`[a]({x})` round-trips with URL "{x}", literal),
                    // so treat the `{` as plain text and let the link
                    // resolver claim the bytes. This also avoids a hard
                    // parse error on unmatched `{` like `[a]({)`.
                    if is_inside_code_span(bytes, ix)
                        || is_inside_link_url_parens(bytes, ix)
                        || is_inside_open_inline_jsx_tag(bytes, ix)
                    {
                        LoopInstruction::ContinueAndSkip(0)
                    } else {
                        // MDX inline expression: try to scan balanced braces.
                        // Lazy-paragraph continuation rules differ between
                        // text- and flow-position `{`. mdx-js's text
                        // tokenizer (`{` after content on a paragraph line)
                        // sets `allowLazy: true`, so body chars on a lazy
                        // line are kept. Its flow tokenizer (`{` first on a
                        // line in a container) sets `allowLazy: false` and
                        // errors. The block-level pass already tried flow;
                        // fall-through here means the flow scan failed, but
                        // we still need to reproduce its strict-lazy
                        // behavior for `{` at line start.
                        let scan_result = if self.tree.spine_len() > 0 {
                            let check = self.make_container_line_check();
                            let allow_lazy_body = !is_at_paragraph_line_start(bytes, ix);
                            scan_mdx_inline_expression_in_container(
                                &bytes[ix..],
                                &check,
                                allow_lazy_body,
                            )
                        } else {
                            scan_mdx_inline_expression(&bytes[ix..])
                        };
                        if let Some((content_start, content_end, total_len)) = scan_result {
                            self.tree.append_text(begin_text, ix, backslash_escaped);
                            backslash_escaped = false;
                            // Strip container prefixes (e.g. blockquote `>`)
                            // from continuation lines and apply the 2-col
                            // indent dedent. Combined in one walk so the
                            // tab-stop math sees the correct per-line
                            // starting column (lazy lines start at col 0;
                            // strict lines start at the post-prefix column).
                            let (normalized, offset_map) =
                                self.inline_expression_value(ix + content_start, ix + content_end);
                            // Validate the expression body as JS via oxc.
                            // Without this, `{h<}` etc. silently produce a
                            // phantom mdxTextExpression and only error at
                            // JS emit. Allocator is reused across calls.
                            if let Some((err_offset, detail)) =
                                crate::mdx::try_parse_expression_body(
                                    &normalized,
                                    &mut self.mdx_expr_allocator,
                                )
                            {
                                // For single-line bodies the map is empty and
                                // the normalized text is a verbatim slice, so a
                                // direct offset is exact; multi-line bodies
                                // resolve through the map.
                                let source_offset =
                                    satteri_arena::mdx_types::Location::relative_to_absolute(
                                        &offset_map,
                                        err_offset,
                                    )
                                    .unwrap_or(ix + content_start + err_offset);
                                self.mdx_errors.push((
                                    source_offset,
                                    format!("Could not parse expression with oxc: {detail}"),
                                ));
                            }
                            let cow_ix = self.allocs.allocate_cow(normalized.into());
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
                    let count = 1 + scan_ch_repeat(&bytes[(ix + 1)..], b'`');
                    // Only suppress as text when this backtick would land
                    // inside a literalAutolink URL *and* couldn't possibly
                    // pair with an earlier backtick of the same length to
                    // form a code span (which would have fired before the
                    // URL construct in micromark's text-construct order).
                    //
                    // When an unbalanced `[` precedes the URL, micromark's
                    // `previousUnbalanced` disables the autolink construct,
                    // so a forward-matching backtick run also wins. Without
                    // the bracket, the URL fires first and claims interior
                    // backticks even when they could pair.
                    let suppressed = self.options.contains(Options::ENABLE_GFM)
                        && is_inside_gfm_autolink_url(bytes, ix)
                        && !has_earlier_backtick_run(bytes, ix, count)
                        && !(has_unbalanced_bracket_in_paragraph(bytes, ix)
                            && has_later_backtick_run(bytes, ix + count, count));
                    if suppressed {
                        LoopInstruction::ContinueAndSkip(count - 1)
                    } else {
                        self.tree.append_text(begin_text, ix, backslash_escaped);
                        backslash_escaped = false;
                        self.tree.append(Item {
                            start: ix,
                            end: ix + count,
                            body: ItemBody::MaybeCode(count, false),
                        });
                        begin_text = ix + count;
                        LoopInstruction::ContinueAndSkip(count - 1)
                    }
                }
                b'<' if self.options.contains(Options::ENABLE_MDX)
                    || bytes.get(ix + 1) != Some(&b'\\') =>
                {
                    // In MDX mode `<\…` is not a CommonMark backslash escape;
                    // the MaybeHtml resolver below validates it (and rejects
                    // `<\>` as an invalid JSX tag start).
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
                        let label_start = dir_data.label_start;
                        let label_end = dir_data.label_end;
                        let dir_ix = self.allocs.allocate_directive(dir_data);
                        let consumed = end_pos - ix;
                        self.tree.append(Item {
                            start: ix,
                            end: end_pos,
                            body: ItemBody::TextDirective(dir_ix),
                        });
                        // Tokenize the label inline as the directive's children,
                        // re-entering the inline scanner over the `[…]` span.
                        if label_start < label_end {
                            self.tree.push();
                            self.parse_line(label_start, Some(label_end), TableParseMode::Disabled);
                            self.tree.pop();
                        }
                        begin_text = end_pos;
                        LoopInstruction::ContinueAndSkip(consumed - 1)
                    } else {
                        LoopInstruction::ContinueAndSkip(0)
                    }
                }
                b'|' => {
                    // Only escaped when an odd number of backslashes precedes
                    // the pipe. `\|` → escaped; `\\|` → literal `\` then
                    // separator pipe.
                    let preceding_backslashes = scan_rev_while(&bytes[..ix], |b| b == b'\\');
                    if preceding_backslashes % 2 == 1 {
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
                b'h' | b'H' | b'w' | b'W' | b'@' if self.options.contains(Options::ENABLE_GFM) => {
                    // GFM literal autolink: protocol/www/email. Runs during
                    // inline tokenization so URL bytes are consumed before
                    // bracket/image resolution claims them. Mirrors
                    // micromark's text-context construct order — see
                    // node_modules/.../micromark-extension-gfm-autolink-literal.
                    //
                    // Only fires for the *construct* path. The mdast-util
                    // find-and-replace fallback (position-less) is left to
                    // gfm_autolink_literal_pass, which still runs as a
                    // backstop.
                    // For multi-line paragraphs, `start` is the current
                    // *line* start. The actual paragraph bounds come from
                    // the Paragraph item on the spine — use that as the
                    // `has_unbalanced_bracket` floor so a `[` on an
                    // earlier paragraph line is still seen.
                    let paragraph_floor = self
                        .tree
                        .peek_up()
                        .map(|ix| self.tree[ix].item.start)
                        .unwrap_or(start);
                    let result = try_emit_gfm_autolink(
                        bytes,
                        ix,
                        byte,
                        paragraph_floor,
                        begin_text,
                        backslash_escaped,
                        self.options,
                        &mut self.tree,
                        &mut self.allocs,
                    );
                    if let Some((new_begin_text, skip)) = result {
                        begin_text = new_begin_text;
                        last_inline_emission_end = new_begin_text;
                        backslash_escaped = false;
                        LoopInstruction::ContinueAndSkip(skip)
                    } else {
                        LoopInstruction::ContinueAndSkip(0)
                    }
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
    /// True when `line_start` (already advanced past container prefixes) sits
    /// on a *pure* closing `:::` fence for an enclosing container directive:
    /// colons only, indented at most 3 spaces, nothing but whitespace after,
    /// and at least as long as that directive's opening fence. HTML blocks
    /// consult this so they stop before the fence instead of swallowing it —
    /// remark-directive recognizes the close at the container level, before
    /// the block's continuation rule fires.
    fn at_closing_directive_fence(&self, line_start: &LineStart<'_>) -> bool {
        if !self.options.contains(Options::ENABLE_DIRECTIVE) {
            return false;
        }
        for &node_ix in self.tree.walk_spine().rev() {
            match self.tree[node_ix].item.body {
                ItemBody::ContainerDirective(length, ..) => {
                    // Keep walking outward on a miss: a shorter fence can still
                    // close an ancestor directive that opened with fewer colons.
                    let mut probe = line_start.clone();
                    let _ = probe.scan_space_upto(3);
                    if probe.scan_closing_container_extensions_fence(length) {
                        probe.scan_all_space();
                        if probe.is_at_eol() {
                            return true;
                        }
                    }
                }
                ItemBody::HtmlBlock(..) | ItemBody::List(..) | ItemBody::ListItem(..) => {}
                _ => break,
            }
        }
        false
    }

    fn parse_html_block_type_1_to_5(
        &mut self,
        start_ix: usize,
        html_end_tag: &str,
        mut remaining_space: usize,
        mut indent: usize,
    ) -> usize {
        // HTML block is a full block — clear lingering empty-list-marker
        // suppression from a prior block (see `parse_hrule` comment).
        self.list_interrupted_paragraph = false;
        // Tree-position of the just-appended HtmlBlock — patched below to
        // record whether the closer pattern was found (so the value gets its
        // trailing newline trimmed) or the block ran to EOF without one.
        let block_node = self.tree.append(Item {
            start: start_ix,
            end: 0, // set later
            body: ItemBody::HtmlBlock(false),
        });
        self.tree.push();

        let bytes = self.text.as_bytes();
        let mut ix = start_ix;
        let end_ix;
        // Two distinct close paths:
        //   * `closer_pattern_found`: the `html_end_tag` matched on a line —
        //     remark trims the trailing newline of the close line.
        //   * container exit / EOF: the block was terminated externally
        //     (parent list item ended, document ended). The trailing newline
        //     of the LAST html line is content per remark, so we don't trim.
        let mut closer_pattern_found = false;
        // Spine depth above the HTML block itself — if we drop below this,
        // a parent container has ended (e.g. list item closed by blank
        // line + outdent). The HTML block ends WITHOUT consuming the
        // boundary line (matches micromark, which has the parent close
        // before the HTML block's continuation rule fires).
        let parent_spine_len = self.tree.spine_len() - 1;
        // Whether any ancestor container is a blockquote. micromark treats
        // the line ending right before a blockquote-close (blank line +
        // outdent) as part of the blockquote's separator, not the HTML
        // block content — so `> <style\n\nfoo` emits `<style` (no trailing
        // `\n`). A bare list item keeps the `\n`. EOF or continuation-
        // marker close ALSO keeps the `\n` for both. The flag below is
        // only consulted on the blank-line exit path.
        let ancestor_has_blockquote = self
            .tree
            .walk_spine()
            .any(|&ix| matches!(self.tree[ix].item.body, ItemBody::BlockQuote(..)));
        // Block ended via two different external close paths:
        //   * lazy: a non-blank line breaks container indent (`* <!-- foo\nbar` —
        //     `bar` at col 0 doesn't satisfy list item indent). REF trims for
        //     ANY parent.
        //   * blank: a blank line ends the parent container. REF trims for
        //     blockquote but KEEPS for list item.
        let mut closed_via_blank = false;
        let mut closed_via_lazy = false;
        loop {
            // Before consuming the current line, peek for a container exit.
            // If we're inside a list/blockquote and the current line is
            // blank, look ahead to the next non-blank line to see if the
            // parent closes there. If yes, end the HTML block now (don't
            // append the blank line).
            if parent_spine_len > 0 && scan_blank_line(&bytes[ix..]).is_some() {
                let mut peek_ix = ix;
                while peek_ix < bytes.len() {
                    if let Some(adv) = scan_blank_line(&bytes[peek_ix..]) {
                        peek_ix += adv;
                    } else {
                        break;
                    }
                }
                if peek_ix == bytes.len() {
                    end_ix = ix;
                    closed_via_blank = true;
                    break;
                }
                let mut peek_line_start = LineStart::new(&bytes[peek_ix..]);
                let n_peek = scan_containers(&self.tree, &mut peek_line_start, self.options);
                if n_peek <= parent_spine_len {
                    end_ix = ix;
                    closed_via_blank = true;
                    break;
                }
            }

            let line_start_ix = ix;
            ix += scan_nextline(&bytes[ix..]);
            self.append_html_line(remaining_space.max(indent), line_start_ix, ix);

            let mut line_start = LineStart::new(&bytes[ix..]);
            let n_containers = scan_containers(&self.tree, &mut line_start, self.options);
            if n_containers < self.tree.spine_len() {
                end_ix = ix;
                if scan_blank_line(&bytes[ix..]).is_some() {
                    closed_via_blank = true;
                } else {
                    // When the outdent is caused by a NEW container marker
                    // (sibling list/bq) opening on this line, remark keeps
                    // the trailing newline of the html block. Only a real
                    // lazy line (paragraph/heading/etc.) trims it.
                    let next_line_ix = ix + line_start.bytes_scanned();
                    let rest = &bytes[next_line_ix..];
                    let opens_container =
                        scan_blockquote_start(rest).is_some() || scan_listitem(rest).is_some();
                    if !opens_container {
                        closed_via_lazy = true;
                    }
                }
                break;
            }

            if self.text[line_start_ix..ix].contains(html_end_tag) {
                end_ix = ix;
                closer_pattern_found = true;
                break;
            }

            let next_line_ix = ix + line_start.bytes_scanned();
            if next_line_ix == self.text.len() {
                end_ix = next_line_ix;
                break;
            }
            // A closing directive fence ends the block before swallowing it.
            if self.at_closing_directive_fence(&line_start) {
                end_ix = ix;
                break;
            }
            ix = next_line_ix;
            remaining_space = line_start.remaining_space();
            indent = 0;
        }
        // Trim trailing `\n` from the html value when:
        //   * the html_end_tag closed the block on its own line
        //   * a lazy line broke the parent container (any parent)
        //   * a blank line closed a blockquote-parented block
        // Don't trim when the block ended via EOF after a continuation
        // marker, or via blank inside a list item.
        let trim_trailing = closer_pattern_found
            || closed_via_lazy
            || (ancestor_has_blockquote && closed_via_blank);
        if trim_trailing {
            self.tree[block_node].item.body = ItemBody::HtmlBlock(true);
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
        // HTML block is a full block — clear lingering empty-list-marker
        // suppression from a prior block (see `parse_hrule` comment).
        self.list_interrupted_paragraph = false;
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
            // A closing directive fence ends the block before swallowing it.
            if self.at_closing_directive_fence(&line_start) {
                end_ix = ix;
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
        // Consume the lazy-after-blockquote-close marker. When set, this
        // code block is one-line only — micromark's `furtherStart` for
        // `codeIndented` rejects lazy lines, so an indented line that
        // follows the implicitly-closed blockquote can't extend across
        // subsequent (also-lazy) lines. Cleared after consumption so a
        // following code block (e.g. after a blank line) parses normally.
        let lazy_one_line = self.pending_lazy_blockquote_close;
        self.pending_lazy_blockquote_close = false;

        self.tree.append(Item {
            start: line_start_ix,
            end: 0, // will get set later
            body: ItemBody::IndentCodeBlock(lazy_one_line),
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

            if lazy_one_line {
                break;
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
            // Only treat the post-strip line as blank when nothing but EOL is
            // left. Lines whose whitespace survives the strip (e.g. `\t\t` →
            // `\t`) still belong to the code block; remark keeps them.
            self.last_line_blank = scan_eol(&bytes[ix..]).is_some();
        }

        // Trim trailing blank lines.
        if let Some(child) = last_nonblank_child {
            self.tree[child].next = None;
            self.tree[child].item.end = last_nonblank_ix;
        }
        self.pop(end_ix);
        // Set `interrupt`-equivalent state ONLY when this code block wasn't a
        // one-line lazy block opened immediately after a popped blockquote.
        // For `>\n\t9\n+` micromark's `interrupt` is false at `+` (the bq's
        // blank-line state propagates through the lazy code), so the empty
        // marker is allowed to open a list. For `\t9\n+` and `code\n\n2.b`,
        // the code wasn't lazy and the suppression should fire.
        if !lazy_one_line {
            self.list_interrupted_paragraph = true;
        }
        ix
    }

    fn parse_fenced_code_block(
        &mut self,
        start_ix: usize,
        indent: usize,
        fence_ch: u8,
        n_fence_char: usize,
    ) -> usize {
        // Fenced code is a full block — clear lingering empty-list-marker
        // suppression from a prior block (see `parse_hrule` comment).
        self.list_interrupted_paragraph = false;
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
            // EOF reached without a closing fence — pop and end the block.
            // arena_build.rs strips the final \n later; no extra trim here.
            if ix >= bytes.len() {
                self.pop(ix);
                return ix;
            }
            let mut line_start = LineStart::new(&bytes[ix..]);
            let n_containers = scan_containers(&self.tree, &mut line_start, self.options);
            if n_containers < self.tree.spine_len() {
                // Container outdent ends the code block. For a leaf-block
                // interrupt (paragraph/heading/fence-close), the blank line
                // immediately before the outdent acts as the container's
                // separator, so we strip one extra \n here. arena_build.rs
                // strips the last content line's \n afterward — together
                // that's 2 \n's stripped, matching `- ```\n  foo\n\noo` →
                // `foo`.
                //
                // BUT when the outdent is caused by a NEW container marker
                // on this line (sibling list-item, blockquote, etc.), remark
                // does NOT consume the preceding blank as a separator — the
                // marker itself is what terminates the previous item. So we
                // strip 0 extra here (arena_build still strips 1), keeping
                // the blank line(s) as code content.
                let next_line_ix = ix + line_start.bytes_scanned();
                let rest = &bytes[next_line_ix..];
                let line_starts_container =
                    scan_blockquote_start(rest).is_some() || scan_listitem(rest).is_some();
                let extra = if line_starts_container { 0 } else { 1 };
                if extra > 0 {
                    trim_trailing_newlines_from_code_block(&mut self.tree, bytes, extra);
                }
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
        // Math fence is a full block — clear lingering empty-list-marker
        // suppression from a prior block (see `parse_hrule` comment).
        self.list_interrupted_paragraph = false;
        let bytes = self.text.as_bytes();
        let mut meta_start = start_ix + n_fence_char;
        meta_start += scan_whitespace_no_nl(&bytes[meta_start..]);
        let mut ix = meta_start + scan_nextline(&bytes[meta_start..]);
        // Only strip the trailing newline; preserve any trailing spaces/tabs
        // in the meta string (remark keeps them verbatim).
        let meta_end = ix - scan_rev_while(&bytes[meta_start..ix], |c| c == b'\n' || c == b'\r');
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
        // mdast-util-math strips a leading AND trailing newline from the
        // accumulated content (matching micromark's `mathFlow`, which doesn't
        // emit the lineEnding past the last content chunk before a closing
        // fence or container exit). To replicate, peek a blank-within-container
        // line before appending it: if the NEXT line breaks the parent
        // container, the blank line's `\n` is given back to the surrounding
        // context and shouldn't enter the math value.
        loop {
            if ix >= bytes.len() {
                self.pop(ix);
                return ix;
            }
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
            let content_start = ix + line_start.bytes_scanned();
            let next_ix = content_start + scan_nextline(&bytes[content_start..]);
            // Blank-within-container line lookahead: if this line's content is
            // empty (just the `\n`) and the next line breaks the parent
            // container, micromark exits the math construct without emitting
            // the lineEnding for this blank line. Skip it here so the math
            // value matches remark.
            let line_is_blank = content_start + 1 == next_ix
                && matches!(bytes[content_start], b'\n' | b'\r')
                || content_start == next_ix;
            if line_is_blank && next_ix < bytes.len() {
                let mut peek_ls = LineStart::new(&bytes[next_ix..]);
                let peek_n = scan_containers(&self.tree, &mut peek_ls, self.options);
                if peek_n < self.tree.spine_len() {
                    self.pop(ix);
                    return ix;
                }
            }
            self.append_code_text(remaining_space, content_start, next_ix);
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
        // A thematic break is a block-terminator. The "list interrupted
        // paragraph" suppression from the previous block (e.g. indented
        // code) no longer applies — a new list after the rule can be
        // empty and still open.
        self.list_interrupted_paragraph = false;
        ix + hrule_size
    }

    /// Parse an ATX heading.
    ///
    /// Returns index of start of next line.
    fn parse_atx_heading(&mut self, start: usize, atx_level: HeadingLevel) -> usize {
        // The heading is a full block; any "interrupting paragraph"
        // suppression from a prior block (e.g. indented code) no longer
        // applies — a new empty list after the heading can still open.
        // Mirrors the equivalent clear in `parse_hrule`.
        self.list_interrupted_paragraph = false;
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
            // ATX headings can't span lines. In MDX mode bound the inline
            // scan to the current line so an unclosed expression body like
            // `# {1 +\n2}` errors at end-of-line (matching mdx-js) instead
            // of consuming across the newline to find a closing `}`.
            let line_end = if self.options.contains(Options::ENABLE_MDX) {
                Some(header_start + scan_nextline(&bytes[header_start..]))
            } else {
                None
            };
            let (line_ix, line_brk) = self.parse_line(ix, line_end, TableParseMode::Disabled);
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
        // GFM footnote labels can't be empty or contain whitespace.
        // `[^a b]` and `[^]` fall through to the regular refdef path.
        let (mut i, label) =
            scan_link_label_rest(&self.text[start + 2..], &|_| None, self.tree.is_in_table())?;
        // GFM/micromark's labelInside rejects whitespace CHARACTER-BY-CHARACTER.
        // Any space/tab/eol — including leading whitespace stripped by
        // scan_link_label_rest's trim — invalidates the footnote definition
        // (it falls through to a regular reference definition). Inspect the
        // raw source bytes between `[^` and `]`, not the trimmed `label`.
        let raw_label = &self.text.as_bytes()[start + 2..start + 2 + i.saturating_sub(1)];
        if raw_label.is_empty()
            || raw_label
                .iter()
                .any(|&b| b == b' ' || b == b'\t' || b == b'\r' || b == b'\n')
        {
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

        // remark folds trailing same-line whitespace after the URL into the
        // definition's source span even with no title. Extend the no-title
        // fallback to match.
        let span_end = i + scan_whitespace_no_nl(&bytes[i..]);

        // no title
        let mut backup = (
            span_end - start,
            LinkDef {
                dest,
                title: None,
                span: span_start..span_end,
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
                // remark folds trailing same-line whitespace after the
                // title into the definition's source span (matches the
                // behavior already applied above in the no-title path).
                let span_end = i + scan_whitespace_no_nl(&bytes[i..]);
                backup.0 = i - start;
                backup.1.span = span_start..span_end;
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
        #[cfg(feature = "mdx")]
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
            self.options.contains(Options::ENABLE_MATH_MULTI_DOLLAR),
            self.options.contains(Options::ENABLE_DIRECTIVE),
            &self.tree,
            tree_position,
        ) {
            return true;
        }
        // pulldown-cmark traditionally only interrupted paragraphs on "heavy"
        // table headers (lines starting with `|`). remark-gfm also lets a
        // "light" table interrupt: any header line followed by a valid
        // delimiter row, provided the header isn't a lazy-continuation line
        // (lazy lines can't open new blocks).
        //
        // ```markdown
        // This is a table
        // | a | b | c |
        // |---|---|---|
        // | d | e | f |
        //
        // Also a table
        //  a | b | c
        // ---|---|---
        // ```
        if !self.options.contains(Options::ENABLE_TABLES) {
            return false;
        }
        if !bytes.starts_with(b"|") && !current_container {
            return false;
        }

        // Cheap pre-filter: a delimiter row must contain at least one `-`.
        // Container paragraphs hit this path on every continuation line, so
        // skipping the pipe-counting loop / scan_table_head when the next
        // line obviously can't be a delimiter row matters for parse perf.
        // Use SIMD-backed `memchr2` rather than a `position` closure — this
        // path runs on every paragraph continuation line.
        let Some(eol_off) = memchr::memchr2(b'\n', b'\r', bytes) else {
            return false;
        };
        let next_line_ix = eol_off + scan_eol(&bytes[eol_off..]).unwrap();
        let next_line_end = memchr::memchr2(b'\n', b'\r', &bytes[next_line_ix..])
            .map(|p| next_line_ix + p)
            .unwrap_or(bytes.len());
        if memchr::memchr(b'-', &bytes[next_line_ix..next_line_end]).is_none() {
            return false;
        }

        // First line, count unescaped pipes. A run of consecutive backslashes
        // toggles the escape state: `\|` escapes the pipe, `\\|` is a literal
        // `\` followed by an unescaped separator pipe.
        let mut pipes = 0;
        let mut bsesc = false;
        let mut last_pipe_ix = 0;
        for (i, &byte) in bytes[..eol_off].iter().enumerate() {
            match byte {
                b'\\' => {
                    bsesc = !bsesc;
                    continue;
                }
                b'|' if !bsesc => {
                    pipes += 1;
                    last_pipe_ix = i;
                }
                _ => {}
            }
            bsesc = false;
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
/// Whether an MDX JSX flow element or `{…}` flow expression at the line start
/// interrupts a paragraph. In the lite (non-mdx) build this is always false —
/// the gated `mdx::*` scanners don't exist there.
#[cfg(feature = "mdx")]
fn mdx_block_interrupts(bytes: &[u8], mdx: bool) -> bool {
    (mdx && bytes.starts_with(b"<") && scan_mdx_jsx_block(bytes, None).is_some())
        || (mdx && bytes.starts_with(b"{") && scan_mdx_expression_block(bytes, None).is_some())
}

#[cfg(not(feature = "mdx"))]
fn mdx_block_interrupts(_bytes: &[u8], _mdx: bool) -> bool {
    false
}

#[allow(clippy::too_many_arguments)]
fn scan_paragraph_interrupt_no_table(
    bytes: &[u8],
    current_container: bool,
    has_footnote: bool,
    definition_list: bool,
    mdx: bool,
    math: bool,
    directive: bool,
    tree: &Tree<Item>,
    tree_position: usize,
) -> bool {
    scan_eol(bytes).is_some()
        || scan_hrule(bytes).is_ok()
        || scan_atx_heading(bytes).is_some()
        || scan_code_fence(bytes).is_some()
        || (math && scan_math_fence(bytes).is_some())
        || (directive && scan_interrupting_container_extensions_fence(bytes))
        || scan_blockquote_start(bytes).is_some()
        || scan_listitem(bytes).is_some_and(|(ix, delim, _index, _)| {
            ! current_container ||
            tree.is_in_table() ||
            // We don't allow interruption by either empty lists or numbered
            // lists whose marker isn't *textually* the single character `1`
            // (so `01.`, `001.`, `10.` all stay paragraph text — micromark
            // matches the literal marker, not the parsed integer value).
            (delim == b'*' || delim == b'-' || delim == b'+'
                || (bytes.first() == Some(&b'1')
                    && matches!(bytes.get(1), Some(b'.') | Some(b')'))))
                && (scan_blank_line(&bytes[ix..]).is_none())
        })
        // HTML types 1–6 interrupt paragraphs. MDX disables HTML blocks
        // entirely (everything `<…>` is JSX), so the type-1/6 gate
        // shouldn't fire — a line like `</div>after` is just paragraph
        // text in MDX, not a new HTML block.
        || (!mdx
            && bytes.starts_with(b"<")
            && (get_html_end_tag(&bytes[1..]).is_some() || starts_html_block_type_6(&bytes[1..])))
        // Type-7 HTML blocks normally can't interrupt a paragraph, but
        // micromark/remark close the paragraph when a type-7 start appears
        // on a *lazy-continuation* line of a container (typically a
        // blockquote without `>` on the next line). Mirror that quirk:
        // accept type-7 only when we're not inside the current container,
        // so plain-paragraph cases (`oo\n<a href="bar">`) keep the inline
        // HTML behavior the spec requires. MDX disables HTML blocks
        // entirely (everything `<...>` is JSX), so don't trigger the
        // quirk in MDX mode either.
        || (!mdx
            && !current_container
            && bytes.starts_with(b"<")
            && scan_html_type_7(bytes).is_some())
        // MDX JSX flow elements and `{ ... }` flow expressions also interrupt
        // paragraphs. Behind a cfg-switched helper so the gated `mdx::*` scan
        // functions aren't named in the lite build (where it returns false).
        || mdx_block_interrupts(bytes, mdx)
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

/// Return `true` if `pos` sits between two backtick runs of matching length
/// within the current paragraph. Used to give code spans priority over MDX
/// inline expressions — `` `{foo}` `` is a code span containing the text
/// `{foo}`, not an inline expression. Code spans can cross newlines, so we
/// scan the full paragraph (bounded by blank lines AND by changes in the
/// leading container prefix — `paragraph + blockquote` are different blocks
/// and their backticks must not pair).
/// Same shape as `is_inside_code_span` but for math `$…$` runs. Used to
/// keep GFM literal-autolink emission from cutting across a math span
/// boundary (e.g. `$<https://x$` — the `$` pair owns those bytes).
fn is_inside_math_span(bytes: &[u8], pos: usize) -> bool {
    let (para_start, para_end) = scope_for_inline(bytes, pos);
    // Fast reject: need at least one `$` on each side of `pos` to form a
    // span. Skips the per-byte walk when the paragraph has no `$`.
    if pos <= para_start
        || pos >= para_end
        || memchr::memchr(b'$', &bytes[para_start..pos]).is_none()
        || memchr::memchr(b'$', &bytes[pos..para_end]).is_none()
    {
        return false;
    }
    let mut runs: Vec<(usize, usize)> = Vec::with_capacity(4);
    let mut i = para_start;
    while i < para_end {
        if bytes[i] == b'\\' && i + 1 < para_end {
            i += 2;
            continue;
        }
        if bytes[i] == b'$' {
            let start = i;
            while i < para_end && bytes[i] == b'$' {
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

fn is_inside_code_span(bytes: &[u8], pos: usize) -> bool {
    let (para_start, para_end) = scope_for_inline(bytes, pos);
    // Fast reject: a code span needs at least two backtick runs around
    // `pos`. If there's no backtick on either side, we're not in one.
    // memchr is SIMD-accelerated and a lot cheaper than the bracket-aware
    // walk below.
    if pos <= para_start
        || pos >= para_end
        || memchr::memchr(b'`', &bytes[para_start..pos]).is_none()
        || memchr::memchr(b'`', &bytes[pos..para_end]).is_none()
    {
        return false;
    }
    // Collect backtick runs in the paragraph, skipping backslash-escaped
    // ones. Each run records its `[`-bracket nesting depth at the opening
    // byte so that backticks inside a `[label]` (e.g. a directive label
    // or a link's body) only pair with backticks at the same depth — not
    // with backticks in the surrounding paragraph text.
    let mut runs: Vec<(usize, usize, i32)> = Vec::with_capacity(8);
    let mut bracket_depth: i32 = 0;
    let mut i = para_start;
    while i < para_end {
        if bytes[i] == b'\\' && i + 1 < para_end {
            i += 2;
            continue;
        }
        match bytes[i] {
            b'[' => {
                bracket_depth += 1;
                i += 1;
            }
            b']' if bracket_depth > 0 => {
                bracket_depth -= 1;
                i += 1;
            }
            b'`' => {
                let start = i;
                while i < para_end && bytes[i] == b'`' {
                    i += 1;
                }
                runs.push((start, i - start, bracket_depth));
            }
            _ => i += 1,
        }
    }
    if runs.len() < 2 {
        return false;
    }
    // First-fit pair the runs: each open matches the next run of the same
    // length AND same bracket depth.
    let mut paired = vec![false; runs.len()];
    for a in 0..runs.len() {
        if paired[a] {
            continue;
        }
        for b in (a + 1)..runs.len() {
            if paired[b] {
                continue;
            }
            if runs[b].1 == runs[a].1 && runs[b].2 == runs[a].2 {
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

/// Bound the inline-content scope around `pos` for code-span pairing.
/// Walks backward and forward line-by-line, stopping at blank lines AND at
/// any line whose container prefix differs from the current line's. This
/// prevents a backtick in a root paragraph from pairing with one inside a
/// blockquote (or vice versa) and incorrectly suppressing an MDX expression
/// scan in between.
fn scope_for_inline(bytes: &[u8], pos: usize) -> (usize, usize) {
    let cur_line_start = {
        let mut j = pos;
        while j > 0 && !matches!(bytes[j - 1], b'\n' | b'\r') {
            j -= 1;
        }
        j
    };
    let cur_prefix = leading_container_prefix(&bytes[cur_line_start..]);
    let start = scope_start_with_prefix(bytes, cur_line_start, cur_prefix);
    let end = scope_end_with_prefix(bytes, cur_line_start, cur_prefix);
    (start, end)
}

/// Count the blockquote (`>`) depth at the start of a line. Two lines belong
/// to the same inline-content scope only if their blockquote depths match.
/// Plain leading whitespace on a continuation line within a paragraph is not
/// a container boundary — only `>` markers are.
fn leading_container_prefix(line: &[u8]) -> usize {
    let mut depth = 0;
    let mut i = 0;
    loop {
        // Up to 3 spaces of indentation before each `>`.
        let mut spaces = 0;
        while i < line.len() && line[i] == b' ' && spaces < 3 {
            i += 1;
            spaces += 1;
        }
        if i < line.len() && line[i] == b'>' {
            depth += 1;
            i += 1;
            // Optional single space/tab after `>`.
            if i < line.len() && (line[i] == b' ' || line[i] == b'\t') {
                i += 1;
            }
        } else {
            break;
        }
    }
    depth
}

fn scope_start_with_prefix(bytes: &[u8], cur_line_start: usize, prefix: usize) -> usize {
    let mut line_start = cur_line_start;
    while line_start > 0 {
        let mut prev_end = line_start - 1;
        if prev_end > 0 && bytes[prev_end] == b'\n' && bytes[prev_end - 1] == b'\r' {
            prev_end -= 1;
        }
        let mut prev_start = prev_end;
        while prev_start > 0 && !matches!(bytes[prev_start - 1], b'\n' | b'\r') {
            prev_start -= 1;
        }
        let prev_line = &bytes[prev_start..prev_end];
        if prev_line.iter().all(|&b| b == b' ' || b == b'\t') {
            return line_start;
        }
        if leading_container_prefix(prev_line) != prefix {
            return line_start;
        }
        line_start = prev_start;
    }
    line_start
}

fn scope_end_with_prefix(bytes: &[u8], cur_line_start: usize, prefix: usize) -> usize {
    let mut i = cur_line_start;
    while i < bytes.len() && !matches!(bytes[i], b'\n' | b'\r') {
        i += 1;
    }
    while i < bytes.len() {
        let after_eol = if bytes[i] == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
            i + 2
        } else {
            i + 1
        };
        let mut next_eol = after_eol;
        while next_eol < bytes.len() && !matches!(bytes[next_eol], b'\n' | b'\r') {
            next_eol += 1;
        }
        let next_line = &bytes[after_eol..next_eol];
        if next_line.iter().all(|&b| b == b' ' || b == b'\t') {
            return i;
        }
        if leading_container_prefix(next_line) != prefix {
            return i;
        }
        i = next_eol;
    }
    i
}

/// True if the byte at `pos` sits inside a CommonMark link URL `[...](...)`
/// — i.e. there is an unmatched `(` between `pos` and the start of the
/// current line, immediately preceded by `]`, that `]` is balanced by an
/// earlier `[` on the same line, AND the `(` has a matching `)` ahead on
/// the same line. mdx-js does not evaluate expressions in link URLs (URL
/// `{x}` is literal text, URL-encoded at render time), so we treat `{`
/// here as plain text rather than an expression start. This both avoids a
/// hard error on unmatched `{` inside a valid URL (e.g. `[a]({)`) and
/// removes a brittle dance where the link resolver previously had to
/// reabsorb a stray `MdxTextExpression` token.
///
/// When the `(` is unmatched (no closing `)` on the line), the link won't
/// form. mdx-js then falls back to expression scanning and errors on the
/// dangling `{`. We mirror that by returning false in this case so the
/// caller can run the expression scanner.
///
/// CommonMark forbids URLs from spanning a blank line, so per-line scans
/// are sufficient in both directions.
#[cfg(feature = "mdx")]
fn is_inside_link_url_parens(bytes: &[u8], pos: usize) -> bool {
    // Fast reject: walkback returns false on first newline, so a `(` past
    // the current line can't reach pos. Skip the walk when this line has
    // no `(` before pos.
    let line_start = memchr::memrchr2(b'\n', b'\r', &bytes[..pos])
        .map(|i| i + 1)
        .unwrap_or(0);
    if memchr::memchr(b'(', &bytes[line_start..pos]).is_none() {
        return false;
    }
    let mut paren_depth: i32 = 0;
    let mut i = pos;
    while i > 0 {
        i -= 1;
        match bytes[i] {
            b'\n' | b'\r' => return false,
            b')' => paren_depth += 1,
            b'(' => {
                if paren_depth == 0 {
                    // Unmatched `(` to our left. If it's immediately
                    // preceded by `]`, this is the link-tail open. Verify
                    // and return.
                    if i > 0 && bytes[i - 1] == b']' {
                        let mut j = i - 1; // position of `]`
                        let mut bracket_depth: i32 = 1;
                        while j > 0 {
                            j -= 1;
                            match bytes[j] {
                                b'\n' | b'\r' => return false,
                                b']' => bracket_depth += 1,
                                b'[' => {
                                    bracket_depth -= 1;
                                    if bracket_depth == 0 {
                                        // Bracket pair matches. Confirm the
                                        // `(` closes properly and any quoted
                                        // title is closed.
                                        return link_tail_well_formed(bytes, i, pos);
                                    }
                                }
                                _ => {}
                            }
                        }
                        return false;
                    }
                    // Otherwise: this `(` may be a paren-delimited title
                    // open inside an outer `](...)` link tail (e.g.
                    // `[a](/u (ti{w))`). Continue walking — we'll find the
                    // outer `](` if it exists. Leave `paren_depth` at 0
                    // (effectively treating this `(` as already matched
                    // by a `)` past `pos`).
                } else {
                    paren_depth -= 1;
                }
            }
            _ => {}
        }
    }
    false
}

/// True if the link-URL parens at `lparen` close on the same line, the
/// URL/title structure is well-formed, AND `pos` sits inside the URL
/// portion (not the title or invalid whitespace gap). Used as a tighter
/// check for `is_inside_link_url_parens`: a malformed tail (e.g. plain
/// URL followed by non-title bytes) means the link won't form, so any
/// `{` between them should be treated as an expression start.
#[cfg(feature = "mdx")]
fn link_tail_well_formed(bytes: &[u8], lparen: usize, pos: usize) -> bool {
    let mut depth: i32 = 1;
    let mut k = lparen + 1;
    let mut rparen = None;
    while k < bytes.len() {
        match bytes[k] {
            b'\n' | b'\r' => return false,
            b'\\' if k + 1 < bytes.len() => {
                k += 2;
                continue;
            }
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    rparen = Some(k);
                    break;
                }
            }
            _ => {}
        }
        k += 1;
    }
    let rparen = match rparen {
        Some(r) => r,
        None => return false,
    };
    // The CommonMark link tail is `(URL[ TITLE])`. Verify the structure
    // matches: URL is either `<...>` or plain (no whitespace, no control
    // chars). After URL, an optional whitespace-delimited title must be
    // a properly closed `"..."`, `'...'`, or `(...)`. Anything else
    // between URL end and `)` means the link fails to form — in that case
    // `pos` should NOT be treated as inside a link URL.
    {
        let mut p = lparen + 1;
        while p < rparen && (bytes[p] == b' ' || bytes[p] == b'\t') {
            p += 1;
        }
        let url_end;
        if p < rparen && bytes[p] == b'<' {
            p += 1;
            let mut found = false;
            while p < rparen {
                match bytes[p] {
                    b'\\' if p + 1 < rparen => p += 2,
                    b'>' => {
                        found = true;
                        p += 1;
                        break;
                    }
                    b'<' | b'\n' | b'\r' => break,
                    _ => p += 1,
                }
            }
            if !found {
                return false;
            }
            url_end = p;
        } else {
            let mut depth_url: i32 = 0;
            while p < rparen {
                let b = bytes[p];
                if b == b'\\' && p + 1 < rparen {
                    p += 2;
                    continue;
                }
                if b == b'(' {
                    depth_url += 1;
                    p += 1;
                } else if b == b')' {
                    if depth_url == 0 {
                        break;
                    }
                    depth_url -= 1;
                    p += 1;
                } else if matches!(b, b' ' | b'\t') {
                    break;
                } else if b < 0x20 || b == 0x7f {
                    return false;
                } else {
                    p += 1;
                }
            }
            url_end = p;
        }
        // pos sits AFTER the URL: the tail is only well-formed if the
        // post-URL bytes parse as `[ws]+(title)?[ws]*)`.
        if pos >= url_end {
            let mut q = url_end;
            while q < rparen && (bytes[q] == b' ' || bytes[q] == b'\t') {
                q += 1;
            }
            if q == rparen {
                // No title — pos sits in trailing whitespace before `)`.
                // The link is well-formed; treating `pos` as inside the
                // URL portion is conservative but matches the original
                // intent.
            } else {
                let title_open = bytes[q];
                let title_close = match title_open {
                    b'"' => b'"',
                    b'\'' => b'\'',
                    b'(' => b')',
                    _ => return false,
                };
                let mut r = q + 1;
                let mut closed = false;
                while r < rparen {
                    if bytes[r] == b'\\' && r + 1 < rparen {
                        r += 2;
                        continue;
                    }
                    if title_open == b'(' && bytes[r] == b'(' {
                        // Nested `(` inside paren-title invalidates the
                        // title per CommonMark.
                        return false;
                    }
                    if bytes[r] == title_close {
                        closed = true;
                        break;
                    }
                    r += 1;
                }
                if !closed {
                    return false;
                }
                let mut s = r + 1;
                while s < rparen && (bytes[s] == b' ' || bytes[s] == b'\t') {
                    s += 1;
                }
                if s != rparen {
                    return false;
                }
            }
        }
    }
    // Walk the bytes between `(` and `)` checking for title quotes. The
    // CommonMark link tail allows one title delimited by matching `"`,
    // `'`, or `(...)`. The paren-title form is already covered by `depth`
    // tracking above. We only need to check `"` and `'` for closure.
    let mut k = lparen + 1;
    while k < rparen {
        let b = bytes[k];
        if b == b'\\' && k + 1 < rparen {
            k += 2;
            continue;
        }
        if b == b'"' || b == b'\'' {
            let quote = b;
            let mut m = k + 1;
            let mut closed = false;
            while m < rparen {
                if bytes[m] == b'\\' && m + 1 < rparen {
                    m += 2;
                    continue;
                }
                if bytes[m] == quote {
                    closed = true;
                    break;
                }
                m += 1;
            }
            // The link parser only treats `"`/`'` as title delimiters
            // when preceded by whitespace and followed by content that
            // closes before `)`. If the quote we found surrounds `pos`,
            // closure of THIS quote determines validity. If the quote is
            // unclosed and `pos` sits past the unclosed quote, the link
            // fails and we should not suppress.
            if !closed && pos > k {
                return false;
            }
            if closed {
                k = m + 1;
                continue;
            }
        }
        k += 1;
    }
    true
}

/// True if the byte at `pos` is the first content char of its source line,
/// allowing only whitespace, blockquote markers (`>`), and at most one
/// leading list marker (`-`, `+`, `*`, or `N.`/`N)`) before it. Used to
/// distinguish flow-position `{` (which follows mdx-js's strict-no-lazy
/// flow rule) from text-position `{` after the block-level pass already
/// failed to take it as flow.
#[cfg(feature = "mdx")]
fn is_at_paragraph_line_start(bytes: &[u8], pos: usize) -> bool {
    let mut j = pos;
    while j > 0 && bytes[j - 1] != b'\n' && bytes[j - 1] != b'\r' {
        j -= 1;
    }
    // j..pos is the slice from line start (after newline) up to `{`.
    let line = &bytes[j..pos];
    let mut k = 0;
    while k < line.len() && (line[k] == b' ' || line[k] == b'\t') {
        k += 1;
    }
    while k < line.len() && line[k] == b'>' {
        k += 1;
        if k < line.len() && (line[k] == b' ' || line[k] == b'\t') {
            k += 1;
        }
    }
    if k < line.len() {
        let b = line[k];
        let consumed = if matches!(b, b'-' | b'+' | b'*')
            && line.get(k + 1).is_some_and(|c| *c == b' ' || *c == b'\t')
        {
            Some(k + 2)
        } else if b.is_ascii_digit() {
            let mut m = k + 1;
            while m < line.len() && line[m].is_ascii_digit() {
                m += 1;
            }
            if m < line.len()
                && (line[m] == b'.' || line[m] == b')')
                && line.get(m + 1).is_some_and(|c| *c == b' ' || *c == b'\t')
            {
                Some(m + 2)
            } else {
                None
            }
        } else {
            None
        };
        if let Some(after_marker) = consumed {
            k = after_marker;
            while k < line.len() && (line[k] == b' ' || line[k] == b'\t') {
                k += 1;
            }
        }
    }
    k == line.len()
}

/// True if the line ending at `ix - 1` contains an `<` that opens an inline
/// MDX JSX tag whose scan extends past `ix`. Used to suppress paragraph
/// interrupts on the line beginning at `ix` when a multi-line JSX tag from
/// the previous line is still open — the would-be interrupt char (e.g. the
/// closing `>`) actually belongs to the JSX tag.
///
/// In the lite (non-mdx) build the body is a `false` stub so the `&&`-guard
/// call sites compile unchanged without naming the gated `mdx::*` scanners;
/// `ENABLE_MDX` is never set there, so the real body would have returned
/// without effect anyway.
#[cfg(not(feature = "mdx"))]
fn prev_line_has_open_inline_jsx(_bytes: &[u8], _ix: usize) -> bool {
    false
}

#[cfg(feature = "mdx")]
fn prev_line_has_open_inline_jsx(bytes: &[u8], ix: usize) -> bool {
    if ix == 0 || ix > bytes.len() {
        return false;
    }
    let mut prev_line_end = ix - 1;
    if !matches!(bytes[prev_line_end], b'\n' | b'\r') {
        return false;
    }
    if prev_line_end > 0 && bytes[prev_line_end] == b'\n' && bytes[prev_line_end - 1] == b'\r' {
        prev_line_end -= 1;
    }
    let mut prev_line_start = prev_line_end;
    while prev_line_start > 0 && !matches!(bytes[prev_line_start - 1], b'\n' | b'\r') {
        prev_line_start -= 1;
    }
    let line = &bytes[prev_line_start..prev_line_end];
    // Lines without `<` (or `\`) can't open a JSX tag — skip them outright.
    // For lines that do contain `<`, memchr to each candidate; a per-byte
    // walk dominated the profile for long-line documents.
    if memchr::memchr2(b'<', b'\\', line).is_none() {
        return false;
    }
    let mut offset = 0;
    while offset < line.len() {
        // memchr to the next interesting byte (`<` or `\`). `\<Foo` is a
        // literal `<` (backslash neutralizes the next byte), so we treat
        // `\` as a skip-2 escape.
        let Some(rel) = memchr::memchr2(b'<', b'\\', &line[offset..]) else {
            return false;
        };
        let i = offset + rel;
        if line[i] == b'\\' {
            offset = i + 2;
            continue;
        }
        let pos = prev_line_start + i;
        if let Some(len) = crate::mdx::scan_mdx_inline_jsx(&bytes[pos..]) {
            if pos + len > ix {
                return true;
            }
        }
        offset = i + 1;
    }
    false
}

/// True if `pos` falls inside the opening or closing tag of an inline MDX
/// JSX element on the same source line (back to the previous newline). Used
/// to suppress the first-pass `MdxTextExpression` handler for `{` that
/// actually belongs to a JSX attribute spread or attribute value — those
/// braces will be consumed by the inline JSX scanner in the second pass.
///
/// Lite (non-mdx) build: `false` stub, see [`prev_line_has_open_inline_jsx`].
#[cfg(not(feature = "mdx"))]
fn is_inside_open_inline_jsx_tag(_bytes: &[u8], _pos: usize) -> bool {
    false
}

#[cfg(feature = "mdx")]
fn is_inside_open_inline_jsx_tag(bytes: &[u8], pos: usize) -> bool {
    if pos == 0 || pos > bytes.len() {
        return false;
    }
    // Walk back to the start of the enclosing block: a blank line (two
    // consecutive line endings) or the start of input. Inline JSX tags
    // can span multiple paragraph-continuation lines, so stopping at a
    // single newline mis-classifies `{...}` on a continuation line as
    // text-position when it's actually inside an open JSX attribute.
    let mut line_start = pos;
    let mut seen_newline = false;
    while line_start > 0 {
        let b = bytes[line_start - 1];
        if matches!(b, b'\n' | b'\r') {
            if seen_newline {
                break;
            }
            seen_newline = true;
        } else if !matches!(b, b' ' | b'\t') {
            seen_newline = false;
        }
        line_start -= 1;
    }
    let mut i = line_start;
    while i < pos {
        // memchr to the next `<` or `\` candidate. `\<` is a literal `<`
        // (backslash neutralizes the next byte), so we skip-2 on `\`.
        let Some(rel) = memchr::memchr2(b'<', b'\\', &bytes[i..pos]) else {
            return false;
        };
        let j = i + rel;
        if bytes[j] == b'\\' {
            i = j + 2;
            continue;
        }
        if let Some(len) = crate::mdx::scan_mdx_inline_jsx(&bytes[j..]) {
            if j + len > pos {
                return true;
            }
        }
        i = j + 1;
    }
    false
}

/// True if `pos` falls inside a GFM autolink URL on the current source line
/// that micromark's `literalAutolink` construct would have tokenized — i.e.
/// no unbalanced `[` / `![` precedes it. When micromark *does* tokenize the
/// autolink, backticks (and other potential inline tokens) inside the URL
/// are consumed as part of the URL, not as code-span markers. We'd
/// otherwise let inline code break `https://foo.bar.\`baz>\``.
///
/// When there *is* an unbalanced `[`, micromark's `previousUnbalanced`
/// check disables the autolink construct, so backticks must still tokenize
/// as code spans (and `mdast-util-find-and-replace` later picks the URL
/// out of the broken-label text). We mirror that here.
fn is_inside_gfm_autolink_url(bytes: &[u8], pos: usize) -> bool {
    if pos == 0 || pos >= bytes.len() {
        return false;
    }
    let mut line_start = pos;
    while line_start > 0 && !matches!(bytes[line_start - 1], b'\n' | b'\r') {
        line_start -= 1;
    }
    // Track unbalanced `[` (and `![`) the way micromark does: if there's
    // one open ahead of the URL, the literalAutolink construct doesn't fire.
    let mut bracket_depth: i32 = 0;
    let mut i = line_start;
    while i < pos {
        let b = bytes[i];
        // Skip ONLY valid backslash escapes (`\` + ASCII punct). `\h` etc.
        // is literal text — the URL `\http://...` starts at the `h`, so we
        // mustn't blindly skip past the next byte.
        if b == b'\\' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_punctuation() {
            i += 2;
            continue;
        }
        match b {
            b'[' => bracket_depth += 1,
            b']' if bracket_depth > 0 => {
                bracket_depth -= 1;
                // Closed link text `]` followed by `(` is a CommonMark link
                // destination — micromark tokenizes the link first, so the URL
                // inside isn't visible to the autolink-literal construct. Skip
                // past the matching `)` so e.g. `[a](http://x)，`code`` doesn't
                // gobble the trailing text into a phantom URL.
                if bracket_depth == 0 && bytes.get(i + 1) == Some(&b'(') {
                    let mut j = i + 2;
                    let mut paren_depth: i32 = 1;
                    while j < bytes.len() && paren_depth > 0 {
                        let c = bytes[j];
                        if c == b'\\' && j + 1 < bytes.len() && bytes[j + 1].is_ascii_punctuation()
                        {
                            j += 2;
                            continue;
                        }
                        if matches!(c, b'\n' | b'\r') {
                            break;
                        }
                        match c {
                            b'(' => paren_depth += 1,
                            b')' => paren_depth -= 1,
                            _ => {}
                        }
                        j += 1;
                    }
                    if paren_depth == 0 {
                        i = j;
                        continue;
                    }
                }
            }
            _ => {}
        }
        // `scan_autolink_literal` rejects false positives, so the prefix
        // scan only needs to recognize `http(s)` and `www.`. TODO(layering):
        // move the scanner to a shared module so firstpass doesn't reach
        // into `post_passes`.
        let prefix_match = bracket_depth == 0
            && ((b == b'h'
                && (bytes[i..].starts_with(b"http://") || bytes[i..].starts_with(b"https://")))
                || (b == b'w' && bytes[i..].starts_with(b"www.")));
        if prefix_match && !is_inside_link_destination(bytes, i) {
            if let Some((_, raw_end, _, _, _)) = crate::post_passes::scan_autolink_literal(bytes, i)
            {
                if raw_end > pos {
                    return true;
                }
                i = raw_end;
                continue;
            }
        }
        i += 1;
    }
    false
}

/// True if there's a backtick run of exactly `count` length earlier on the
/// same line as `pos`. Used by the autolink-vs-code-span tie-break in the
/// firstpass: if a backtick *could* close a code span opened earlier, we
/// must let the normal MaybeCode logic decide — micromark fires code-span
/// constructs at the opener position before any later URL ever gets a
/// chance to tokenize.
/// True if an unbalanced `[` (or `![`) sits before `pos` in the same
/// paragraph (i.e. no matching `]` before `pos`). When this holds,
/// micromark's `previousUnbalanced` rule disables the autolink-literal
/// construct, letting backticks tokenize as a code span first. Used as a
/// gate for the forward-looking backtick suppression check.
/// GFM autolink-literal construct, dispatched during inline tokenization
/// from `parse_line`'s callback on `h`/`H`/`w`/`W`/`@` triggers. Mirrors
/// micromark's text-context construct (see
/// `node_modules/.../micromark-extension-gfm-autolink-literal/dev/lib/syntax.js`):
/// the URL bytes are consumed before bracket/image/code-span/emphasis
/// resolvers see them, so cases like `[a](https://x[![alt](url)` produce a
/// trailing literal autolink instead of nesting an image into the failed
/// link's destination.
///
/// Returns `Some((new_begin_text, skip))` if a Link was emitted — the
/// caller should set `begin_text = new_begin_text`, clear
/// `backslash_escaped`, and return `ContinueAndSkip(skip)`.
/// Walk forward from `start_ix` (an atext-class char like `_`) through
/// `+`/`-`/`.`/`_`/alphanumeric to find an `@`, then check whether an
/// email autolink tokenizes exactly at `start_ix..`. Returns `(start, end,
/// "mailto:..")` on success. Mirrors micromark's `emailAutolink` construct
/// when bound to atext start chars.
fn scan_email_forward_from_atext(
    bytes: &[u8],
    start_ix: usize,
    begin_text: usize,
    paragraph_start: usize,
) -> Option<(usize, usize, String)> {
    // The walkback in `scan_email_autolink` would land on `start_ix` only
    // when every byte in `[start_ix, at_ix)` is a local-part char and the
    // char before `start_ix` is not. Bail if `start_ix` isn't at a token
    // boundary in that sense.
    if start_ix > 0 && is_email_local_char(bytes[start_ix - 1]) {
        return None;
    }
    // Scan forward for the `@`.
    let mut at_ix = start_ix;
    while at_ix < bytes.len() && is_email_local_char(bytes[at_ix]) {
        at_ix += 1;
    }
    if at_ix >= bytes.len() || bytes[at_ix] != b'@' {
        return None;
    }
    let (sc_start, sc_end, full_url, retry_needed) = scan_email_autolink(bytes, at_ix)?;
    if retry_needed || sc_start != start_ix {
        return None;
    }
    // Construct path requires the email's local-part not to begin past the
    // current text-emission point (else there are already-emitted Maybe*
    // tokens we'd be stomping on).
    if sc_start < begin_text {
        return None;
    }
    if sc_start < paragraph_start {
        return None;
    }
    Some((sc_start, sc_end, full_url))
}

fn is_email_local_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.' | b'_')
}

#[allow(clippy::too_many_arguments)]
fn try_emit_gfm_autolink<'a>(
    bytes: &[u8],
    ix: usize,
    byte: u8,
    paragraph_start: usize,
    begin_text: usize,
    backslash_escaped: bool,
    options: Options,
    tree: &mut Tree<Item>,
    allocs: &mut Allocations<'a>,
) -> Option<(usize, usize)> {
    // Fast structural reject: every `h`/`H`/`w`/`W`/`@` in prose fires this
    // path, but only a tiny fraction can actually start an autolink. The
    // precedence predicates below each cost O(paragraph) to evaluate, so
    // bail out on the cheap byte-level check first.
    match byte {
        b'h' | b'H' | b'w' | b'W' => {
            let rest = &bytes[ix..];
            if !(rest.starts_with(b"http://")
                || rest.starts_with(b"https://")
                || rest.starts_with(b"www."))
            {
                return None;
            }
            // Pointed-autolink precedence: when an *unescaped* `<`
            // immediately precedes AND a `>` closer exists before line
            // end / whitespace, the CommonMark autolink construct will
            // claim these bytes during MaybeHtml resolution. Cheap, so
            // run before the paragraph-scan predicates.
            if ix > 0 && bytes[ix - 1] == b'<' {
                let backslashes_before_lt = bytes[..ix - 1]
                    .iter()
                    .rev()
                    .take_while(|&&b| b == b'\\')
                    .count();
                let lt_is_escaped = backslashes_before_lt % 2 == 1;
                if !lt_is_escaped {
                    let has_close = bytes[ix..]
                        .iter()
                        .take_while(|&&b| !matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b'<'))
                        .any(|&b| b == b'>');
                    if has_close {
                        return None;
                    }
                }
            }
        }
        b'@' => {
            // Email requires at least one atext char immediately before @.
            if ix == 0 || !is_email_local_char(bytes[ix - 1]) {
                return None;
            }
        }
        _ => return None,
    }

    // previousUnbalanced: suppress when an unclosed `[`/`![` precedes the
    // trigger in this paragraph.
    if has_unbalanced_bracket_from(bytes, paragraph_start, ix) {
        return None;
    }
    // Link/image destination precedence.
    if is_inside_link_destination(bytes, ix) {
        return None;
    }
    // Code span precedence.
    if is_inside_code_span(bytes, ix) {
        return None;
    }
    // Math span precedence (only matters when math is enabled).
    if options.has_math() && is_inside_math_span(bytes, ix) {
        return None;
    }
    // MDX JSX tag precedence (only matters when MDX is enabled).
    if options.contains(Options::ENABLE_MDX) && is_inside_open_inline_jsx_tag(bytes, ix) {
        return None;
    }

    match byte {
        b'h' | b'H' | b'w' | b'W' => {
            // previousProtocol / previousWww are checked inside
            // `scan_autolink_literal` (loose check: prev not ASCII
            // alphabetic). `fnr_only=false` means the *construct* path
            // accepted — exactly the case the inline tokenizer fires on.
            let (start, _raw_end, end, full_url, fnr_only) = scan_autolink_literal(bytes, ix)?;
            if fnr_only {
                return None;
            }
            let link_type = LinkType::Autolink;
            let link_ix = allocs.allocate_link(link_type, full_url.into(), "".into(), "".into());
            tree.append_text(begin_text, start, backslash_escaped);
            let link_node_ix = tree.append(Item {
                start,
                end,
                body: ItemBody::Link(link_ix),
            });
            let text_child = tree.create_node(Item {
                start,
                end,
                body: ItemBody::Text {
                    backslash_escaped: false,
                },
            });
            tree[link_node_ix].child = Some(text_child);
            // Skip the URL bytes so subsequent special-byte callbacks don't
            // re-trigger on `[`, `!`, etc. inside the URL. ContinueAndSkip(N)
            // advances ix by N then +1, so to land at `end` we want N = end - ix - 1.
            // `end > ix` always holds here (the construct requires non-empty
            // body past the scheme).
            let skip = end.saturating_sub(ix + 1);
            Some((end, skip))
        }
        b'@' => {
            // Email walks backward from `@` for atext, so the emitted
            // Link's start may be BEFORE `ix`. micromark fires its email
            // construct FORWARD from the first atext char (not backward
            // from `@`), so when an atext char (e.g. `_`) is shared with
            // another text construct (attentionSequence here) micromark's
            // construct ordering decides which wins. We can't replicate
            // that cleanly mid-iteration; if the walkback would cross
            // back over an already-emitted Maybe* item, defer to the
            // post-pass (which sees the resolved/flat text and runs
            // find-and-replace if the construct rejected it).
            let (email_start, email_end, full_url, retry_needed) = scan_email_autolink(bytes, ix)?;
            if retry_needed {
                return None;
            }
            if email_start < begin_text {
                return None;
            }
            if email_start < paragraph_start {
                return None;
            }
            // Construct-path rejection when an active backslash escape
            // directly precedes the local-part. micromark sees `\X` as one
            // escape token whose char is `X`; since `X` is punctuation
            // (atext for `+`/`-`/`.`/`_`), `previousEmail` rejects.
            // `scan_email_autolink` only checks raw `bytes[start-1] != '/'`
            // so it misses this case. Let the post-pass emit the
            // position-less FNR fallback.
            if email_start > 0
                && bytes[email_start - 1] == b'\\'
                && is_ascii_punctuation(bytes[email_start])
            {
                return None;
            }
            // `scan_email_autolink` returns `mailto:<addr>`; arena_build's
            // Email-link path will prepend `mailto:` again, so strip here.
            let email_addr = full_url
                .strip_prefix("mailto:")
                .map(str::to_owned)
                .unwrap_or(full_url);
            let link_ix =
                allocs.allocate_link(LinkType::Email, email_addr.into(), "".into(), "".into());
            tree.append_text(begin_text, email_start, backslash_escaped);
            let link_node_ix = tree.append(Item {
                start: email_start,
                end: email_end,
                body: ItemBody::Link(link_ix),
            });
            let text_child = tree.create_node(Item {
                start: email_start,
                end: email_end,
                body: ItemBody::Text {
                    backslash_escaped: false,
                },
            });
            tree[link_node_ix].child = Some(text_child);
            let skip = email_end.saturating_sub(ix + 1);
            Some((email_end, skip))
        }
        _ => None,
    }
}

/// True when `pos` sits inside an inline link destination `[label](DEST`
/// whose closing `)` actually exists — i.e. the link parse will succeed.
/// In that case micromark's labelEnd resolver consumes the destination
/// bytes before any text-context construct sees them, so the autolink
/// construct must defer.
///
/// When the would-be destination has no valid closer (e.g. unmatched
/// brackets, runs to EOF without `)`), micromark's labelEnd attempt
/// fails and the destination bytes fall back to text context — the
/// autolink construct *should* fire there.
/// Does the line starting at `pos` open a block-level construct that would
/// break paragraph continuation? Conservative: only matches markers that
/// can't appear mid-paragraph (fenced code, ATX heading, blockquote, list
/// marker, thematic break). Used by `is_inside_link_destination` to decide
/// whether `[…]` can span across the line.
fn line_starts_block(bytes: &[u8], pos: usize) -> bool {
    let mut i = pos;
    // Skip up to 3 cols of leading space (≥4 would be indented code, but
    // that doesn't apply mid-paragraph either — punt and treat as block).
    let mut sp = 0;
    while i < bytes.len() && bytes[i] == b' ' && sp < 4 {
        sp += 1;
        i += 1;
    }
    if sp == 4 {
        return true;
    }
    let Some(&c) = bytes.get(i) else {
        return false;
    };
    match c {
        b'>' => true,
        b'#' => {
            // ATX heading: 1-6 `#` then space/eol.
            let mut h = 0;
            while bytes.get(i + h) == Some(&b'#') && h < 7 {
                h += 1;
            }
            (1..=6).contains(&h)
                && matches!(bytes.get(i + h), None | Some(b' ' | b'\t' | b'\n' | b'\r'))
        }
        b'`' | b'~' => {
            // Fenced code: 3+ identical fence chars.
            let mut n = 0;
            while bytes.get(i + n) == Some(&c) {
                n += 1;
            }
            n >= 3
        }
        b'-' | b'_' | b'*' => {
            // Thematic break: 3+ of the same char, only `- _ *` and spaces.
            let mut j = i;
            let mut count = 0;
            while j < bytes.len() {
                match bytes[j] {
                    b' ' | b'\t' => {}
                    x if x == c => count += 1,
                    b'\n' | b'\r' => break,
                    _ => return false,
                }
                j += 1;
            }
            count >= 3
        }
        _ => false,
    }
}

fn is_inside_link_destination(bytes: &[u8], pos: usize) -> bool {
    if pos < 2 {
        return false;
    }
    // Fast reject: the walkback stops at the first newline, so a `(`
    // outside the current source line can't reach `pos`. If there's none
    // on this line before `pos`, no link tail is possible.
    let line_start = memchr::memrchr2(b'\n', b'\r', &bytes[..pos])
        .map(|i| i + 1)
        .unwrap_or(0);
    if memchr::memchr(b'(', &bytes[line_start..pos]).is_none() {
        return false;
    }
    let mut paren_close_excess: i32 = 0;
    let mut paren_start: Option<usize> = None;
    let mut i = pos;
    while i > 0 {
        i -= 1;
        let b = bytes[i];
        if matches!(b, b'\n' | b'\r') {
            return false;
        }
        if i > 0 && bytes[i - 1] == b'\\' {
            let mut bs = 0;
            let mut j = i;
            while j > 0 && bytes[j - 1] == b'\\' {
                bs += 1;
                j -= 1;
            }
            if bs % 2 == 1 {
                continue;
            }
        }
        match b {
            b')' => paren_close_excess += 1,
            b'(' => {
                if paren_close_excess > 0 {
                    paren_close_excess -= 1;
                } else if i > 0 && bytes[i - 1] == b']' {
                    paren_start = Some(i);
                    break;
                } else {
                    return false;
                }
            }
            _ => {}
        }
    }
    let Some(paren_start) = paren_start else {
        return false;
    };
    // Verify the `]` immediately before `(` has a matching `[` within the
    // same paragraph. A `]` without an opener in the same paragraph can't
    // form a link. CommonMark allows `[…]` to span multiple lines, but a
    // blank line — or a line that opens a block-level construct (fenced
    // code, ATX heading, blockquote, list marker, …) — terminates the
    // paragraph and prevents the link from forming.
    let rbracket = paren_start - 1;
    {
        let mut k = rbracket;
        let mut depth: i32 = 1;
        let mut matched = false;
        let mut just_saw_newline = false;
        while k > 0 {
            k -= 1;
            let b = bytes[k];
            if matches!(b, b'\n' | b'\r') {
                if just_saw_newline {
                    break;
                }
                just_saw_newline = true;
                // After `\n` (walking back), bytes[k+1..] is the line that
                // came AFTER this newline (closer to `]`). If that line opens
                // a new block, the paragraph carrying `[` can't continue
                // into it, so the link can't form.
                if line_starts_block(bytes, k + 1) {
                    break;
                }
                continue;
            }
            if b == b' ' || b == b'\t' {
                continue;
            }
            just_saw_newline = false;
            if k > 0 && bytes[k - 1] == b'\\' {
                let mut bs = 0;
                let mut j = k;
                while j > 0 && bytes[j - 1] == b'\\' {
                    bs += 1;
                    j -= 1;
                }
                if bs % 2 == 1 {
                    continue;
                }
            }
            if b == b']' {
                depth += 1;
            } else if b == b'[' {
                depth -= 1;
                if depth == 0 {
                    matched = true;
                    break;
                }
            }
        }
        if !matched {
            return false;
        }
    }
    // Destination = non-whitespace run with balanced unescaped parens.
    // Titles aren't modeled — those after-destination tokens are uncommon
    // and would re-trigger the autolink check anyway.
    let mut j = paren_start + 1;
    let mut depth: i32 = 0;
    while j < bytes.len() {
        let b = bytes[j];
        if matches!(b, b' ' | b'\t' | b'\n' | b'\r') {
            break;
        }
        if b == b'\\' && j + 1 < bytes.len() {
            j += 2;
            continue;
        }
        match b {
            b'(' => depth += 1,
            b')' => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
        j += 1;
    }
    if depth != 0 {
        return false;
    }
    while j < bytes.len() && matches!(bytes[j], b' ' | b'\t') {
        j += 1;
    }
    j < bytes.len() && bytes[j] == b')'
}

fn has_unbalanced_bracket_in_paragraph(bytes: &[u8], pos: usize) -> bool {
    has_unbalanced_bracket_from(bytes, 0, pos)
}

/// Bounded variant: search back only to `floor` (typically the paragraph
/// start passed to inline tokenization). Avoids leaking brackets from a
/// previous block (e.g. indented code `\t![…\n` would otherwise mark the
/// next paragraph's first byte as bracket-pending).
fn has_unbalanced_bracket_from(bytes: &[u8], floor: usize, pos: usize) -> bool {
    if pos <= floor {
        return false;
    }
    // Fast reject: if no `[` appears anywhere between floor and pos there
    // can't be an unbalanced bracket — bypass both the blank-line walkback
    // and the bracket-depth walk.
    if memchr::memchr(b'[', &bytes[floor..pos]).is_none() {
        return false;
    }
    let mut search_start = floor;
    {
        let mut i = pos;
        while i > floor {
            i -= 1;
            if matches!(bytes[i], b'\n' | b'\r') {
                let mut line_start = i;
                while line_start > floor && !matches!(bytes[line_start - 1], b'\n' | b'\r') {
                    line_start -= 1;
                }
                let line_is_blank = bytes[line_start..i]
                    .iter()
                    .all(|&b| matches!(b, b' ' | b'\t'));
                if line_is_blank {
                    search_start = i + 1;
                    break;
                }
            }
        }
    }
    // Re-check after narrowing the floor: the only `[` in the range may
    // have been before a blank line we just skipped past.
    if memchr::memchr(b'[', &bytes[search_start..pos]).is_none() {
        return false;
    }
    let mut depth: i32 = 0;
    let mut i = search_start;
    while i < pos {
        let b = bytes[i];
        if b == b'\\' {
            i += 2;
            continue;
        }
        match b {
            b'[' => depth += 1,
            b']' if depth > 0 => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    depth > 0
}

/// True if there's a backtick run of exactly `count` length later in the
/// same paragraph, starting at or after `pos`. Mirrors the backward
/// `has_earlier_backtick_run` for the open-side of a code span: a
/// `\`foo\`` inside what would otherwise be an autolink URL must still
/// tokenize as a code span if the closing backticks exist later in the
/// paragraph.
fn has_later_backtick_run(bytes: &[u8], pos: usize, count: usize) -> bool {
    if pos >= bytes.len() {
        return false;
    }
    let mut search_end = bytes.len();
    {
        let mut i = pos;
        while i < bytes.len() {
            if matches!(bytes[i], b'\n' | b'\r') {
                let next = i + 1;
                let line_end = (next..bytes.len())
                    .find(|&j| matches!(bytes[j], b'\n' | b'\r'))
                    .unwrap_or(bytes.len());
                let line_is_blank = bytes[next..line_end]
                    .iter()
                    .all(|&b| matches!(b, b' ' | b'\t'));
                if line_is_blank {
                    search_end = i;
                    break;
                }
                i = next;
                continue;
            }
            i += 1;
        }
    }
    let mut i = pos;
    while i < search_end {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == b'`' {
            let run = 1 + scan_ch_repeat(&bytes[(i + 1)..], b'`');
            if run == count {
                return true;
            }
            i += run;
            continue;
        }
        i += 1;
    }
    false
}

fn has_earlier_backtick_run(bytes: &[u8], pos: usize, count: usize) -> bool {
    if pos == 0 {
        return false;
    }
    // Code spans extend across line breaks, so walk back to the start of
    // the paragraph (the previous blank line or start-of-input). A line-
    // scoped check would miss a backtick opener on a previous line and
    // wrongly suppress a closing backtick that micromark would have
    // matched.
    let mut search_start = 0;
    {
        let mut i = pos;
        while i > 0 {
            i -= 1;
            // Detect blank line: scan back to find line start, then check
            // whether everything from there to the previous newline is ws.
            if matches!(bytes[i], b'\n' | b'\r') {
                let line_end = i;
                let mut j = if i > 0 { i - 1 } else { 0 };
                while j > 0 && !matches!(bytes[j - 1], b'\n' | b'\r') {
                    j -= 1;
                }
                let line_is_blank = bytes[j..line_end]
                    .iter()
                    .all(|&b| matches!(b, b' ' | b'\t'));
                if line_is_blank {
                    search_start = i + 1;
                    break;
                }
            }
        }
    }
    let mut i = search_start;
    while i < pos {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == b'`' {
            let run = 1 + scan_ch_repeat(&bytes[(i + 1)..], b'`');
            if run == count {
                return true;
            }
            i += run;
            continue;
        }
        i += 1;
    }
    false
}

#[cfg(feature = "mdx")]
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
            initial_size: 0,
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

/// Strip up to `max_newlines` trailing `\n` (or `\r`) chars from the
/// current fenced-code-block's content. The current tree position is
/// inside the code block (the loop hasn't popped yet); we shrink the
/// last child text node's range to drop the trailing line endings.
/// `bytes` is the full source byte slice — needed to peek at the last
/// byte before shrinking.
fn trim_trailing_newlines_from_code_block(
    tree: &mut Tree<Item>,
    bytes: &[u8],
    max_newlines: usize,
) {
    let Some(last_child) = tree.cur() else { return };
    if !matches!(tree[last_child].item.body, ItemBody::Text { .. }) {
        return;
    }
    let mut stripped = 0;
    while stripped < max_newlines {
        let start = tree[last_child].item.start;
        let end = tree[last_child].item.end;
        if end <= start {
            break;
        }
        let last = bytes[end - 1];
        if last == b'\n' {
            tree[last_child].item.end = end - 1;
            // Treat `\r\n` as one newline.
            if end >= 2 && bytes[end - 2] == b'\r' {
                tree[last_child].item.end = end - 2;
            }
            stripped += 1;
        } else if last == b'\r' {
            tree[last_child].item.end = end - 1;
            stripped += 1;
        } else {
            break;
        }
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
    // `~~` (and longer) follows the same flanking rules as `**`: it can open
    // when followed by a non-punctuation char, OR when followed by punctuation
    // BUT preceded by whitespace/punctuation (handled by the fall-through at
    // the end of this function). Previously we returned `true` unconditionally
    // here, which let `a~~/foo~~` open a strikethrough — GFM rejects that.
    if delim == b'~' && run_len > 1 && !is_punctuation(next_char) {
        return true;
    }
    let prev_char = s[..ix].chars().last().unwrap();
    // See the matching comment in `delim_run_can_close`: the
    // `prev_char == '~'` shortcut bypasses standard flanking and lets pairing
    // walk across escaped tildes, so it's now gated on subscript mode only.
    if delim == b'~' && options.contains(Options::ENABLE_SUBSCRIPT) && !is_punctuation(next_char) {
        return true;
    }
    if delim == b'~' && options.contains(Options::ENABLE_STRIKETHROUGH) && run_len == 1 {
        return !is_punctuation(next_char)
            || (is_punctuation(next_char)
                && (prev_char.is_whitespace() || is_punctuation(prev_char)));
    }

    // Double quotes can open after a non-space word character. For example, `에"About Me"` has
    // quoted text attached directly after Korean text.
    if delim == b'"' {
        return !is_punctuation(next_char)
            || prev_char.is_whitespace()
            || is_punctuation(prev_char);
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
    // The `prev_char == '~'` shortcut historically let any `~`-adjacent run
    // close, but that bypasses GFM's strict flanking rules and lets a run
    // pair across an escaped (literal) `~`. Subscript mode depends on the
    // relaxed pairing, so gate the shortcut on it.
    if delim == b'~' && options.contains(Options::ENABLE_SUBSCRIPT) {
        return true;
    }
    if delim == b'~' && options.contains(Options::ENABLE_STRIKETHROUGH) && run_len == 1 {
        return !is_punctuation(prev_char)
            || (is_punctuation(prev_char)
                && (next_char.is_whitespace() || is_punctuation(next_char)));
    }

    // Double quotes can close before a non-space word character. For example, `"About Me"로` has
    // Korean text attached directly after the quoted phrase.
    if delim == b'"' {
        return !is_punctuation(prev_char)
            || next_char.is_whitespace()
            || is_punctuation(next_char);
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
    if options.has_math() {
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
    if options.contains(Options::ENABLE_GFM) {
        // GFM literal autolinks: protocol (h/H), www (w/W), email (@).
        // Fires during inline tokenization so URL bytes are consumed
        // before the bracket/emphasis/code-span resolvers see them.
        bytes[b'h' as usize] = true;
        bytes[b'H' as usize] = true;
        bytes[b'w' as usize] = true;
        bytes[b'W' as usize] = true;
        bytes[b'@' as usize] = true;
    }

    bytes
}

enum LoopInstruction<T> {
    /// Continue looking for more special bytes, but skip next few bytes.
    ContinueAndSkip(usize),
    /// Break looping immediately, returning with the given index and value.
    BreakAtWith(usize, T),
}

/// Result of `extend_indented_code_block`: the position end the block
/// should advertise and how many blank lines (line terminators) to
/// append to its `value` to match remark's behavior.
pub(crate) struct IndentCodeExtension {
    pub end_offset: u32,
    pub extra_blank_lines: usize,
}

/// Walk forward from `item.end` past blank/whitespace lines that meet
/// the indented-code-block indent threshold, returning the extended
/// end and the number of blank lines to fold back into the body.
///
/// Returns `None` when:
///   * the item isn't an indented code block, or
///   * the block was opened as a lazy continuation
///     (`IndentCodeBlock(true)`), or
///   * the parent is a blockquote — the blank lines downstream would
///     need to carry the `>` marker to extend, and firstpass already
///     produces the correct end for that case.
pub(crate) fn extend_indented_code_block(
    item: &Item,
    source: &[u8],
    parent_body: Option<&ItemBody>,
    start_column: u32,
    // Where to begin scanning forward — pass the mdast-trimmed end
    // (typically `mdast_position_end(item, source, parent_body)`) so
    // the initial `\n` between the code body and the first blank line
    // is correctly counted toward `extra_blank_lines`.
    start_from: u32,
) -> Option<IndentCodeExtension> {
    if !matches!(item.body, ItemBody::IndentCodeBlock(_)) {
        return None;
    }
    if matches!(item.body, ItemBody::IndentCodeBlock(true)) {
        return None;
    }
    if matches!(parent_body, Some(ItemBody::BlockQuote(_))) {
        return None;
    }

    // A blank-or-content line continues *this* code block only when
    // its tab-expanded indent reaches the chunk-content column
    // (start_column + 3): at top level col 4 of indent, inside a
    // list-item with content-col 3 it's col 6, etc.
    let required_indent_cols = (start_column as usize).saturating_add(3);
    let line_indent_cols = |bytes: &[u8], from: usize| -> usize {
        let mut p = from;
        let mut cols = 0usize;
        while p < bytes.len() {
            match bytes[p] {
                b' ' => cols += 1,
                b'\t' => cols += 4 - (cols % 4),
                _ => break,
            }
            p += 1;
        }
        cols
    };

    let mut pos = start_from as usize;
    let mut last_indented_end: Option<usize> = None;
    let mut last_indented_newlines = 0usize;
    let mut newlines_skipped = 0usize;
    loop {
        while pos < source.len() && (source[pos] == b'\r' || source[pos] == b'\n') {
            if source[pos] == b'\r' {
                pos += 1;
                if pos < source.len() && source[pos] == b'\n' {
                    pos += 1;
                }
            } else {
                pos += 1;
            }
            newlines_skipped += 1;
        }
        if pos >= source.len() {
            break;
        }
        let is_indented = line_indent_cols(source, pos) >= required_indent_cols;
        if !is_indented {
            // Whitespace-only line that doesn't meet the threshold
            // still sits between chunks — its newline contributes to
            // `newlines_skipped`. Anything else ends the extension.
            let mut p = pos;
            while p < source.len() && (source[p] == b' ' || source[p] == b'\t') {
                p += 1;
            }
            if p < source.len() && (source[p] == b'\r' || source[p] == b'\n') {
                pos = p;
                continue;
            }
            break;
        }
        let line_start = pos;
        while pos < source.len() && source[pos] != b'\n' && source[pos] != b'\r' {
            pos += 1;
        }
        let line_content = &source[line_start..pos];
        if !line_content.iter().all(|&b| b == b' ' || b == b'\t') {
            break;
        }
        last_indented_end = Some(pos);
        last_indented_newlines = newlines_skipped;
    }

    last_indented_end.map(|end| IndentCodeExtension {
        end_offset: end as u32,
        // The first walked newline followed the last chunk and is
        // already accounted for by the body content. Any additional
        // newlines are blank inter-chunk lines that need to be folded
        // back into the value.
        extra_blank_lines: last_indented_newlines.saturating_sub(1),
    })
}

/// remark quirk: when a list-item's `cont_end` (the end derived from its
/// children) lands on a line terminator AND a sibling list-item follows,
/// the list-item's position extends through the next item's marker to
/// that item's first content column.
///
/// Example: `- \`\`\`\n- d` — listItem 1 ends at the content column of
/// listItem 2 (start of `d`), not after its own fenced-code body.
///
/// Returns the extended end offset when the rule applies; otherwise
/// `None`.
pub(crate) fn extend_list_item_to_next_sibling_content(
    tree: &Tree<Item>,
    ix: TreeIndex,
    source: &[u8],
    cont_end: u32,
) -> Option<u32> {
    if cont_end == 0 {
        return None;
    }
    if !matches!(source.get(cont_end as usize - 1), Some(b'\n' | b'\r')) {
        return None;
    }
    let next_ix = tree[ix].next?;
    if !matches!(tree[next_ix].item.body, ItemBody::ListItem(..)) {
        return None;
    }
    let child_ix = tree[next_ix].child?;
    let child_start = tree[child_ix].item.start as u32;
    if child_start > cont_end {
        Some(child_start)
    } else {
        None
    }
}

/// Nested-blockquote extension: when an inner BlockQuote sits inside
/// outer BlockQuote(s) and the next line(s) carry the outer `>`
/// marker(s) but NOT this inner one's, the inner bq's end extends
/// through the outer markers. Matches micromark's lazy-bq position:
/// the inner bq absorbs trailing marker-only lines that its parent
/// still claims.
///
/// Skipped when this bq has a next sibling — those marker-only lines
/// then belong to the outer bq's space *between* siblings, not to
/// this inner one.
///
/// Returns the extended `cont_end` when the rule fires, else `None`.
pub(crate) fn extend_inner_blockquote_through_outer_markers(
    tree: &Tree<Item>,
    ix: TreeIndex,
    source: &[u8],
    cont_end: u32,
) -> Option<u32> {
    if !matches!(tree[ix].item.body, ItemBody::BlockQuote(..)) {
        return None;
    }
    if tree[ix].next.is_some() {
        return None;
    }
    let outer_bq_count = tree
        .walk_spine()
        .filter(|&&i| matches!(tree[i].item.body, ItemBody::BlockQuote(_)))
        .count();
    if outer_bq_count == 0 {
        return None;
    }
    let next_start = tree[ix]
        .next
        .map(|n| tree[n].item.start)
        .unwrap_or(source.len());
    let mut pos = cont_end as usize;
    let mut new_end: Option<u32> = None;
    while pos < next_start {
        if pos < source.len() && source[pos] == b'\r' {
            pos += 1;
            if pos < source.len() && source[pos] == b'\n' {
                pos += 1;
            }
        } else if pos < source.len() && source[pos] == b'\n' {
            pos += 1;
        }
        if pos >= next_start || pos >= source.len() {
            break;
        }
        let mut scan = pos;
        let mut markers = 0usize;
        while markers < outer_bq_count && scan < source.len() {
            while scan < source.len() && matches!(source[scan], b' ' | b'\t') {
                scan += 1;
            }
            if scan < source.len() && source[scan] == b'>' {
                scan += 1;
                markers += 1;
            } else {
                break;
            }
        }
        if markers < outer_bq_count {
            break;
        }
        // After the outer markers we must NOT see this inner bq's `>` —
        // and we must NOT see content. If we do, the inner bq either
        // continues normally (let firstpass handle it) or the next
        // block belongs to the outer bq, not to this inner one.
        let mut p = scan;
        while p < source.len() && matches!(source[p], b' ' | b'\t') {
            p += 1;
        }
        if p < source.len() && source[p] == b'>' {
            break;
        }
        if p < source.len() && !matches!(source[p], b'\n' | b'\r') {
            break;
        }
        new_end = Some(scan as u32);
        pos = p;
    }
    new_end
}

/// remark/micromark quirk: when a list inside a blockquote is followed
/// by more blockquote content (e.g. `>>- one\n>>\n  >  > two`), the
/// list "absorbs" the blank `>>` continuation lines between it and the
/// next sibling block. The list's end extends to right after the
/// parent blockquote markers on the last blank continuation line.
///
/// Only fires when the spine above this list is *pure* blockquotes
/// (no enclosing list-item) — see comments inline.
///
/// Returns the extended `cont_end` when the rule fires, else `None`.
pub(crate) fn extend_list_in_blockquote_through_marker_lines(
    tree: &Tree<Item>,
    ix: TreeIndex,
    source: &[u8],
    cont_end: u32,
) -> Option<u32> {
    if !matches!(tree[ix].item.body, ItemBody::List(..)) {
        return None;
    }
    let bq_count = tree
        .walk_spine()
        .filter(|&&i| matches!(tree[i].item.body, ItemBody::BlockQuote(_)))
        .count();
    if bq_count == 0 {
        return None;
    }
    let any_outer_listitem = tree
        .walk_spine()
        .any(|&i| matches!(tree[i].item.body, ItemBody::ListItem(..)));
    if any_outer_listitem {
        return None;
    }
    let next_start = tree[ix]
        .next
        .map(|n| tree[n].item.start)
        .unwrap_or(source.len());
    if next_start <= cont_end as usize {
        return None;
    }

    let mut pos = cont_end as usize;
    let mut new_end = cont_end;
    let mut saw_blank = false;
    let mut stop = false;
    while pos < next_start && !stop {
        if pos < source.len() && source[pos] == b'\r' {
            pos += 1;
            if pos < source.len() && source[pos] == b'\n' {
                pos += 1;
            }
        } else if pos < source.len() && source[pos] == b'\n' {
            pos += 1;
        }
        if pos >= next_start {
            break;
        }
        // CommonMark: a blockquote continuation line allows 0–3 spaces
        // of leading indent. 4+ spaces would make the line an indented
        // code block at the outer scope.
        {
            let mut leading_cols = 0usize;
            let mut ws = pos;
            while ws < source.len() && matches!(source[ws], b' ' | b'\t') && leading_cols < 4 {
                leading_cols += if source[ws] == b'\t' {
                    4 - (leading_cols % 4)
                } else {
                    1
                };
                ws += 1;
            }
            if leading_cols >= 4 {
                break;
            }
        }
        let mut markers_found = 0usize;
        let mut scan = pos;
        while markers_found < bq_count && scan < source.len() {
            while scan < source.len() && matches!(source[scan], b' ' | b'\t') {
                scan += 1;
            }
            if scan < source.len() && source[scan] == b'>' {
                scan += 1;
                markers_found += 1;
            } else {
                break;
            }
        }
        if markers_found < bq_count {
            break;
        }
        let after_markers = scan;
        let mut p = after_markers;
        while p < source.len() && matches!(source[p], b' ' | b'\t') {
            p += 1;
        }
        let blank = p >= source.len() || matches!(source[p], b'\n' | b'\r');
        if blank {
            // Extend through trailing whitespace on the marker line.
            new_end = p as u32;
            saw_blank = true;
            pos = p;
        } else if !saw_blank && source.get(p) == Some(&b'>') {
            // First line, deeper-nested bq that doesn't continue this
            // list. Include the outer markers in the list span, stop.
            new_end = after_markers as u32;
            stop = true;
        } else if !saw_blank {
            // First line, regular text — sibling paragraph in same bq.
            stop = true;
        } else {
            // Blank lines preceded content — don't include its markers.
            stop = true;
        }
    }

    if new_end > cont_end {
        Some(new_end)
    } else {
        None
    }
}

/// Compute the MDAST `position.end` byte offset for a block item.
///
/// The first-pass tree records each block's `item.end` as the offset just
/// past the block's bytes — which for most blocks includes the trailing
/// line terminator. The mdast representation, however, follows remark's
/// rules: most blocks strip all trailing line terminators, but math and
/// fenced-code at EOF preserve them; in-blockquote fenced code at EOF
/// trims; an unclosed list-item fenced code only keeps its trailing `\n`
/// when the outdent line opens a new container marker.
///
/// `parent_body` is the body of the spine parent (the block one level up)
/// at the time the block closes — needed for the blockquote-at-EOF and
/// list-item-fenced-code carve-outs.
pub(crate) fn mdast_position_end(
    item: &Item,
    source: &[u8],
    parent_body: Option<&ItemBody>,
) -> u32 {
    let end = item.end as u32;
    if !item.body.is_block_level() {
        return end;
    }
    let is_math = matches!(item.body, ItemBody::MathBlock(_));
    let math_at_eof = is_math && end as usize >= source.len();
    let is_fenced = matches!(item.body, ItemBody::FencedCodeBlock(_));
    let fenced_at_eof = is_fenced && end as usize >= source.len();
    let parent_is_bq = matches!(parent_body, Some(ItemBody::BlockQuote(_)));
    let parent_is_listitem = matches!(parent_body, Some(ItemBody::ListItem(..)));
    // Blockquote-parented fenced/math at EOF: the blockquote closed
    // because the next non-existent line carries no `>` marker, so
    // the trailing `\n` belongs to neither the inner block nor the
    // blockquote (`>$$\\\n` / `>\`\`\`\n` — both end before the `\n`).
    let fenced_at_eof_in_bq = fenced_at_eof && parent_is_bq;
    let math_at_eof_in_bq = math_at_eof && parent_is_bq;
    // Unclosed fenced/math in a list-item ended by container outdent:
    // the trailing `\n` at end-1 belongs to the inner block ONLY when
    // the outdent line opens a new container (list marker or `>`).
    // When the next block is a leaf (paragraph, heading, indented
    // code), remark drops the `\n`.
    let next_line_opens_container = end > 1
        && matches!(source.get(end as usize - 1), Some(b'\n' | b'\r'))
        && !matches!(source.get(end as usize - 2), Some(b'\n' | b'\r'))
        && parent_is_listitem
        && {
            match source.get(end as usize).copied() {
                Some(b'-' | b'*' | b'+' | b'>') => true,
                Some(c) if c.is_ascii_digit() => {
                    let mut p = end as usize + 1;
                    while p < source.len() && source[p].is_ascii_digit() {
                        p += 1;
                    }
                    matches!(source.get(p), Some(b'.' | b')'))
                }
                _ => false,
            }
        };
    let fenced_unclosed_in_listitem = is_fenced && !fenced_at_eof && next_line_opens_container;
    let math_unclosed_in_listitem = is_math && !math_at_eof && next_line_opens_container;
    let skip_trim =
        (math_at_eof || fenced_at_eof || fenced_unclosed_in_listitem || math_unclosed_in_listitem)
            && !fenced_at_eof_in_bq
            && !math_at_eof_in_bq;
    if skip_trim {
        return end;
    }
    let mut e = end;
    while e > item.start as u32 && matches!(source.get(e as usize - 1), Some(b'\n' | b'\r')) {
        e -= 1;
        // Math/fenced code: only strip a single line terminator.
        if is_math || is_fenced {
            break;
        }
    }
    e
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
        if options.has_math() {
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
