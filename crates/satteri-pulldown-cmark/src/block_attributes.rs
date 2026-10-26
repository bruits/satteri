//! Attributes on blocks ([`Options::ENABLE_ATTRIBUTES`]), placed as
//! markdown-it-attrs places them: a `{...}` ending a paragraph or table cell
//! goes on that block, or on the list item, list or blockquote around it. Runs
//! on the block tree before the arena is built, so the block's text is gone
//! before the inline pass sees it.
//!
//! [`Options::ENABLE_ATTRIBUTES`]: crate::Options::ENABLE_ATTRIBUTES

use alloc::vec::Vec;

use crate::firstpass::{parse_inside_attribute_block, scan_inline_attribute_block};
use crate::parse::{HeadingAttributes, ItemBody, ParserInner};
use crate::tree::TreeIndex;

/// What precedes a trailing attribute block in its paragraph or cell.
enum Lead {
    /// Nothing: the block is the whole paragraph.
    Nothing,
    /// Text on the same line (`para {.x}`).
    Text,
    /// A line break: the block is the paragraph's last line.
    SoftBreak(TreeIndex),
    HardBreak,
}

impl<'input> ParserInner<'input> {
    /// Walks the block tree from the first top-level block, iteratively, as
    /// containers nest arbitrarily deep.
    pub(crate) fn collect_block_attributes(&mut self) {
        let mut ancestors: Vec<TreeIndex> = Vec::new();
        let mut cur = self.tree.cur();
        let mut prev: Option<TreeIndex> = None;
        loop {
            if let Some(ix) = cur {
                let body = self.tree[ix].item.body;
                match body {
                    // The paragraph went into the block before it.
                    ItemBody::Paragraph | ItemBody::TightParagraph
                        if self.paragraph_attributes(ix, &ancestors, prev) =>
                    {
                        cur = self.tree[ix].next;
                        continue;
                    }
                    ItemBody::TableCell | ItemBody::DefinitionListTitle => {
                        self.single_line_attributes(ix)
                    }
                    ItemBody::Heading(_, None) => self.heading_attributes(ix),
                    ItemBody::BlockQuote(_)
                    | ItemBody::List(..)
                    | ItemBody::ListItem(..)
                    | ItemBody::Table(_)
                    | ItemBody::TableHead
                    | ItemBody::TableRow
                    | ItemBody::FootnoteDefinition(_)
                    | ItemBody::ContainerDirective(..)
                    | ItemBody::DefinitionList(_)
                    | ItemBody::DefinitionListDefinition(..)
                        if self.tree[ix].child.is_some() =>
                    {
                        ancestors.push(ix);
                        cur = self.tree[ix].child;
                        prev = None;
                        continue;
                    }
                    _ => {}
                }
                prev = Some(ix);
                cur = self.tree[ix].next;
            } else if let Some(parent) = ancestors.pop() {
                prev = Some(parent);
                cur = self.tree[parent].next;
            } else {
                break;
            }
        }
    }

