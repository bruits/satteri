//! Parse an HTML string into a HAST arena. Feature-gated behind `from-html`.
//!
//! html5ever's tree builder needs random-access mutation (foster parenting,
//! reparenting, insert-before-sibling), which the append-only `ArenaBuilder`
//! cannot offer, so parsing goes through a flat, index-addressed `Vec<Node>`
//! that is then emitted into the builder in document order.

use std::cell::{Cell, Ref, RefCell};

use html5ever::interface::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::tokenizer::{Doctype, Tag, TagKind, Token, TokenSink};
use html5ever::{
    Attribute, LocalName, Namespace, ParseOpts, Parser, QualName, parse_document, parse_fragment,
    tree_builder::TreeBuilderOpts,
};
use satteri_arena::{Arena, ArenaBuilder, Hast, StringRef};
use satteri_property_info::{PropKind, find_property};

use crate::hast::HastNodeType;
use crate::hast::codec::{
    decode_element_prop, decode_element_prop_count, decode_element_tag, decode_text_data,
    encode_element_data,
};
use crate::hast::properties::property_to_attribute;
use crate::hast::render::is_void_element;
#[cfg(feature = "mdx")]
use crate::mdast::codec::{
    decode_mdx_jsx_attr, decode_mdx_jsx_attr_count, decode_mdx_jsx_element_name,
    decode_mdx_jsx_explicit, encode_mdx_jsx_element_data,
};
use crate::shared::{
    PROP_BOOL_TRUE, PROP_COMMA_SEP, PROP_COMMA_SEP_NUM, PROP_INT, PROP_SPACE_SEP, PROP_STRING,
};

const HTML_NAMESPACE: &str = "http://www.w3.org/1999/xhtml";
const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";

/// The namespace a fragment's own top-level content is parsed in.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum HtmlSpace {
    #[default]
    Html,
    Svg,
}

impl HtmlSpace {
    /// `<template>` is the most permissive insertion mode, so table parts survive outside a table.
    fn context_element(self) -> QualName {
        match self {
            HtmlSpace::Html => QualName::new(
                None,
                Namespace::from(HTML_NAMESPACE),
                LocalName::from("template"),
            ),
            HtmlSpace::Svg => {
                QualName::new(None, Namespace::from(SVG_NAMESPACE), LocalName::from("svg"))
            }
        }
    }
}

/// Handles are indices into `HtmlSink::nodes`; the document is always index 0.
struct Node {
    parent: Option<usize>,
    children: Vec<usize>,
    data: NodeData,
    /// Source-arena node this was fed from; `None` when it came from raw HTML.
    origin: Option<u32>,
    /// Raw chunk this was tokenized out of; `NO_CHUNK` when fed as tokens.
    chunk: u32,
}

const NO_CHUNK: u32 = u32::MAX;

enum NodeData {
    Document,
    Doctype,
    Text {
        contents: StrTendril,
    },
    Comment {
        contents: StrTendril,
    },
    ProcessingInstruction,
    /// Placeholder for the preserved MDX node `stitches[index]` during the
    /// raw-HTML reparse.
    Stitch(usize),
    Element {
        name: QualName,
        attrs: Vec<Attribute>,
        template_contents: Option<usize>,
    },
}

/// Interior mutability because every `TreeSink` method takes `&self`.
struct HtmlSink {
    nodes: RefCell<Vec<Node>>,
    quirks_mode: Cell<QuirksMode>,
    stitch: Option<StitchRecognizer>,
}

impl HtmlSink {
    fn new(stitch: Option<StitchRecognizer>) -> Self {
        HtmlSink {
            nodes: RefCell::new(vec![Node {
                parent: None,
                children: Vec::new(),
                data: NodeData::Document,
                origin: None,
                chunk: NO_CHUNK,
            }]),
            quirks_mode: Cell::new(QuirksMode::NoQuirks),
            stitch,
        }
    }
}

/// Claims the reparse's own MDX placeholder comments as the tree builder
/// creates them. The marker prefix embeds a per-reparse random nonce, so
/// document content cannot forge one. A marker the parser swallowed as text
/// (unclosed raw-text element, split tag, unterminated comment) is never
/// claimed; [`Self::leaked_markers`] reports those for scrubbing.
struct StitchRecognizer {
    prefix: String,
    claimed: RefCell<Vec<bool>>,
}

impl StitchRecognizer {
    fn new(prefix: String) -> Self {
        StitchRecognizer {
            prefix,
            claimed: RefCell::new(Vec::new()),
        }
    }

    /// Reserve the next marker index; the reparse discovers MDX nodes as it feeds them.
    fn register(&self) -> usize {
        let mut claimed = self.claimed.borrow_mut();
        claimed.push(false);
        claimed.len() - 1
    }

    fn claim(&self, contents: &str) -> Option<usize> {
        let index: usize = contents.strip_prefix(self.prefix.as_str())?.parse().ok()?;
        let mut claimed = self.claimed.borrow_mut();
        if index < claimed.len() && !claimed[index] {
            claimed[index] = true;
            Some(index)
        } else {
            None
        }
    }

    /// Markers of stitches that were never claimed during the parse.
    fn leaked_markers(self) -> Vec<String> {
        let claimed = self.claimed.into_inner();
        claimed
            .iter()
            .enumerate()
            .filter(|&(_, &was_claimed)| !was_claimed)
            .map(|(index, _)| format!("{}{}", self.prefix, index))
            .collect()
    }
}

/// Marker prefix carrying a 128-bit nonce. `RandomState` supplies OS entropy
/// without a new dependency.
fn stitch_prefix() -> String {
    use std::hash::{BuildHasher, Hasher};
    let entropy = std::collections::hash_map::RandomState::new();
    let mut lo = entropy.build_hasher();
    lo.write_u64(0);
    let mut hi = entropy.build_hasher();
    hi.write_u64(1);
    format!("satteri:stitch:{:016x}{:016x}:", hi.finish(), lo.finish())
}

fn new_node(nodes: &mut Vec<Node>, data: NodeData) -> usize {
    let id = nodes.len();
    nodes.push(Node {
        parent: None,
        children: Vec::new(),
        data,
        origin: None,
        chunk: NO_CHUNK,
    });
    id
}

fn parent_and_index(nodes: &[Node], target: usize) -> Option<(usize, usize)> {
    let parent = nodes[target].parent?;
    let index = nodes[parent]
        .children
        .iter()
        .position(|&child| child == target)
        .expect("node has a parent but is missing from its children");
    Some((parent, index))
}

fn detach(nodes: &mut [Node], target: usize) {
    if let Some((parent, index)) = parent_and_index(nodes, target) {
        nodes[parent].children.remove(index);
        nodes[target].parent = None;
    }
}

fn append_node(nodes: &mut [Node], parent: usize, child: usize) {
    debug_assert!(
        nodes[child].parent.is_none(),
        "append_node on a node with a parent"
    );
    nodes[child].parent = Some(parent);
    nodes[parent].children.push(child);
}

/// The tree builder expects adjacent text to coalesce into a single node.
fn push_text(nodes: &mut [Node], target: usize, text: &str) -> bool {
    if let NodeData::Text { contents } = &mut nodes[target].data {
        contents.push_slice(text);
        true
    } else {
        false
    }
}

impl TreeSink for HtmlSink {
    type Handle = usize;
    type Output = Self;
    type ElemName<'a> = Ref<'a, QualName>;

    fn finish(self) -> Self {
        self
    }

