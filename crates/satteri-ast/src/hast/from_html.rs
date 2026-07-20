//! Parse an HTML string into a HAST arena. Feature-gated behind `from-html`.
//!
//! html5ever's tree builder needs random-access mutation (foster parenting,
//! reparenting, insert-before-sibling), which the append-only `ArenaBuilder`
//! cannot offer, so parsing goes through a flat, index-addressed `Vec<Node>`
//! that is then emitted into the builder in document order.

use std::cell::{Cell, Ref, RefCell};

use html5ever::interface::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::{
    parse_document, parse_fragment, tree_builder::TreeBuilderOpts, Attribute, LocalName, Namespace,
    ParseOpts, QualName,
};
use satteri_arena::{Arena, ArenaBuilder, Hast, StringRef};
use satteri_property_info::{find_property, PropKind};

use crate::hast::codec::{
    decode_element_prop, decode_element_prop_count, decode_element_tag, decode_text_data,
    encode_element_data,
};
use crate::hast::render::render_node_inner;
use crate::hast::HastNodeType;
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

/// Handles are indices into `HtmlSink::nodes`; the document is always index 0.
struct Node {
    parent: Option<usize>,
    children: Vec<usize>,
    data: NodeData,
}

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
    fn new(prefix: String, count: usize) -> Self {
        StitchRecognizer {
            prefix,
            claimed: RefCell::new(vec![false; count]),
        }
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
            .filter(|(_, &was_claimed)| !was_claimed)
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
        if let NodeOrText::AppendText(text) = &child {
            if let Some(&last) = nodes[parent].children.last() {
                if push_text(&mut nodes, last, text) {
                    return;
                }
            }
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

        match &nodes[id].data {
            NodeData::Document => {
                for &child in nodes[id].children.iter().rev() {
                    stack.push(EmitTask::Emit(child));
                }
            }
            NodeData::Doctype => {
                builder.add_leaf_raw(HastNodeType::Doctype as u8);
            }
            NodeData::Text { contents } => {
                let text = scrub_markers(contents, leaked);
                let text_ref = builder.alloc_string(&text);
                let leaf = builder.add_leaf_raw(HastNodeType::Text as u8);
                builder
                    .arena_mut()
                    .set_type_data(leaf, &text_ref.as_bytes());
            }
            NodeData::Comment { contents } => {
                let text = scrub_markers(contents, leaked);
                let text_ref = builder.alloc_string(&text);
                let leaf = builder.add_leaf_raw(HastNodeType::Comment as u8);
                builder
                    .arena_mut()
                    .set_type_data(leaf, &text_ref.as_bytes());
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
                        let (property, prop_kind) = find_property(&attr.name.local, in_svg);
                        let name_ref = builder.alloc_string(&property);
                        let value = scrub_markers(&attr.value, leaked);
                        let (kind, value_ref) =
                            coerce_value(builder, prop_kind, &attr.name.local, &value);
                        (name_ref, kind, value_ref)
                    })
                    .collect();
                let element = builder.open_node_raw(HastNodeType::Element as u8);
                let data = encode_element_data(tag_ref, &props);
                builder.arena_mut().set_type_data(element, &data);

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
            builder.add_leaf_raw(HastNodeType::Doctype as u8);
        }
        Some(HastNodeType::Text | HastNodeType::Comment | HastNodeType::Raw) if data.len() >= 8 => {
            let value_ref = builder.alloc_string(src.get_str(decode_text_data(data)));
            let leaf = builder.add_leaf_raw(node_type);
            builder
                .arena_mut()
                .set_type_data(leaf, &value_ref.as_bytes());
        }
        Some(HastNodeType::Element) if data.len() >= 16 => {
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

/// Serialise `parent`'s children to HTML — MDX nodes become placeholder
/// comments — reparse that as a fragment, and emit the result into the
/// currently open node. Recurses once per nested MDX level via
/// [`emit_arena_node`]. An MDX node whose placeholder the parser swallowed as
/// text (e.g. inside an unclosed raw `<script>`) is dropped and its marker
/// scrubbed from the output.
fn reparse_children_into(src: &Arena<Hast>, parent: u32, builder: &mut ArenaBuilder<Hast>) {
    let prefix = stitch_prefix();
    let mut html = String::new();
    let mut stitches: Vec<u32> = Vec::new();
    {
        let mut on_mdx = |out: &mut String, node_id: u32| {
            out.push_str("<!--");
            out.push_str(&prefix);
            out.push_str(&stitches.len().to_string());
            out.push_str("-->");
            stitches.push(node_id);
        };
        for &child in src.get_children(parent) {
            render_node_inner(child, src, &mut html, false, false, Some(&mut on_mdx));
        }
    }
    let recognizer = StitchRecognizer::new(prefix, stitches.len());
    let (nodes, roots, leaked) = parse_fragment_nodes(&html, Some(recognizer));
    emit(&nodes, &roots, builder, Some(src), &stitches, &leaked);
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

/// Reparse the raw HTML embedded in a HAST arena into real HAST nodes.
///
/// The whole tree is rendered back to HTML (raw nodes verbatim) and reparsed
/// as one fragment, so a tag opened in one raw node and closed in a later one
/// resolves against the surrounding markup. The result is a fresh `root` with
/// no synthesised `<html>`/`<head>`/`<body>` wrapper.
///
/// MDX nodes have no HTML form; they are carried through as placeholder
/// comments and spliced back afterwards. Positions are not preserved: the
/// tree is rebuilt from serialised HTML.
pub fn raw_to_hast_arena(arena: &Arena<Hast>) -> Arena<Hast> {
    let mut builder = ArenaBuilder::<Hast>::new(String::new());
    builder.open_node_raw(HastNodeType::Root as u8);
    reparse_children_into(arena, 0, &mut builder);
    builder.close_node();
    builder.finish()
}

/// Parse an HTML fragment in a `<template>` context — the most permissive
/// insertion mode, so table parts (`<td>`, `<tr>`, ...) survive outside a
/// table instead of being dropped. Returns the node list, the top-level node
/// indices, and the markers of any swallowed stitches. The fragment algorithm
/// wraps content in a synthesised `<html>` root; the returned roots are that
/// wrapper's children.
fn parse_fragment_nodes(
    html: &str,
    stitch: Option<StitchRecognizer>,
) -> (Vec<Node>, Vec<usize>, Vec<String>) {
    let context = QualName::new(
        None,
        Namespace::from(HTML_NAMESPACE),
        LocalName::from("template"),
    );
    let sink = parse_fragment(
        HtmlSink::new(stitch),
        parse_opts(),
        context,
        Vec::new(),
        false,
    )
    .one(html);
    let leaked = sink
        .stitch
        .map(StitchRecognizer::leaked_markers)
        .unwrap_or_default();
    let nodes = sink.nodes.into_inner();
    let roots = match nodes[0].children.first() {
        Some(&html_root) => nodes[html_root].children.clone(),
        None => Vec::new(),
    };
    (nodes, roots, leaked)
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
        // Count by scanning the flat arena — a recursive walk would itself
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
}