    /// Returns whether the paragraph was removed, its attributes having gone
    /// to the block before it.
    fn paragraph_attributes(
        &mut self,
        ix: TreeIndex,
        ancestors: &[TreeIndex],
        prev: Option<TreeIndex>,
    ) -> bool {
        let Some((open, chain_start, attributes, lead)) = self.trailing_attribute_block(ix) else {
            return false;
        };
        let parent = ancestors.last().copied();
        let parent_body = parent.map(|parent| self.tree[parent].item.body);
        match lead {
            Lead::Nothing => {
                // A paragraph of attributes alone goes on the list, table or
                // thematic break before it; anywhere else it stays, empty.
                if let Some(prev) = prev
                    && matches!(
                        self.tree[prev].item.body,
                        ItemBody::List(..) | ItemBody::Table(_) | ItemBody::Rule
                    )
                {
                    // The blank line before the paragraph still makes its list
                    // item loose, as it would with the paragraph in place.
                    if let (Some(item), Some(ItemBody::ListItem(indent, _))) = (parent, parent_body)
                        && self.follows_blank_line(self.tree[ix].item.start)
                    {
                        self.tree[item].item.body = ItemBody::ListItem(indent, true);
                    }
                    self.tree[prev].next = self.tree[ix].next;
                    self.add_block_attributes(prev, attributes);
                    return true;
                }
                self.tree[ix].child = None;
                self.add_block_attributes(ix, attributes);
            }
            Lead::Text => self.line_end_attributes(ix, parent, open, chain_start, attributes),
            Lead::SoftBreak(_) | Lead::HardBreak => {
                let target = self.own_line_target(ix, ancestors, open);
                let cut = match lead {
                    Lead::SoftBreak(soft_break) => self.tree[soft_break].item.start,
                    _ => open,
                };
                self.cut_children(ix, cut);
                self.add_block_attributes(target, attributes);
                // Then a block ending the line before it, as in `- item {.x}`.
                if let Some((open, chain_start, attributes, Lead::Text)) =
                    self.trailing_attribute_block(ix)
                {
                    self.line_end_attributes(ix, parent, open, chain_start, attributes);
                }
            }
        }
        false
    }

    /// Gives a line-ending block to the paragraph `ix`, or to the list item or tight definition it opens.
    fn line_end_attributes(
        &mut self,
        ix: TreeIndex,
        parent: Option<TreeIndex>,
        open: usize,
        chain_start: usize,
        attributes: HeadingAttributes<'input>,
    ) {
        let item = parent.filter(|&parent| {
            self.tree[parent].child == Some(ix)
                && matches!(
                    self.tree[parent].item.body,
                    ItemBody::ListItem(..) | ItemBody::DefinitionListDefinition(_, false)
                )
        });
        if may_end_inline_element(self.text, chain_start) {
            self.trailing_attributes
                .insert(ix, (open, attributes, item));
        } else {
            self.cut_children(ix, open);
            self.add_block_attributes(item.unwrap_or(ix), attributes);
        }
    }

    /// Where a block on a line of its own, opening at `open`, goes: on the
    /// block it's written after, at the level the line's `>` markers and
    /// indentation put it, as kramdown places a block IAL. That's the paragraph
    /// (its list item or definition, when tight), or, for a line lazily written
    /// outside some of the blockquotes, list items and definitions around the
    /// paragraph, the outermost of those it leaves (a list item's list).
    fn own_line_target(&self, ix: TreeIndex, ancestors: &[TreeIndex], open: usize) -> TreeIndex {
        let text = self.text;
        let line_start = |offset: usize| text[..offset].rfind('\n').map_or(0, |i| i + 1);
        let prefix = &text[line_start(open)..open];
        let markers = prefix.bytes().filter(|&b| b == b'>').count();
        let mut quotes = 0;
        for (i, &container) in ancestors.iter().enumerate() {
            match self.tree[container].item.body {
                ItemBody::BlockQuote(_) => {
                    quotes += 1;
                    if markers < quotes {
                        return container;
                    }
                }
                ItemBody::ListItem(..) | ItemBody::DefinitionListDefinition(..) => {
                    let content = self.content_start(container);
                    if prefix.len() < content - line_start(content) {
                        return if i > 0 { ancestors[i - 1] } else { container };
                    }
                }
                _ => {}
            }
        }
        match ancestors.last() {
            Some(&container)
                if match self.tree[container].item.body {
                    ItemBody::ListItem(..) => {
                        matches!(self.tree[ix].item.body, ItemBody::TightParagraph)
                    }
                    ItemBody::DefinitionListDefinition(_, loose) => !loose,
                    _ => false,
                } =>
            {
                container
            }
            _ => ix,
        }
    }

    /// A task item's checkbox sits before its paragraph, so it marks where the content starts.
    fn content_start(&self, container: TreeIndex) -> usize {
        self.task_marker(container)
            .or(self.tree[container].child)
            .map_or(self.tree[container].item.start, |first| {
                self.tree[first].item.start
            })
    }