    fn parse_error(&self, _msg: std::borrow::Cow<'static, str>) {}

    fn get_document(&self) -> usize {
        0
    }

    fn elem_name<'a>(&'a self, target: &'a usize) -> Ref<'a, QualName> {
        Ref::map(self.nodes.borrow(), |nodes| match &nodes[*target].data {
            NodeData::Element { name, .. } => name,
            _ => panic!("elem_name called on a non-element node"),
        })
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> usize {
        let mut nodes = self.nodes.borrow_mut();
        let template_contents = flags
            .template
            .then(|| new_node(&mut nodes, NodeData::Document));
        new_node(
            &mut nodes,
            NodeData::Element {
                name,
                attrs,
                template_contents,
            },
        )
    }

    fn create_comment(&self, text: StrTendril) -> usize {
        if let Some(index) = self.stitch.as_ref().and_then(|s| s.claim(&text)) {
            return new_node(&mut self.nodes.borrow_mut(), NodeData::Stitch(index));
        }
        new_node(
            &mut self.nodes.borrow_mut(),
            NodeData::Comment { contents: text },
        )
    }

    fn create_pi(&self, _target: StrTendril, _data: StrTendril) -> usize {
        new_node(
            &mut self.nodes.borrow_mut(),
            NodeData::ProcessingInstruction,
        )
    }

    fn append(&self, parent: &usize, child: NodeOrText<usize>) {
        let mut nodes = self.nodes.borrow_mut();
        let parent = *parent;
        if let NodeOrText::AppendText(text) = &child
            && let Some(&last) = nodes[parent].children.last()
            && push_text(&mut nodes, last, text)
        {
            return;
        }
        let child = match child {
            NodeOrText::AppendText(text) => new_node(&mut nodes, NodeData::Text { contents: text }),
            NodeOrText::AppendNode(node) => node,
        };
        append_node(&mut nodes, parent, child);
    }

    fn append_before_sibling(&self, sibling: &usize, child: NodeOrText<usize>) {
        let mut nodes = self.nodes.borrow_mut();
        let sibling = *sibling;
        let (parent, index) =
            parent_and_index(&nodes, sibling).expect("append_before_sibling on a parentless node");

        let child = match (child, index) {
            (NodeOrText::AppendText(text), 0) => {
                new_node(&mut nodes, NodeData::Text { contents: text })
            }
            (NodeOrText::AppendText(text), index) => {
                let prev = nodes[parent].children[index - 1];
                if push_text(&mut nodes, prev, &text) {
                    return;
                }
                new_node(&mut nodes, NodeData::Text { contents: text })
            }
            (NodeOrText::AppendNode(node), _) => node,
        };

        // The node may still be attached elsewhere (adoption agency), so detach
        // first, then recompute the sibling's index in case removal shifted it.
        detach(&mut nodes, child);
        let (parent, index) =
            parent_and_index(&nodes, sibling).expect("sibling lost its parent during insertion");
        nodes[child].parent = Some(parent);
        nodes[parent].children.insert(index, child);
    }

    fn append_based_on_parent_node(
        &self,
        element: &usize,
        prev_element: &usize,
        child: NodeOrText<usize>,
    ) {
        let has_parent = self.nodes.borrow()[*element].parent.is_some();
        if has_parent {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(
        &self,
        _name: StrTendril,
        _public_id: StrTendril,
        _system_id: StrTendril,
    ) {
        let mut nodes = self.nodes.borrow_mut();
        let doctype = new_node(&mut nodes, NodeData::Doctype);
        append_node(&mut nodes, 0, doctype);
    }

    fn get_template_contents(&self, target: &usize) -> usize {
        match &self.nodes.borrow()[*target].data {
            NodeData::Element {
                template_contents: Some(contents),
                ..
            } => *contents,
            _ => panic!("get_template_contents called on a non-template element"),
        }
    }

    fn same_node(&self, x: &usize, y: &usize) -> bool {
        x == y
    }

    fn set_quirks_mode(&self, mode: QuirksMode) {
        self.quirks_mode.set(mode);
    }

    fn add_attrs_if_missing(&self, target: &usize, attrs: Vec<Attribute>) {
        let mut nodes = self.nodes.borrow_mut();
        if let NodeData::Element {
            attrs: existing, ..
        } = &mut nodes[*target].data
        {
            for attr in attrs {
                if !existing.iter().any(|present| present.name == attr.name) {
                    existing.push(attr);
                }
            }
        }
    }

    fn remove_from_parent(&self, target: &usize) {
        detach(&mut self.nodes.borrow_mut(), *target);
    }

    fn reparent_children(&self, node: &usize, new_parent: &usize) {
        let mut nodes = self.nodes.borrow_mut();
        let moved = std::mem::take(&mut nodes[*node].children);
        for &child in &moved {
            nodes[child].parent = Some(*new_parent);
        }
        nodes[*new_parent].children.extend(moved);
    }
}

/// A unit of work for the iterative emitter: emit a sink node, copy a
/// preserved node from the source arena, or close the open element.
enum EmitTask {
    Emit(usize),
    EmitArena(u32),
    Close,
}

/// Emit `roots` into the HAST builder in document order.
///
/// Walks with an explicit stack: HTML nesting is unbounded, so recursion
/// would overflow the native stack on adversarially deep input.
///
/// A [`NodeData::Stitch`] node is replaced by the preserved subtree
/// `stitches[N]` from `src` (`None` for [`html_to_hast_arena`], which never
/// stitches). `leaked` markers are scrubbed from emitted text, comments, and
/// attributes.
fn emit(
    nodes: &[Node],
    roots: &[usize],
    builder: &mut ArenaBuilder<Hast>,
    src: Option<&Arena<Hast>>,
    stitches: &[u32],
    leaked: &[String],
) {
    // Seed with the roots reversed, so they pop in document order.
    let mut stack: Vec<EmitTask> = roots.iter().rev().map(|&id| EmitTask::Emit(id)).collect();

    while let Some(task) = stack.pop() {
        let id = match task {
            EmitTask::Close => {
                builder.close_node();
                continue;
            }
            EmitTask::EmitArena(aid) => {
                emit_arena_node(
                    src.expect("EmitArena without a source arena"),
                    aid,
                    builder,
                    &mut stack,
                );
                continue;
            }
            EmitTask::Emit(id) => id,
        };

        let origin = nodes[id].origin.zip(src);

        match &nodes[id].data {
            NodeData::Document => {
                for &child in nodes[id].children.iter().rev() {
                    stack.push(EmitTask::Emit(child));
                }
            }
            NodeData::Doctype => {
                let leaf = builder.add_leaf_raw(HastNodeType::Doctype as u8);
                copy_position(builder, leaf, origin);
            }
            NodeData::Text { contents } => {
                let text = scrub_markers(contents, leaked);
                let text_ref = builder.alloc_string(&text);
                let leaf = builder.add_leaf_raw(HastNodeType::Text as u8);
                builder
                    .arena_mut()
                    .set_type_data(leaf, &text_ref.as_bytes());
                copy_position(builder, leaf, origin);
            }
            NodeData::Comment { contents } => {
                let text = scrub_markers(contents, leaked);
                let text_ref = builder.alloc_string(&text);
                let leaf = builder.add_leaf_raw(HastNodeType::Comment as u8);
                builder
                    .arena_mut()
                    .set_type_data(leaf, &text_ref.as_bytes());
                copy_position(builder, leaf, origin);
            }
            NodeData::Stitch(index) => {
                stack.push(EmitTask::EmitArena(stitches[*index]));
            }
            // HAST has no processing-instruction node; HTML parsing turns `<?...>`
            // into a comment anyway, so this is effectively unreachable.
            NodeData::ProcessingInstruction => {}
            NodeData::Element {
                name,
                attrs,
                template_contents,
            } => {
                // Fed properties are already normalised; copying beats round-tripping.
                match origin.filter(|&(aid, src)| src.get_type_data(aid).len() >= 16) {
                    Some((aid, src)) => {
                        open_arena_element(src, aid, builder);
                    }
                    None => {
                        let tag_ref = builder.alloc_string(&scrub_markers(&name.local, leaked));
                        // The SVG property schema keeps attribute casing (`viewBox`);
                        // the HTML schema normalises it.
                        let in_svg = &*name.ns == SVG_NAMESPACE;
                        let props: Vec<(StringRef, u8, StringRef)> = attrs
                            .iter()
                            // An attribute name containing a leaked marker is junk the
                            // tokenizer minted from marker text; drop it.
                            .filter(|attr| {
                                leaked.is_empty()
                                    || !leaked.iter().any(|m| attr.name.local.contains(m.as_str()))
                            })
                            .map(|attr| {
                                let attr_name = qualified_attr_name(&attr.name);
                                let (property, prop_kind) = find_property(&attr_name, in_svg);
                                let name_ref = builder.alloc_string(&property);
                                let value = scrub_markers(&attr.value, leaked);
                                let (kind, value_ref) =
                                    coerce_value(builder, prop_kind, &attr_name, &value);
                                (name_ref, kind, value_ref)
                            })
                            .collect();
                        let element = builder.open_node_raw(HastNodeType::Element as u8);
                        let data = encode_element_data(tag_ref, &props);
                        builder.arena_mut().set_type_data(element, &data);
                        copy_position(builder, element, origin);
                    }
                }

                stack.push(EmitTask::Close);
                // `<template>` content lives in a detached document, not the
                // element's children. The arena has no separate content field,
                // so emit it as the template's children rather than dropping it.
                if let Some(contents) = template_contents {
                    for &child in nodes[*contents].children.iter().rev() {
                        stack.push(EmitTask::Emit(child));
                    }
                }
                for &child in nodes[id].children.iter().rev() {
                    stack.push(EmitTask::Emit(child));
                }
            }
        }
    }
}

/// Give `node_id` the span of the source node it was fed from, if any.
fn copy_position(
    builder: &mut ArenaBuilder<Hast>,
    node_id: u32,
    origin: Option<(u32, &Arena<Hast>)>,
) {
    let Some((aid, src)) = origin else { return };
    let node = src.get_node(aid);
    builder.arena_mut().set_position(
        node_id,
        node.start_offset,
        node.end_offset,
        node.start_line,
        node.start_column,
        node.end_line,
        node.end_column,
    );
}

/// Open an element copied from the source arena. Children are the caller's business.
fn open_arena_element(src: &Arena<Hast>, aid: u32, builder: &mut ArenaBuilder<Hast>) -> u32 {
    let data = src.get_type_data(aid);
    let tag_ref = builder.alloc_string(src.get_str(decode_element_tag(data)));
    let props: Vec<(StringRef, u8, StringRef)> = (0..decode_element_prop_count(data))
        .map(|i| {
            let (name, kind, value) = decode_element_prop(data, i);
            (
                builder.alloc_string(src.get_str(name)),
                kind,
                builder.alloc_string(src.get_str(value)),
            )
        })
        .collect();
    let element = builder.open_node_raw(HastNodeType::Element as u8);
    let encoded = encode_element_data(tag_ref, &props);
    builder.arena_mut().set_type_data(element, &encoded);
    copy_position(builder, element, Some((aid, src)));
    element
}

/// html5ever splits foreign attrs into prefix + local; the tables key `prefix:local`.
fn qualified_attr_name(name: &QualName) -> std::borrow::Cow<'_, str> {
    match &name.prefix {
        Some(prefix) => std::borrow::Cow::Owned(format!("{prefix}:{}", name.local)),
        None => std::borrow::Cow::Borrowed(&name.local),
    }
}

/// Remove leaked stitch markers from `text`, longest form first so a marker's
/// `<!--`/`-->` shell goes with it. Borrows unchanged when nothing leaked.
fn scrub_markers<'a>(text: &'a str, leaked: &[String]) -> std::borrow::Cow<'a, str> {
    let mut out = std::borrow::Cow::Borrowed(text);
    for marker in leaked {
        if out.contains(marker.as_str()) {
            let scrubbed = out
                .replace(&format!("<!--{marker}-->"), "")
                .replace(&format!("<!--{marker}"), "")
                .replace(marker.as_str(), "");
            out = std::borrow::Cow::Owned(scrubbed);
        }
    }
    out
}

/// Copy the source-arena subtree rooted at `aid` into `builder`, scheduling
/// its children on `stack`. Strings are re-allocated because `src` and the
/// builder have separate pools.
fn emit_arena_node(
    src: &Arena<Hast>,
    aid: u32,
    builder: &mut ArenaBuilder<Hast>,
    stack: &mut Vec<EmitTask>,
) {
    let node_type = src.get_node(aid).node_type;
    let data = src.get_type_data(aid);

    match HastNodeType::from_u8(node_type) {
        Some(HastNodeType::Root) => {
            for &child in src.get_children(aid).iter().rev() {
                stack.push(EmitTask::EmitArena(child));
            }
        }
        Some(HastNodeType::Doctype) => {
            let leaf = builder.add_leaf_raw(HastNodeType::Doctype as u8);
            copy_position(builder, leaf, Some((aid, src)));
        }
        Some(HastNodeType::Text | HastNodeType::Comment | HastNodeType::Raw) if data.len() >= 8 => {
            let value_ref = builder.alloc_string(src.get_str(decode_text_data(data)));
            let leaf = builder.add_leaf_raw(node_type);
            builder
                .arena_mut()
                .set_type_data(leaf, &value_ref.as_bytes());
            copy_position(builder, leaf, Some((aid, src)));
        }
        Some(HastNodeType::Element) if data.len() >= 16 => {
            open_arena_element(src, aid, builder);
            stack.push(EmitTask::Close);
            for &child in src.get_children(aid).iter().rev() {
                stack.push(EmitTask::EmitArena(child));
            }
        }
        #[cfg(feature = "mdx")]
        Some(HastNodeType::MdxJsxElement | HastNodeType::MdxJsxTextElement) if data.len() >= 16 => {
            let name_ref = builder.alloc_string(src.get_str(decode_mdx_jsx_element_name(data)));
            let explicit = decode_mdx_jsx_explicit(data);
            let attrs: Vec<(u8, StringRef, StringRef)> = (0..decode_mdx_jsx_attr_count(data))
                .map(|i| {
                    let (kind, name, value) = decode_mdx_jsx_attr(data, i);
                    (
                        kind,
                        builder.alloc_string(src.get_str(name)),
                        builder.alloc_string(src.get_str(value)),
                    )
                })
                .collect();
            let element = builder.open_node_raw(node_type);
            let encoded = encode_mdx_jsx_element_data(name_ref, &attrs, explicit);
            builder.arena_mut().set_type_data(element, &encoded);
            copy_position(builder, element, Some((aid, src)));
            // Reparse rather than copy, so raw HTML nested inside the MDX
            // element is resolved too.
            reparse_children_into(src, aid, builder);
            builder.close_node();
        }
        #[cfg(feature = "mdx")]
        Some(
            HastNodeType::MdxFlowExpression
            | HastNodeType::MdxTextExpression
            | HastNodeType::MdxEsm,
        ) if data.len() >= 8 => {
            let value_ref = builder.alloc_string(src.get_str(decode_text_data(data)));
            let leaf = builder.add_leaf_raw(node_type);
            builder
                .arena_mut()
                .set_type_data(leaf, &value_ref.as_bytes());
            copy_position(builder, leaf, Some((aid, src)));
        }
        // Unknown or malformed node: emit its children so a bad wrapper never
        // silently swallows a whole subtree.
        _ => {
            for &child in src.get_children(aid).iter().rev() {
                stack.push(EmitTask::EmitArena(child));
            }
        }
    }
}

/// Reparse `parent`'s children as one fragment and emit the result into the
/// currently open node. Raw nodes are tokenized, so a tag opened in one and
/// closed in another resolves; every other node is fed to the tree builder as
/// tokens tagged with the node they came from, which is what lets the emitted
/// tree keep their source positions. Recurses once per nested MDX level via
/// [`emit_arena_node`]. An MDX node whose placeholder the parser swallowed as
/// text (e.g. inside an unclosed raw `<script>`) is dropped and its marker
/// scrubbed from the output.
fn reparse_children_into(src: &Arena<Hast>, parent: u32, builder: &mut ArenaBuilder<Hast>) {
    let mut parser = parse_fragment(
        HtmlSink::new(Some(StitchRecognizer::new(stitch_prefix()))),
        parse_opts(),
        HtmlSpace::Html.context_element(),
        Vec::new(),
        false,
    );
    let mut feed = Feed::default();
    feed_children(&mut parser, src, parent, &mut feed, 0);

    let sink = parser.finish();
    let leaked = sink
        .stitch
        .map(StitchRecognizer::leaked_markers)
        .unwrap_or_default();
    let mut nodes = sink.nodes.into_inner();
    adopt_self_contained_chunks(&mut nodes, &feed.chunks, src);
    let roots = fragment_roots(&nodes);
    emit(&nodes, &roots, builder, Some(src), &feed.stitches, &leaked);
}

/// MDX nodes held back as placeholders, and the raw node behind each chunk.
#[derive(Default)]
struct Feed {
    stitches: Vec<u32>,
    chunks: Vec<u32>,
}

/// Give the one element a raw chunk produced the span of the raw node it came
/// from. Sound only when that element is the whole chunk: one yielding two
/// elements says nothing about where either ends, and spans *within* a chunk
/// need offsets html5ever does not expose.
fn adopt_self_contained_chunks(nodes: &mut [Node], chunks: &[u32], src: &Arena<Hast>) {
    let mut sole_product: Vec<Option<usize>> = vec![None; chunks.len()];
    let mut disqualified = vec![false; chunks.len()];

    for id in 0..nodes.len() {
        let chunk = nodes[id].chunk;
        if chunk == NO_CHUNK {
            continue;
        }
        let nested = nodes[id]
            .parent
            .is_some_and(|parent| nodes[parent].chunk == chunk);
        if nested {
            continue;
        }
        let chunk = chunk as usize;
        if sole_product[chunk].is_some() || !matches!(nodes[id].data, NodeData::Element { .. }) {
            disqualified[chunk] = true;
        }
        sole_product[chunk] = Some(id);
    }

    for chunk in 0..chunks.len() {
        let Some(id) = sole_product[chunk] else {
            continue;
        };
        if disqualified[chunk] || !subtree_is_all_chunk(nodes, id, chunk as u32) {
            continue;
        }
        let NodeData::Element { name, .. } = &nodes[id].data else {
            continue;
        };
        let raw = src.get_type_data(chunks[chunk]);
        if raw.len() >= 8 && is_lone_element_source(src.get_str(decode_text_data(raw)), &name.local)
        {
            nodes[id].origin = Some(chunks[chunk]);
        }
    }
}

/// Whether `html` is exactly one `tag` element and nothing else, so that the raw
/// node's span is that element's span. Deliberately literal: markup the tokenizer
/// drops (`<body>`, a stray `</div>`, a second `<form>`) leaves the element
/// covering less than the text, and every uncertain case declines.
fn is_lone_element_source(html: &str, tag: &str) -> bool {
    let opens = count_tag(html, tag, false);
    let body = html.trim();
    if !starts_with_tag(body, tag) {
        return false;
    }
    if is_void_element(tag) {
        return opens == 1 && count_tag(html, tag, true) == 0 && body.ends_with('>');
    }
    let mut close = String::with_capacity(tag.len() + 3);
    close.push_str("</");
    close.push_str(tag);
    close.push('>');
    opens == 1 && count_tag(html, tag, true) == 1 && body.ends_with(&close)
}

/// Occurrences of `<tag` (or `</tag`). Counts them anywhere, comments and
/// attribute values included, so a lookalike only ever forces a decline.
fn count_tag(html: &str, tag: &str, closing: bool) -> usize {
    let mut needle = String::with_capacity(tag.len() + 2);
    needle.push('<');
    if closing {
        needle.push('/');
    }
    needle.push_str(tag);
    html.match_indices(&needle)
        .filter(|(at, _)| {
            let rest = &html[at + needle.len()..];
            !rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '-')
        })
        .count()
}