    fn task_marker(&self, item: TreeIndex) -> Option<TreeIndex> {
        let first = self.tree[item].child?;
        let marker = match self.tree[first].item.body {
            ItemBody::TaskListMarker(_) => first,
            _ => self.tree[first].child?,
        };
        matches!(self.tree[marker].item.body, ItemBody::TaskListMarker(_)).then_some(marker)
    }

    pub(crate) fn task_list_class(&self, block: TreeIndex) -> Option<&'static str> {
        match self.tree[block].item.body {
            ItemBody::ListItem(..) => self.task_marker(block).map(|_| "task-list-item"),
            ItemBody::List(..) => {
                let mut item = self.tree[block].child;
                while let Some(ix) = item {
                    if self.task_marker(ix).is_some() {
                        return Some("contains-task-list");
                    }
                    item = self.tree[ix].next;
                }
                None
            }
            _ => None,
        }
    }

    /// Whether the line before the one holding `offset` is blank, container
    /// markers (`>`) aside. A list's extent runs over the blank line after it,
    /// so this reads the source rather than comparing items.
    fn follows_blank_line(&self, offset: usize) -> bool {
        let before = &self.text[..offset];
        let Some(line_start) = before.rfind('\n') else {
            return false;
        };
        let previous = before[..line_start].trim_end_matches('\r');
        let previous = &previous[previous.rfind('\n').map_or(0, |i| i + 1)..];
        previous.bytes().all(|b| matches!(b, b' ' | b'\t' | b'>'))
    }

    fn single_line_attributes(&mut self, ix: TreeIndex) {
        if let Some((open, chain_start, attributes, Lead::Text)) = self.trailing_attribute_block(ix)
        {
            if may_end_inline_element(self.text, chain_start) {
                self.trailing_attributes
                    .insert(ix, (open, attributes, None));
            } else {
                self.cut_children(ix, open);
                self.add_block_attributes(ix, attributes);
            }
        }
    }

    /// A `{...}` the first pass left on a heading because an inline element
    /// may end right before it (`# Title [link](/u){.x}`): it goes on the
    /// heading as the heading is built, if that element doesn't take it.
    fn heading_attributes(&mut self, ix: TreeIndex) {
        if let Some((open, chain_start, attributes, Lead::Text)) = self.trailing_attribute_block(ix)
            && may_end_inline_element(self.text, chain_start)
        {
            self.trailing_attributes
                .insert(ix, (open, attributes, None));
        }
    }

    /// The attribute block ending the inline content of `ix`, its opening
    /// offset, where the run of blocks it ends starts, and what precedes it. Only plain text may make up the block
    /// (an emphasis marker inside is text, as the block takes it first), and a
    /// block right after other markup is left to that markup's inline rule.
    fn trailing_attribute_block(
        &self,
        ix: TreeIndex,
    ) -> Option<(usize, usize, HeadingAttributes<'input>, Lead)> {
        let mut children = Vec::new();
        let mut child = self.tree[ix].child;
        while let Some(c) = child {
            children.push(c);
            child = self.tree[c].next;
        }
        let text = self.text;
        let mut end = self.tree[*children.last()?].item.end;
        while end > 0 && matches!(text.as_bytes()[end - 1], b' ' | b'\t') {
            end -= 1;
        }
        let open = attribute_block_open(text, end)?;
        let attributes = parse_inside_attribute_block(&text[open + 1..end - 1])?;
        // Of several blocks that follow one another (`{.a}{.b}`), only the last
        // goes on the block, the rest staying text; but an inline element before
        // the first reads them all, so what precedes the first decides.
        let chain_start = attribute_chain_start(text, open);

        let first_in_block = children
            .iter()
            .position(|&c| self.tree[c].item.end > open)?;
        for &c in &children[first_in_block..] {
            if !matches!(
                self.tree[c].item.body,
                ItemBody::Text {
                    backslash_escaped: false
                } | ItemBody::MaybeEmphasis(..)
            ) {
                return None;
            }
        }
        let lead = if self.tree[children[first_in_block]].item.start < open {
            Lead::Text
        } else if first_in_block == 0 {
            Lead::Nothing
        } else {
            let before = children[first_in_block - 1];
            match self.tree[before].item.body {
                ItemBody::SoftBreak => Lead::SoftBreak(before),
                ItemBody::HardBreak(_) => Lead::HardBreak,
                _ => Lead::Text,
            }
        };
        Some((open, chain_start, attributes, lead))
    }

    /// Drops the inline children from offset `cut` on, then the spaces left
    /// at the end of the text before it.
    pub(crate) fn cut_children(&mut self, ix: TreeIndex, cut: usize) {
        let bytes = self.text.as_bytes();
        let mut kept: Option<TreeIndex> = None;
        let mut child = self.tree[ix].child;
        while let Some(c) = child {
            if self.tree[c].item.start >= cut {
                break;
            }
            if self.tree[c].item.end > cut {
                self.tree[c].item.end = cut;
            }
            kept = Some(c);
            child = self.tree[c].next;
        }
        match kept {
            Some(last) => self.tree[last].next = None,
            None => self.tree[ix].child = None,
        }

        // Trailing spaces before the block, as before a line ending.
        if let Some(last) = kept
            && matches!(self.tree[last].item.body, ItemBody::Text { .. })
        {
            let item = &mut self.tree[last].item;
            while item.end > item.start && matches!(bytes[item.end - 1], b' ' | b'\t') {
                item.end -= 1;
            }
            if item.end == item.start {
                self.unlink_child(ix, last);
            }
        }
    }

    fn unlink_child(&mut self, parent: TreeIndex, target: TreeIndex) {
        let mut prev: Option<TreeIndex> = None;
        let mut child = self.tree[parent].child;
        while let Some(c) = child {
            if c == target {
                let next = self.tree[c].next;
                match prev {
                    Some(prev) => self.tree[prev].next = next,
                    None => self.tree[parent].child = next,
                }
                return;
            }
            prev = Some(c);
            child = self.tree[c].next;
        }
    }

    fn add_block_attributes(&mut self, target: TreeIndex, attributes: HeadingAttributes<'input>) {
        match self.block_attributes.get_mut(&target) {
            Some(existing) => existing.merge(attributes),
            None => {
                self.block_attributes.insert(target, attributes);
            }
        }
    }
}