fn starts_with_tag(html: &str, tag: &str) -> bool {
    html.strip_prefix('<')
        .and_then(|rest| rest.strip_prefix(tag))
        .is_some_and(|rest| !rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '-'))
}

/// Whether every node under `id` came from `chunk`. Anything else in there means
/// the element outlived its chunk — an unclosed tag that swallowed what follows,
/// or a tag closed by a later raw node — so the chunk's span is not its span.
fn subtree_is_all_chunk(nodes: &[Node], id: usize, chunk: u32) -> bool {
    let mut stack = vec![id];
    while let Some(node) = stack.pop() {
        if nodes[node].chunk != chunk {
            return false;
        }
        stack.extend_from_slice(&nodes[node].children);
    }
    true
}

/// Feed `parent`'s children into an in-flight fragment parse.
fn feed_children(
    parser: &mut Parser<HtmlSink>,
    src: &Arena<Hast>,
    parent: u32,
    feed: &mut Feed,
    depth: u32,
) {
    for &child in src.get_children(parent) {
        crate::stack::with_headroom(depth, || feed_node(parser, src, child, feed, depth));
    }
}

fn feed_node(
    parser: &mut Parser<HtmlSink>,
    src: &Arena<Hast>,
    node_id: u32,
    feed: &mut Feed,
    depth: u32,
) {
    let data = src.get_type_data(node_id);
    let Some(node_type) = HastNodeType::from_u8(src.get_node(node_id).node_type) else {
        feed_children(parser, src, node_id, feed, depth + 1);
        return;
    };

    match node_type {
        HastNodeType::Root => feed_children(parser, src, node_id, feed, depth + 1),
        HastNodeType::Raw if data.len() >= 8 => {
            let html = src.get_str(decode_text_data(data));
            let chunk = feed.chunks.len() as u32;
            feed.chunks.push(node_id);
            let before = node_count(parser);
            parser.process(StrTendril::from_slice(html));
            let mut nodes = parser.tokenizer.sink.sink.nodes.borrow_mut();
            for node in nodes.iter_mut().skip(before) {
                node.chunk = chunk;
            }
        }
        HastNodeType::Element if data.len() >= 16 => {
            let tag = src.get_str(decode_element_tag(data));
            let in_svg = tag == "svg";
            let attrs = (0..decode_element_prop_count(data))
                .filter_map(|i| {
                    let (name_ref, kind, value_ref) = decode_element_prop(data, i);
                    let name = property_to_attribute(src.get_str(name_ref), in_svg);
                    let value = match kind {
                        PROP_BOOL_TRUE => "",
                        PROP_STRING | PROP_INT | PROP_SPACE_SEP | PROP_COMMA_SEP
                        | PROP_COMMA_SEP_NUM => src.get_str(value_ref),
                        _ => return None,
                    };
                    Some(Attribute {
                        name: QualName::new(None, Namespace::from(""), LocalName::from(&*name)),
                        value: StrTendril::from_slice(value),
                    })
                })
                .collect();
            let element = feed_token(
                parser,
                tag_token(TagKind::StartTag, tag, attrs),
                Some(node_id),
            );
            if !is_void_element(tag) {
                let first_child = node_count(parser);
                feed_children(parser, src, node_id, feed, depth + 1);
                feed_token(parser, tag_token(TagKind::EndTag, tag, Vec::new()), None);
                if let Some(element) = element
                    && escaped_element(parser, element, first_child)
                {
                    parser.tokenizer.sink.sink.nodes.borrow_mut()[element].origin = None;
                }
            }
        }
        HastNodeType::Text if data.len() >= 8 => {
            let text = src.get_str(decode_text_data(data));
            feed_token(
                parser,
                Token::CharacterTokens(StrTendril::from_slice(text)),
                Some(node_id),
            );
        }
        HastNodeType::Comment if data.len() >= 8 => {
            let text = src.get_str(decode_text_data(data));
            feed_token(
                parser,
                Token::CommentToken(StrTendril::from_slice(text)),
                Some(node_id),
            );
        }
        HastNodeType::Doctype => {
            feed_token(
                parser,
                Token::DoctypeToken(Doctype {
                    name: Some(StrTendril::from_slice("html")),
                    ..Doctype::default()
                }),
                Some(node_id),
            );
        }
        // MDX has no HTML form. The marker goes through the tokenizer so a raw
        // text context still swallows it, as an element's tokens never can.
        HastNodeType::MdxJsxElement
        | HastNodeType::MdxJsxTextElement
        | HastNodeType::MdxFlowExpression
        | HastNodeType::MdxTextExpression
        | HastNodeType::MdxEsm => {
            let Some(stitch) = parser.tokenizer.sink.sink.stitch.as_ref() else {
                return;
            };
            let marker = format!("<!--{}{}-->", stitch.prefix, stitch.register());
            feed.stitches.push(node_id);
            parser.process(StrTendril::from_slice(&marker));
        }
        _ => {}
    }
}

fn tag_token(kind: TagKind, name: &str, attrs: Vec<Attribute>) -> Token {
    Token::TagToken(Tag {
        kind,
        name: LocalName::from(name),
        self_closing: false,
        attrs,
        had_duplicate_attributes: false,
    })
}

/// Process one synthesised token and mark what it created as coming from
/// `origin`, returning that node. The tree builder creates the token's own node
/// last, after any element it had to reconstruct first, and creates nothing at
/// all when text merges into a preceding text node.
fn feed_token(parser: &mut Parser<HtmlSink>, token: Token, origin: Option<u32>) -> Option<usize> {
    let tree_builder = &parser.tokenizer.sink;
    let before = tree_builder.sink.nodes.borrow().len();
    let _ = tree_builder.process_token(token, 1);
    let mut nodes = tree_builder.sink.nodes.borrow_mut();
    if nodes.len() == before {
        return None;
    }
    let id = nodes.len() - 1;
    nodes[id].origin = origin;
    Some(id)
}

fn node_count(parser: &Parser<HtmlSink>) -> usize {
    parser.tokenizer.sink.sink.nodes.borrow().len()
}

/// Whether the parser closed `element` before its end tag and carried on
/// outside it, which is what happens when block-level raw HTML interrupts a
/// Markdown element. What it holds is then a fragment of the source node, so
/// the source span would overstate it.
fn escaped_element(parser: &Parser<HtmlSink>, element: usize, first_child: usize) -> bool {
    let nodes = parser.tokenizer.sink.sink.nodes.borrow();
    if nodes.len() <= first_child {
        return false;
    }
    let mut cursor = nodes.len() - 1;
    loop {
        if cursor == element {
            return false;
        }
        match nodes[cursor].parent {
            Some(parent) => cursor = parent,
            None => return true,
        }
    }
}