/// Whether the character before a block at `open` can close an inline element
/// (`*em*{.x}`, `[a](/u){.x}`), whose own rule then reads the block first;
/// inline markup isn't resolved yet, so this is decided as the paragraph,
/// cell or heading is built (see `trailing_attributes`).
pub(crate) fn may_end_inline_element(text: &str, open: usize) -> bool {
    open > 0
        && matches!(
            text.as_bytes()[open - 1],
            b'*' | b'_' | b'~' | b'`' | b')' | b']' | b'>'
        )
}

/// The `{` opening the attribute block that ends at `end`, skipping braces in quoted values.
pub(crate) fn attribute_block_open(text: &str, end: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let line_start = memchr::memrchr2(b'\n', b'\r', &bytes[..end]).map_or(0, |ix| ix + 1);
    memchr::memchr_iter(b'{', &bytes[line_start..end])
        .map(|ix| line_start + ix)
        .find(|&open| scan_inline_attribute_block(&bytes[open..]) == Some(end - open))
}

/// Where the run of blocks that follow one another (`{.a}{.b}`) and end with
/// the block at `open` starts.
pub(crate) fn attribute_chain_start(text: &str, open: usize) -> usize {
    let mut chain_start = open;
    while chain_start > 0
        && text.as_bytes()[chain_start - 1] == b'}'
        && let Some(earlier) = attribute_block_open(text, chain_start)
        && parse_inside_attribute_block(&text[earlier + 1..chain_start - 1]).is_some()
    {
        chain_start = earlier;
    }
    chain_start
}