/// Coerce an attribute string into its typed wire `(kind, value)` pair.
/// `attr_name` is the lowercased attribute name: a boolean attribute is only
/// `true` when its value is empty or repeats the name (`disabled="disabled"`);
/// any other value stays a string (`disabled="false"` is NOT `true`).
fn coerce_value(
    builder: &mut ArenaBuilder<Hast>,
    kind: PropKind,
    attr_name: &str,
    value: &str,
) -> (u8, StringRef) {
    match kind {
        PropKind::Boolean | PropKind::OverloadedBoolean
            if value.is_empty() || value.eq_ignore_ascii_case(attr_name) =>
        {
            (PROP_BOOL_TRUE, StringRef::empty())
        }
        PropKind::Number if is_numeric(value) => (PROP_INT, builder.alloc_string(value)),
        PropKind::SpaceSeparated => {
            let joined = value.split_whitespace().collect::<Vec<_>>().join(" ");
            (PROP_SPACE_SEP, builder.alloc_string(&joined))
        }
        PropKind::CommaSeparated => {
            let joined = split_comma(value).join(",");
            (PROP_COMMA_SEP, builder.alloc_string(&joined))
        }
        PropKind::NumberCommaSeparated => {
            let joined = split_comma(value).join(",");
            (PROP_COMMA_SEP_NUM, builder.alloc_string(&joined))
        }
        PropKind::CommaOrSpaceSeparated => {
            let joined = value
                .split(|c: char| c == ',' || c.is_ascii_whitespace())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            (PROP_SPACE_SEP, builder.alloc_string(&joined))
        }
        // `String`, plus the non-empty overloaded-boolean/number fallbacks.
        _ => (PROP_STRING, builder.alloc_string(value)),
    }
}

/// Split a comma-separated value: items are trimmed, interior empty items are
/// kept, and only a trailing empty item is dropped (`"a,,b"` → `["a","","b"]`,
/// `"a,"` → `["a"]`).
fn split_comma(value: &str) -> Vec<&str> {
    let mut items: Vec<&str> = value.split(',').map(str::trim).collect();
    if items.last() == Some(&"") {
        items.pop();
    }
    items
}

/// Whether `value` coerces to a number under JavaScript `Number()` semantics,
/// which is what consumers use to read the wire value back: decimal (with
/// optional sign/exponent), `0x`/`0o`/`0b` integer literals, and exactly-spelled
/// `Infinity`. Rust-only spellings (`inf`, `nan`, lowercase `infinity`) and
/// `NaN` stay strings.
fn is_numeric(value: &str) -> bool {
    let t = value.trim();
    if t.is_empty() {
        return false;
    }
    for (prefix, radix) in [
        ("0x", 16),
        ("0X", 16),
        ("0o", 8),
        ("0O", 8),
        ("0b", 2),
        ("0B", 2),
    ] {
        if let Some(digits) = t.strip_prefix(prefix) {
            return !digits.is_empty() && digits.chars().all(|c| c.is_digit(radix));
        }
    }
    let unsigned = t.strip_prefix(['+', '-']).unwrap_or(t);
    if unsigned == "Infinity" {
        return true;
    }
    if unsigned.eq_ignore_ascii_case("inf")
        || unsigned.eq_ignore_ascii_case("infinity")
        || unsigned.eq_ignore_ascii_case("nan")
    {
        return false;
    }
    t.parse::<f64>().is_ok()
}

/// Scripting disabled, so `<noscript>` content parses as markup rather than
/// as a single raw-text node.
fn parse_opts() -> ParseOpts {
    ParseOpts {
        tree_builder: TreeBuilderOpts {
            scripting_enabled: false,
            ..TreeBuilderOpts::default()
        },
        ..ParseOpts::default()
    }
}

/// Parse an HTML document string into a HAST arena: a `root` whose children
/// are the doctype (if any) and the implied `<html>` subtree.
pub fn html_to_hast_arena(html: &str) -> Arena<Hast> {
    let sink = parse_document(HtmlSink::new(None), parse_opts()).one(html);
    let nodes = sink.nodes.into_inner();

    let mut builder = ArenaBuilder::<Hast>::new(String::new());
    builder.open_node_raw(HastNodeType::Root as u8);
    emit(&nodes, &nodes[0].children, &mut builder, None, &[], &[]);
    builder.close_node();
    builder.finish()
}

/// Parse an HTML fragment into a HAST arena: a `root` whose children are the
/// fragment's own top-level nodes, with no synthesised `<html>`/`<head>`/
/// `<body>` wrapper. `space` picks the namespace that content parses in.
pub fn html_fragment_to_hast_arena(html: &str, space: HtmlSpace) -> Arena<Hast> {
    let (nodes, roots, _) = parse_fragment_nodes(html, None, space);

    let mut builder = ArenaBuilder::<Hast>::new(String::new());
    builder.open_node_raw(HastNodeType::Root as u8);
    emit(&nodes, &roots, &mut builder, None, &[], &[]);
    builder.close_node();
    builder.finish()
}

/// Parse an HTML fragment into a wrap-payload arena: the single element
/// becomes node 0, the shape `Patch::Wrap` takes as the wrapper. Whitespace
/// around it is ignored; anything else (no element, extra top-level nodes, a
/// void element) errors with the reason.
pub fn html_fragment_to_wrap_arena(html: &str) -> Result<Arena<Hast>, String> {
    let (nodes, roots, _) = parse_fragment_nodes(html, None, HtmlSpace::Html);
    let mut wrapper: Option<usize> = None;
    for &r in &roots {
        match &nodes[r].data {
            NodeData::Text { contents } if contents.trim().is_empty() => {}
            NodeData::Element { name, .. } if wrapper.is_none() => {
                if is_void_element(&name.local) {
                    return Err(format!(
                        "is a void element (<{}>), which cannot hold the wrapped node",
                        name.local
                    ));
                }
                wrapper = Some(r);
            }
            _ => return Err("must parse to exactly one element".to_string()),
        }
    }
    let Some(wrapper) = wrapper else {
        return Err("must parse to exactly one element".to_string());
    };
    let mut builder = ArenaBuilder::<Hast>::new(String::new());
    emit(&nodes, &[wrapper], &mut builder, None, &[], &[]);
    Ok(builder.finish())
}

/// Reparse the raw HTML embedded in a HAST arena into real HAST nodes.
///
/// The whole tree is rendered back to HTML (raw nodes verbatim) and reparsed
/// as one fragment, so a tag opened in one raw node and closed in a later one
/// resolves against the surrounding markup. The result is a fresh `root` with
/// no synthesised `<html>`/`<head>`/`<body>` wrapper.
///
/// MDX nodes have no HTML form; they are carried through as placeholder
/// comments and spliced back afterwards. Child positions do not survive the
/// rebuild from serialised HTML; the source string and the root's own span
/// describe the document rather than the reparse, so both carry over.
pub fn raw_to_hast_arena(arena: &Arena<Hast>) -> Arena<Hast> {
    let mut builder = ArenaBuilder::<Hast>::new(arena.source().to_string());
    builder.open_node_raw(HastNodeType::Root as u8);
    let root = arena.get_node(0);
    builder.set_position_current(
        root.start_offset,
        root.end_offset,
        root.start_line,
        root.start_column,
        root.end_line,
        root.end_column,
    );
    reparse_children_into(arena, 0, &mut builder);
    builder.close_node();
    builder.finish()
}

/// Parse an HTML fragment in `space`'s context element, the insertion mode the
/// fragment's top-level content is read in. Returns the node list, the
/// top-level node indices, and the markers of any swallowed stitches.
fn parse_fragment_nodes(
    html: &str,
    stitch: Option<StitchRecognizer>,
    space: HtmlSpace,
) -> (Vec<Node>, Vec<usize>, Vec<String>) {
    let sink = parse_fragment(
        HtmlSink::new(stitch),
        parse_opts(),
        space.context_element(),
        Vec::new(),
        false,
    )
    .one(html);
    let leaked = sink
        .stitch
        .map(StitchRecognizer::leaked_markers)
        .unwrap_or_default();
    let nodes = sink.nodes.into_inner();
    let roots = fragment_roots(&nodes);
    (nodes, roots, leaked)
}

/// The fragment algorithm wraps content in a synthesised `<html>` root; the
/// fragment's own top-level nodes are that wrapper's children.
fn fragment_roots(nodes: &[Node]) -> Vec<usize> {
    match nodes[0].children.first() {
        Some(&html_root) => nodes[html_root].children.clone(),
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hast::codec::decode_element_tag;
    use crate::hast::hast_arena_to_html;

    fn render(html: &str) -> String {
        hast_arena_to_html(&html_to_hast_arena(html))
            .trim_end()
            .to_string()
    }

    /// Collect element tag names in document order.
    fn tags(arena: &Arena<Hast>) -> Vec<String> {
        fn walk(arena: &Arena<Hast>, id: u32, out: &mut Vec<String>) {
            let node = arena.get_node(id);
            if node.node_type == HastNodeType::Element as u8 {
                let tag = arena.get_str(decode_element_tag(arena.get_type_data(id)));
                out.push(tag.to_string());
            }
            for &child in arena.get_children(id) {
                walk(arena, child, out);
            }
        }
        let mut out = Vec::new();
        walk(arena, 0, &mut out);
        out
    }

    #[test]
    fn wraps_document_in_root_html_head_body() {
        let arena = html_to_hast_arena("<p>hi</p>");
        assert_eq!(arena.get_node(0).node_type, HastNodeType::Root as u8);
        assert_eq!(tags(&arena), ["html", "head", "body", "p"]);
    }

    #[test]
    fn fragment_keeps_top_level_nodes_under_root() {
        let arena = html_fragment_to_hast_arena("<p>hi</p>", HtmlSpace::Html);
        assert_eq!(arena.get_node(0).node_type, HastNodeType::Root as u8);
        assert_eq!(tags(&arena), ["p"]);
    }

    #[test]
    fn fragment_keeps_every_sibling() {
        assert_eq!(
            tags(&html_fragment_to_hast_arena(
                "<p>a</p><p>b</p>",
                HtmlSpace::Html
            )),
            ["p", "p"]
        );
    }

    #[test]
    fn fragment_keeps_table_parts_outside_a_table() {
        assert_eq!(
            tags(&html_fragment_to_hast_arena(
                "<tr><td>a</td></tr>",
                HtmlSpace::Html
            )),
            ["tr", "td"]
        );
    }

    #[test]
    fn fragment_round_trips_without_document_wrapper() {
        assert_eq!(
            hast_arena_to_html(&html_fragment_to_hast_arena("<p>hi</p>", HtmlSpace::Html))
                .trim_end(),
            "<p>hi</p>"
        );
    }

    #[test]
    fn svg_space_self_closes_and_keeps_camel_cased_tags() {
        assert_eq!(
            hast_arena_to_html(&html_fragment_to_hast_arena(
                r#"<circle cx="1" /><clipPath id="c"></clipPath>"#,
                HtmlSpace::Svg
            ))
            .trim_end(),
            r#"<circle cx="1"></circle><clipPath id="c"></clipPath>"#
        );
    }

    #[test]
    fn html_space_does_not_self_close_svg_children() {
        assert_eq!(
            tags(&html_fragment_to_hast_arena(
                r#"<circle cx="1" /><clipPath id="c"></clipPath>"#,
                HtmlSpace::Html
            )),
            ["circle", "clippath"]
        );
    }

    #[test]
    fn structured_element_and_text_round_trip() {
        assert_eq!(
            render("<p>hi</p>"),
            "<html><head></head><body><p>hi</p></body></html>"
        );
    }

    #[test]
    fn preserves_attributes_in_order() {
        assert_eq!(
            render(r#"<a href="/x" class="y">z</a>"#),
            r#"<html><head></head><body><a href="/x" class="y">z</a></body></html>"#
        );
    }

    #[test]
    fn decodes_and_re_escapes_entities() {
        assert_eq!(
            render("<p>a &amp; b &lt; c</p>"),
            "<html><head></head><body><p>a &amp; b &lt; c</p></body></html>"
        );
    }

    #[test]
    fn keeps_comments() {
        assert_eq!(
            render("<div><!--note--></div>"),
            "<html><head></head><body><div><!--note--></div></body></html>"
        );
    }

    #[test]
    fn void_elements_have_no_closing_tag() {
        assert_eq!(
            render(r#"<img src="a.png">"#),
            r#"<html><head></head><body><img src="a.png"></body></html>"#
        );
    }

    #[test]
    fn preserves_doctype() {
        let arena = html_to_hast_arena("<!doctype html><title>t</title>");
        assert_eq!(
            arena.get_node(arena.get_children(0)[0]).node_type,
            HastNodeType::Doctype as u8
        );
        assert_eq!(
            hast_arena_to_html(&arena).trim_end(),
            "<!doctype html><html><head><title>t</title></head><body></body></html>"
        );
    }

    #[test]
    fn implies_tbody_and_foster_parents_stray_content() {
        // Exercises append_before_sibling.
        let out = render("<table><b>x</b><tr><td>y</td></tr></table>");
        assert!(out.contains("<b>x</b><table>"), "foster parenting: {out}");
        assert!(
            out.contains("<tbody><tr><td>y</td></tr></tbody>"),
            "implied tbody: {out}"
        );
    }

    #[test]
    fn handles_misnested_tags_via_adoption_agency() {
        // Exercises reparent_children.
        let out = render("<b>1<p>2</b>3</p>");
        assert!(
            out.contains("<b>1</b><p><b>2</b>3</p>"),
            "adoption agency: {out}"
        );
    }

    #[test]
    fn keeps_raw_text_element_content_unescaped() {
        let out = render("<script>a < b && c</script>");
        assert!(
            out.contains("<script>a < b && c</script>"),
            "raw text: {out}"
        );
    }

    #[test]
    fn parses_nested_elements() {
        assert_eq!(
            render("<ul><li>one</li><li>two</li></ul>"),
            "<html><head></head><body><ul><li>one</li><li>two</li></ul></body></html>"
        );
    }

    #[test]
    fn preserves_template_content() {
        assert_eq!(
            render("<template><p>hi</p></template>"),
            "<html><head><template><p>hi</p></template></head><body></body></html>"
        );
        assert_eq!(
            render("<template>foo</template>"),
            "<html><head><template>foo</template></head><body></body></html>"
        );
    }

    #[test]
    fn parses_noscript_content_as_markup_with_scripting_disabled() {
        let out = render("<head><noscript><link><!--c--></noscript>");
        assert!(
            out.contains("<noscript><link><!--c--></noscript>"),
            "noscript parsed as markup: {out}"
        );
    }

    #[test]
    fn deeply_nested_input_does_not_overflow_the_stack() {
        // Count by scanning the flat arena: a recursive walk would itself
        // overflow and defeat the test. `<span>` avoids html5ever's per-token
        // scope re-scans, keeping the parse linear at this depth.
        let depth = 50_000;
        let arena = html_to_hast_arena(&"<span>".repeat(depth));

        let mut spans = 0usize;
        for id in 0..arena.len() as u32 {
            if arena.get_node(id).node_type == HastNodeType::Element as u8 {
                let tag = arena.get_str(decode_element_tag(arena.get_type_data(id)));
                if tag == "span" {
                    spans += 1;
                }
            }
        }
        assert_eq!(spans, depth, "every nested <span> should survive the walk");
    }

    use crate::hast::codec::{decode_element_prop, decode_element_prop_count};

    /// Decode an element's properties as `(name, kind, value)` triples.
    fn props_of(arena: &Arena<Hast>, tag: &str) -> Vec<(String, u8, String)> {
        for id in 0..arena.len() as u32 {
            if arena.get_node(id).node_type == HastNodeType::Element as u8 {
                let data = arena.get_type_data(id);
                if arena.get_str(decode_element_tag(data)) != tag {
                    continue;
                }
                return (0..decode_element_prop_count(data))
                    .map(|i| {
                        let (name, kind, value) = decode_element_prop(data, i);
                        (
                            arena.get_str(name).to_string(),
                            kind,
                            arena.get_str(value).to_string(),
                        )
                    })
                    .collect();
            }
        }
        panic!("no <{tag}> element found");
    }

    #[test]
    fn normalizes_attributes_like_property_information() {
        let arena = html_to_hast_arena(
            r#"<a class="x  y" href="/h" download tabindex="3" data-foo-bar="1" aria-label="l">z</a>"#,
        );
        assert_eq!(
            props_of(&arena, "a"),
            vec![
                ("className".into(), PROP_SPACE_SEP, "x y".into()),
                ("href".into(), PROP_STRING, "/h".into()),
                ("download".into(), PROP_BOOL_TRUE, String::new()),
                ("tabIndex".into(), PROP_INT, "3".into()),
                ("dataFooBar".into(), PROP_STRING, "1".into()),
                ("ariaLabel".into(), PROP_STRING, "l".into()),
            ]
        );
    }

    #[test]
    fn overloaded_boolean_and_numeric_fallbacks() {
        let arena = html_to_hast_arena(r#"<a download="f.txt">x</a><img width="auto">"#);
        assert_eq!(
            props_of(&arena, "a"),
            [("download".into(), PROP_STRING, "f.txt".into())]
        );
        assert_eq!(
            props_of(&arena, "img"),
            [("width".into(), PROP_STRING, "auto".into())]
        );
    }

    /// A boolean attribute is `true` only when its value is empty or repeats
    /// the attribute name (case-insensitively); anything else stays a string.
    #[test]
    fn boolean_true_only_for_empty_or_name_matching_values() {
        let arena = html_to_hast_arena(
            r#"<input disabled><input disabled="" data-i="2"><input disabled="disabled" data-i="3"><input disabled="DISABLED" data-i="4"><input disabled="false" data-i="5"><input checked="0" data-i="6"><a download="download">x</a><div hidden="hidden">y</div>"#,
        );
        let all: Vec<Vec<(String, u8, String)>> = (0..arena.len() as u32)
            .filter(|&id| arena.get_node(id).node_type == HastNodeType::Element as u8)
            .filter(|&id| {
                let tag = decode_element_tag(arena.get_type_data(id));
                arena.get_str(tag) == "input"
            })
            .map(|id| {
                let data = arena.get_type_data(id);
                (0..decode_element_prop_count(data))
                    .map(|i| {
                        let (name, kind, value) = decode_element_prop(data, i);
                        (
                            arena.get_str(name).to_string(),
                            kind,
                            arena.get_str(value).to_string(),
                        )
                    })
                    .filter(|(name, ..)| !name.starts_with("dataI"))
                    .collect()
            })
            .collect();
        assert_eq!(
            all,
            [
                vec![("disabled".to_string(), PROP_BOOL_TRUE, String::new())],
                vec![("disabled".to_string(), PROP_BOOL_TRUE, String::new())],
                vec![("disabled".to_string(), PROP_BOOL_TRUE, String::new())],
                vec![("disabled".to_string(), PROP_BOOL_TRUE, String::new())],
                vec![("disabled".to_string(), PROP_STRING, "false".to_string())],
                vec![("checked".to_string(), PROP_STRING, "0".to_string())],
            ]
        );
        assert_eq!(
            props_of(&arena, "a"),
            [("download".into(), PROP_BOOL_TRUE, String::new())]
        );
        assert_eq!(
            props_of(&arena, "div"),
            [("hidden".into(), PROP_BOOL_TRUE, String::new())]
        );
    }

    /// A `<div>` split across two raw nodes with a real element between them.
    fn arena_with_split_raw() -> Arena<Hast> {
        let mut b = ArenaBuilder::<Hast>::new(String::new());
        b.open_node_raw(HastNodeType::Root as u8);

        add_raw_node(&mut b, r#"<div class="n">"#);
        let tag = b.alloc_string("p");
        let el = b.open_node_raw(HastNodeType::Element as u8);
        let data = encode_element_data(tag, &[]);
        b.arena_mut().set_type_data(el, &data);
        let t = b.alloc_string("hi");
        let text = b.add_leaf_raw(HastNodeType::Text as u8);
        b.arena_mut().set_type_data(text, &t.as_bytes());
        b.close_node();
        add_raw_node(&mut b, "</div>");

        b.close_node();
        b.finish()
    }

    #[test]
    fn raw_reparse_resolves_tags_split_across_raw_nodes() {
        let reparsed = raw_to_hast_arena(&arena_with_split_raw());
        assert_eq!(
            hast_arena_to_html(&reparsed).trim_end(),
            r#"<div class="n"><p>hi</p></div>"#
        );
        assert_eq!(
            props_of(&reparsed, "div"),
            [("className".into(), PROP_SPACE_SEP, "n".into())]
        );
    }

    /// Raw tags opening before and closing after an MDX node must resolve
    /// around it, with the MDX node preserved in place.
    #[cfg(feature = "mdx")]
    #[test]
    fn raw_reparse_preserves_mdx_nodes_and_wraps_them_in_surrounding_raw() {
        use crate::shared::MDX_ATTR_EXPRESSION_PROP;

        let mut b = ArenaBuilder::<Hast>::new(String::new());
        b.open_node_raw(HastNodeType::Root as u8);

        // raw "<section>" + <Foo bar={1}>hi</Foo> + raw "</section>"
        let open = b.alloc_string("<section>");
        let leaf = b.add_leaf_raw(HastNodeType::Raw as u8);
        b.arena_mut().set_type_data(leaf, &open.as_bytes());

        let name = b.alloc_string("Foo");
        let attr_name = b.alloc_string("bar");
        let attr_value = b.alloc_string("1");
        let mdx = b.open_node_raw(HastNodeType::MdxJsxElement as u8);
        let data = encode_mdx_jsx_element_data(
            name,
            &[(MDX_ATTR_EXPRESSION_PROP, attr_name, attr_value)],
            true,
        );
        b.arena_mut().set_type_data(mdx, &data);
        let hi = b.alloc_string("hi");
        let text = b.add_leaf_raw(HastNodeType::Text as u8);
        b.arena_mut().set_type_data(text, &hi.as_bytes());
        b.close_node(); // </Foo>

        let close = b.alloc_string("</section>");
        let leaf = b.add_leaf_raw(HastNodeType::Raw as u8);
        b.arena_mut().set_type_data(leaf, &close.as_bytes());

        b.close_node(); // </root>
        let arena = b.finish();

        let reparsed = raw_to_hast_arena(&arena);

        // root > section(element) > Foo(mdx) > "hi"(text)
        let root_children = reparsed.get_children(0);
        assert_eq!(root_children.len(), 1, "single <section> at the root");
        let section = root_children[0];
        assert_eq!(
            reparsed.get_node(section).node_type,
            HastNodeType::Element as u8
        );
        assert_eq!(
            reparsed.get_str(decode_element_tag(reparsed.get_type_data(section))),
            "section"
        );

        let section_children = reparsed.get_children(section);
        assert_eq!(section_children.len(), 1, "<section> wraps the MDX node");
        let foo = section_children[0];
        assert_eq!(
            reparsed.get_node(foo).node_type,
            HastNodeType::MdxJsxElement as u8,
            "the MDX node survived the reparse"
        );

        let foo_data = reparsed.get_type_data(foo);
        assert_eq!(
            reparsed.get_str(decode_mdx_jsx_element_name(foo_data)),
            "Foo"
        );
        assert!(decode_mdx_jsx_explicit(foo_data));
        assert_eq!(decode_mdx_jsx_attr_count(foo_data), 1);
        let (kind, an, av) = decode_mdx_jsx_attr(foo_data, 0);
        assert_eq!(kind, MDX_ATTR_EXPRESSION_PROP);
        assert_eq!(reparsed.get_str(an), "bar");
        assert_eq!(reparsed.get_str(av), "1");

        let foo_children = reparsed.get_children(foo);
        assert_eq!(foo_children.len(), 1);
        let text = foo_children[0];
        assert_eq!(reparsed.get_node(text).node_type, HastNodeType::Text as u8);
        assert_eq!(
            reparsed.get_str(decode_text_data(reparsed.get_type_data(text))),
            "hi"
        );
    }

    /// A raw node nested inside a preserved MDX element is itself reparsed,
    /// not copied through verbatim.
    #[cfg(feature = "mdx")]
    #[test]
    fn raw_reparse_recurses_into_mdx_element_children() {
        let mut b = ArenaBuilder::<Hast>::new(String::new());
        b.open_node_raw(HastNodeType::Root as u8);

        // <Note> containing a single raw node `<em>hi</em>`.
        let name = b.alloc_string("Note");
        let mdx = b.open_node_raw(HastNodeType::MdxJsxElement as u8);
        let data = encode_mdx_jsx_element_data(name, &[], true);
        b.arena_mut().set_type_data(mdx, &data);

        let raw = b.alloc_string("<em>hi</em>");
        let leaf = b.add_leaf_raw(HastNodeType::Raw as u8);
        b.arena_mut().set_type_data(leaf, &raw.as_bytes());

        b.close_node(); // </Note>
        b.close_node(); // </root>
        let arena = b.finish();

        let reparsed = raw_to_hast_arena(&arena);

        // root > Note(mdx) > em(element) > "hi"(text): the nested raw became <em>.
        let note = reparsed.get_children(0)[0];
        assert_eq!(
            reparsed.get_node(note).node_type,
            HastNodeType::MdxJsxElement as u8,
            "the MDX element is preserved"
        );
        let note_children = reparsed.get_children(note);
        assert_eq!(note_children.len(), 1, "nested raw reparsed to one element");
        let em = note_children[0];
        assert_eq!(
            reparsed.get_node(em).node_type,
            HastNodeType::Element as u8,
            "the nested raw node was reparsed, not copied verbatim"
        );
        assert_eq!(
            reparsed.get_str(decode_element_tag(reparsed.get_type_data(em))),
            "em"
        );
        let text = reparsed.get_children(em)[0];
        assert_eq!(
            reparsed.get_str(decode_text_data(reparsed.get_type_data(text))),
            "hi"
        );
    }

    fn add_raw_node(b: &mut ArenaBuilder<Hast>, html: &str) {
        let r = b.alloc_string(html);
        let leaf = b.add_leaf_raw(HastNodeType::Raw as u8);
        b.arena_mut().set_type_data(leaf, &r.as_bytes());
    }

    #[cfg(feature = "mdx")]
    fn add_mdx_foo(b: &mut ArenaBuilder<Hast>) {
        let name = b.alloc_string("Foo");
        let mdx = b.open_node_raw(HastNodeType::MdxJsxElement as u8);
        let data = encode_mdx_jsx_element_data(name, &[], true);
        b.arena_mut().set_type_data(mdx, &data);
        b.close_node();
    }

    /// A stitch-like comment authored in raw HTML must survive as an ordinary
    /// comment, not be swapped for (and duplicate) the preserved MDX node.
    #[cfg(feature = "mdx")]
    #[test]
    fn raw_reparse_ignores_forged_stitch_markers() {
        let mut b = ArenaBuilder::<Hast>::new(String::new());
        b.open_node_raw(HastNodeType::Root as u8);
        add_raw_node(&mut b, "<!--satteri:stitch:0-->");
        add_mdx_foo(&mut b);
        b.close_node();
        let reparsed = raw_to_hast_arena(&b.finish());

        let children = reparsed.get_children(0);
        assert_eq!(children.len(), 2, "comment + MDX node, nothing duplicated");
        assert_eq!(
            reparsed.get_node(children[0]).node_type,
            HastNodeType::Comment as u8
        );
        assert_eq!(
            reparsed.get_str(decode_text_data(reparsed.get_type_data(children[0]))),
            "satteri:stitch:0",
            "the forged comment survives verbatim"
        );
        assert_eq!(
            reparsed.get_node(children[1]).node_type,
            HastNodeType::MdxJsxElement as u8
        );
    }

    /// An MDX node between an unclosed raw `<script>` and its close tag cannot
    /// be preserved (its placeholder is swallowed as script text), but the
    /// marker must not leak into the output.
    #[cfg(feature = "mdx")]
    #[test]
    fn raw_reparse_scrubs_markers_swallowed_by_raw_text_elements() {
        let mut b = ArenaBuilder::<Hast>::new(String::new());
        b.open_node_raw(HastNodeType::Root as u8);
        add_raw_node(&mut b, "<script>alert(1)");
        add_mdx_foo(&mut b);
        add_raw_node(&mut b, "</script>");
        b.close_node();
        let reparsed = raw_to_hast_arena(&b.finish());

        let out = hast_arena_to_html(&reparsed);
        assert!(
            !out.contains("satteri:stitch"),
            "marker text must not leak: {out}"
        );
        assert!(
            out.contains("<script>alert(1)</script>"),
            "script content is restored exactly: {out}"
        );
    }

    /// An MDX node between a raw chunk ending mid-tag and the chunk closing
    /// the tag: the marker becomes attribute junk, which must be dropped.
    #[cfg(feature = "mdx")]
    #[test]
    fn raw_reparse_scrubs_markers_swallowed_into_tags() {
        let mut b = ArenaBuilder::<Hast>::new(String::new());
        b.open_node_raw(HastNodeType::Root as u8);
        add_raw_node(&mut b, "<div ");
        add_mdx_foo(&mut b);
        add_raw_node(&mut b, "class=\"x\">hi</div>");
        b.close_node();
        let reparsed = raw_to_hast_arena(&b.finish());

        let out = hast_arena_to_html(&reparsed);
        assert!(
            !out.contains("satteri:stitch"),
            "marker text must not leak: {out}"
        );
        assert!(
            props_of(&reparsed, "div").is_empty(),
            "the marker-junk attribute is dropped"
        );
    }

    /// An MDX node after an unterminated raw comment: the marker merges into
    /// that comment's contents and must be scrubbed back out.
    #[cfg(feature = "mdx")]
    #[test]
    fn raw_reparse_scrubs_markers_merged_into_unterminated_comments() {
        let mut b = ArenaBuilder::<Hast>::new(String::new());
        b.open_node_raw(HastNodeType::Root as u8);
        add_raw_node(&mut b, "<!--oops ");
        add_mdx_foo(&mut b);
        b.close_node();
        let reparsed = raw_to_hast_arena(&b.finish());

        let out = hast_arena_to_html(&reparsed);
        assert!(
            !out.contains("satteri:stitch"),
            "marker text must not leak: {out}"
        );
        let comment = reparsed.get_children(0)[0];
        assert_eq!(
            reparsed.get_node(comment).node_type,
            HastNodeType::Comment as u8
        );
        assert_eq!(
            reparsed.get_str(decode_text_data(reparsed.get_type_data(comment))),
            "oops "
        );
    }

    #[test]
    fn wrap_arena_single_element() {
        let arena = html_fragment_to_wrap_arena("  <div class=\"x\"><a href=\"#\">#</a></div>\n")
            .expect("single element must parse");
        assert_eq!(arena.get_node(0).node_type, HastNodeType::Element as u8);
        assert_eq!(
            arena.get_str(decode_element_tag(arena.get_type_data(0))),
            "div"
        );
        let children = arena.get_children(0);
        assert_eq!(children.len(), 1);
        assert_eq!(
            arena.get_str(decode_element_tag(arena.get_type_data(children[0]))),
            "a"
        );
    }

    /// Template-context parsing keeps table parts; document parsing would
    /// foster-parent them away.
    #[test]
    fn wrap_arena_accepts_table_parts() {
        let arena = html_fragment_to_wrap_arena("<tr></tr>").expect("tr must parse");
        assert_eq!(
            arena.get_str(decode_element_tag(arena.get_type_data(0))),
            "tr"
        );
    }

    #[test]
    fn wrap_arena_rejects_non_single_element() {
        for html in ["hello", "<i></i><b></b>", "<!--x--><div></div>", "", "   "] {
            let err = html_fragment_to_wrap_arena(html).expect_err(html);
            assert!(err.contains("exactly one element"), "{html}: {err}");
        }
    }

    #[test]
    fn wrap_arena_rejects_void_elements() {
        let err = html_fragment_to_wrap_arena("<img src=\"x.png\">").expect_err("void");
        assert!(err.contains("void element"), "{err}");
    }
}
