//! Parse an HTML string into a HAST arena.
//!
//! Feature-gated behind `from-html`. This is the `hast-util-from-html`
//! equivalent: it runs `html5ever`'s spec-compliant tree builder against a
//! minimal in-memory tree (`TreeSink`), then walks that tree in document order
//! and emits it into an append-only `ArenaBuilder<Hast>`.
//!
//! The tree builder needs random-access mutation (foster parenting,
//! reparenting, insert-before-sibling), which the append-only builder cannot
//! offer, so the sink first materialises a flat `Vec<Node>` addressed by index.
//! Attributes are normalised into typed hast properties via the
//! [`satteri_property_info`] crate, so `class` becomes `className: [...]`,
//! `disabled` becomes `true`, `tabindex` becomes a number, and `data-foo-bar`
//! becomes `dataFooBar` — matching `hast-util-from-html`.
//!
//! `<template>` content is parsed into a detached content document by the tree
//! builder. Standard hast models this as a separate `content` root, which the
//! arena has no field for, so the content is emitted as the template's
//! `children` instead of being dropped. This keeps Sätteri's own round-trip
//! lossless; a third-party `hast-util-to-html` won't re-serialise it.

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
use crate::hast::render::{render_node_inner, STITCH_COMMENT_PREFIX};
use crate::hast::HastNodeType;
#[cfg(feature = "mdx")]
use crate::mdast::codec::{
    decode_mdx_jsx_attr, decode_mdx_jsx_attr_count, decode_mdx_jsx_element_name,
    decode_mdx_jsx_explicit, encode_mdx_jsx_element_data,
};
use crate::shared::{PROP_BOOL_TRUE, PROP_COMMA_SEP, PROP_INT, PROP_SPACE_SEP, PROP_STRING};

const HTML_NAMESPACE: &str = "http://www.w3.org/1999/xhtml";
const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";

/// A node in the sink's intermediate tree. Handles are indices into
/// `HtmlSink::nodes`; the document is always index 0.
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
    Element {
        name: QualName,
        attrs: Vec<Attribute>,
        template_contents: Option<usize>,
    },
}

/// A `TreeSink` that builds a flat, index-addressed tree. Interior mutability
/// mirrors the trait (all methods take `&self`); a single `RefCell<Vec<Node>>`
/// stands in for `rcdom`'s per-node `RefCell`s.
struct HtmlSink {
    nodes: RefCell<Vec<Node>>,
    quirks_mode: Cell<QuirksMode>,
}

impl HtmlSink {
    fn new() -> Self {
        HtmlSink {
            nodes: RefCell::new(vec![Node {
                parent: None,
                children: Vec::new(),
                data: NodeData::Document,
            }]),
            quirks_mode: Cell::new(QuirksMode::NoQuirks),
        }
    }
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

/// Find `target`'s parent and its position within that parent's children.
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

/// Append `child` (a parentless node) as the last child of `parent`.
fn append_node(nodes: &mut [Node], parent: usize, child: usize) {
    debug_assert!(
        nodes[child].parent.is_none(),
        "append_node on a node with a parent"
    );
    nodes[child].parent = Some(parent);
    nodes[parent].children.push(child);
}

/// Coalesce `text` into `target` when it is a text node, mirroring the tree
/// builder's expectation that adjacent text is merged.
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

/// A unit of work for the iterative emitter: emit a node from the parsed sink
/// tree (scheduling its subtree), splice a preserved node back in from the
/// source arena (`EmitArena`, used for `hast-util-raw`-style MDX passthrough),
/// or close the element that a previously emitted node opened.
enum EmitTask {
    Emit(usize),
    EmitArena(u32),
    Close,
}

/// Emit `roots` (and their subtrees) into the HAST builder in document order.
///
/// Walks with an explicit work stack rather than recursion: HTML nesting is
/// unbounded (html5ever imposes no depth cap on the tree it builds), so a
/// recursive walk would overflow the native stack — and abort the process —
/// on adversarially deep input like `"<div>".repeat(100_000)`. The explicit
/// stack moves that growth to the heap.
///
/// `src`/`stitches` drive MDX passthrough: when the parsed tree contains a
/// placeholder comment (`<!--satteri:stitch:N-->`, emitted by
/// [`render_node_inner`] for a node with no HTML form), the emitter splices the
/// original subtree — `stitches[N]` in `src` — back in via [`EmitTask::EmitArena`]
/// instead of emitting the comment. `src` is `None` for [`html_to_hast_arena`],
/// which never stitches.
fn emit(
    nodes: &[Node],
    roots: &[usize],
    builder: &mut ArenaBuilder<Hast>,
    src: Option<&Arena<Hast>>,
    stitches: &[u32],
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
                emit_arena_node(src.expect("EmitArena without a source arena"), aid, builder, &mut stack);
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
                let text_ref = builder.alloc_string(contents);
                let leaf = builder.add_leaf_raw(HastNodeType::Text as u8);
                builder
                    .arena_mut()
                    .set_type_data(leaf, &text_ref.as_bytes());
            }
            NodeData::Comment { contents } => {
                // A placeholder comment stands in for a preserved MDX node:
                // splice the original subtree from `src` rather than emitting a
                // comment. `parse_stitch` only matches our own marker, so real
                // comments fall through untouched.
                if let Some(index) = parse_stitch(contents, stitches) {
                    stack.push(EmitTask::EmitArena(stitches[index]));
                    continue;
                }
                let text_ref = builder.alloc_string(contents);
                let leaf = builder.add_leaf_raw(HastNodeType::Comment as u8);
                builder
                    .arena_mut()
                    .set_type_data(leaf, &text_ref.as_bytes());
            }
            // HAST has no processing-instruction node; HTML parsing turns `<?...>`
            // into a comment anyway, so this is effectively unreachable.
            NodeData::ProcessingInstruction => {}
            NodeData::Element {
                name,
                attrs,
                template_contents,
            } => {
                let tag_ref = builder.alloc_string(&name.local);
                // `hast-util-from-html` picks the property schema from the
                // element's namespace, so SVG attributes (`viewBox`, ...) keep
                // their casing while HTML ones normalise.
                let in_svg = &*name.ns == SVG_NAMESPACE;
                let props: Vec<(StringRef, u8, StringRef)> = attrs
                    .iter()
                    .map(|attr| {
                        let (property, prop_kind) = find_property(&attr.name.local, in_svg);
                        let name_ref = builder.alloc_string(&property);
                        let (kind, value_ref) = coerce_value(builder, prop_kind, &attr.value);
                        (name_ref, kind, value_ref)
                    })
                    .collect();
                let element = builder.open_node_raw(HastNodeType::Element as u8);
                let data = encode_element_data(tag_ref, &props);
                builder.arena_mut().set_type_data(element, &data);

                // Emitted in reverse of document order, since the stack is LIFO:
                // Close runs last, then `<template>` content, then the element's
                // own children (which therefore emit first).
                stack.push(EmitTask::Close);
                // `<template>` content is parsed into a detached document node
                // (`template_contents`), not the element's own children. HAST
                // models this as a separate `content` root, which Sätteri's arena
                // has no field for, so emit the content as the template's children
                // rather than dropping it. `children` is otherwise empty for
                // templates.
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

/// If `contents` is one of our stitch placeholders (`satteri:stitch:N`),
/// return the stitch index `N`, but only when it is in range for `stitches`.
/// Any other comment (including a coincidental look-alike with an out-of-range
/// index) returns `None` and is emitted as a normal comment.
fn parse_stitch(contents: &str, stitches: &[u32]) -> Option<usize> {
    let index: usize = contents.strip_prefix(STITCH_COMMENT_PREFIX)?.parse().ok()?;
    (index < stitches.len()).then_some(index)
}

/// Copy the source-arena subtree rooted at `aid` into `builder`, scheduling its
/// children (and a matching `Close`) on `stack`. Used to splice a preserved MDX
/// node back into the reparsed tree. Strings are re-allocated into the builder's
/// pool and the type-data re-encoded, since `src` and the builder have separate
/// string pools.
fn emit_arena_node(
    src: &Arena<Hast>,
    aid: u32,
    builder: &mut ArenaBuilder<Hast>,
    stack: &mut Vec<EmitTask>,
) {
    let node_type = src.get_node(aid).node_type;
    let data = src.get_type_data(aid);

    // Schedule `aid`'s children (reversed, so they pop in document order) after
    // a `Close`. Shared by every container arm below.
    let open_container = |stack: &mut Vec<EmitTask>| {
        stack.push(EmitTask::Close);
        for &child in src.get_children(aid).iter().rev() {
            stack.push(EmitTask::EmitArena(child));
        }
    };

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
            let value = src.get_str(decode_text_data(data)).to_owned();
            let value_ref = builder.alloc_string(&value);
            let leaf = builder.add_leaf_raw(node_type);
            builder.arena_mut().set_type_data(leaf, &value_ref.as_bytes());
        }
        Some(HastNodeType::Element) if data.len() >= 16 => {
            let tag = src.get_str(decode_element_tag(data)).to_owned();
            let props: Vec<(String, u8, String)> = (0..decode_element_prop_count(data))
                .map(|i| {
                    let (name, kind, value) = decode_element_prop(data, i);
                    (src.get_str(name).to_owned(), kind, src.get_str(value).to_owned())
                })
                .collect();
            let tag_ref = builder.alloc_string(&tag);
            let props: Vec<(StringRef, u8, StringRef)> = props
                .iter()
                .map(|(name, kind, value)| {
                    (builder.alloc_string(name), *kind, builder.alloc_string(value))
                })
                .collect();
            let element = builder.open_node_raw(HastNodeType::Element as u8);
            let encoded = encode_element_data(tag_ref, &props);
            builder.arena_mut().set_type_data(element, &encoded);
            open_container(stack);
        }
        #[cfg(feature = "mdx")]
        Some(HastNodeType::MdxJsxElement | HastNodeType::MdxJsxTextElement) if data.len() >= 16 => {
            let name = src.get_str(decode_mdx_jsx_element_name(data)).to_owned();
            let explicit = decode_mdx_jsx_explicit(data);
            let attrs: Vec<(u8, String, String)> = (0..decode_mdx_jsx_attr_count(data))
                .map(|i| {
                    let (kind, name, value) = decode_mdx_jsx_attr(data, i);
                    (kind, src.get_str(name).to_owned(), src.get_str(value).to_owned())
                })
                .collect();
            let name_ref = builder.alloc_string(&name);
            let attrs: Vec<(u8, StringRef, StringRef)> = attrs
                .iter()
                .map(|(kind, name, value)| {
                    (*kind, builder.alloc_string(name), builder.alloc_string(value))
                })
                .collect();
            let element = builder.open_node_raw(node_type);
            let encoded = encode_mdx_jsx_element_data(name_ref, &attrs, explicit);
            builder.arena_mut().set_type_data(element, &encoded);
            // Passthrough recurses into the MDX node's own children, reparsing
            // them so raw HTML nested inside the element is resolved too (the
            // `[<x>]</x>` case) — matching `hast-util-raw`, which runs the
            // passed-through node's children back through the parser rather than
            // copying them verbatim.
            reparse_children_into(src, aid, builder);
            builder.close_node();
        }
        #[cfg(feature = "mdx")]
        Some(HastNodeType::MdxFlowExpression | HastNodeType::MdxTextExpression | HastNodeType::MdxEsm)
            if data.len() >= 8 =>
        {
            let value = src.get_str(decode_text_data(data)).to_owned();
            let value_ref = builder.alloc_string(&value);
            let leaf = builder.add_leaf_raw(node_type);
            builder.arena_mut().set_type_data(leaf, &value_ref.as_bytes());
        }
        // Unknown discriminant, or a known node with a malformed/empty type-data
        // buffer: preserve its children so a wrapper never silently swallows a
        // whole subtree. (MDX arms are compiled out in non-`mdx` builds, where
        // such nodes cannot occur anyway.)
        _ => {
            for &child in src.get_children(aid).iter().rev() {
                stack.push(EmitTask::EmitArena(child));
            }
        }
    }
}

/// Serialise `parent`'s children to HTML — MDX nodes become stitch placeholders
/// — reparse that as a fragment, and emit the result into `builder` at the
/// currently open node.
///
/// Shared by the top-level raw reparse ([`raw_to_hast_arena`], where `parent` is
/// the root) and by the passthrough of a preserved MDX node's own children, so
/// raw HTML nested inside an MDX element is resolved just like top-level raw
/// HTML. Recurses through [`emit`] → [`emit_arena_node`] once per nested MDX
/// level, mirroring `hast-util-raw`'s recursive stitch.
fn reparse_children_into(src: &Arena<Hast>, parent: u32, builder: &mut ArenaBuilder<Hast>) {
    let mut html = String::new();
    let mut stitches: Vec<u32> = Vec::new();
    for &child in src.get_children(parent) {
        render_node_inner(child, src, &mut html, false, false, Some(&mut stitches));
    }
    let (nodes, roots) = parse_fragment_nodes(&html);
    emit(&nodes, &roots, builder, Some(src), &stitches);
}

/// Coerce an attribute string value into a typed hast property, mirroring
/// `hast-util-from-html`. Returns the wire `(kind, value)` pair. The property
/// name and [`PropKind`] come from [`find_property`](satteri_property_info::find_property).
fn coerce_value(builder: &mut ArenaBuilder<Hast>, kind: PropKind, value: &str) -> (u8, StringRef) {
    match kind {
        PropKind::Boolean => (PROP_BOOL_TRUE, StringRef::empty()),
        PropKind::OverloadedBoolean if value.is_empty() => (PROP_BOOL_TRUE, StringRef::empty()),
        PropKind::Number if is_numeric(value) => (PROP_INT, builder.alloc_string(value)),
        PropKind::SpaceSeparated => {
            let joined = value.split_whitespace().collect::<Vec<_>>().join(" ");
            (PROP_SPACE_SEP, builder.alloc_string(&joined))
        }
        PropKind::CommaSeparated => {
            let joined = split_comma(value).join(",");
            (PROP_COMMA_SEP, builder.alloc_string(&joined))
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

fn split_comma(value: &str) -> Vec<&str> {
    value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

/// Whether `hast-util-from-html` would coerce this value to a number: non-empty
/// and parses as a finite number (`Number(value)` is not `NaN`/`±Infinity`).
fn is_numeric(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty() && trimmed.parse::<f64>().is_ok_and(f64::is_finite)
}

/// Shared parse options: scripting disabled, matching `hast-util-from-html`'s
/// default so `<noscript>` contents parse as a normal tree of nodes rather than
/// a single raw-text node (the WHATWG "no scripting" mode used by tools that
/// transform HTML without executing scripts).
fn parse_opts() -> ParseOpts {
    ParseOpts {
        tree_builder: TreeBuilderOpts {
            scripting_enabled: false,
            ..TreeBuilderOpts::default()
        },
        ..ParseOpts::default()
    }
}

/// Parse an HTML document string into a HAST arena.
///
/// Mirrors `hast-util-from-html` in document mode: the result is a `root`
/// whose children are the parsed document (the doctype, if any, and the
/// implied `<html>` subtree).
pub fn html_to_hast_arena(html: &str) -> Arena<Hast> {
    let sink = parse_document(HtmlSink::new(), parse_opts()).one(html);
    let nodes = sink.nodes.into_inner();

    let mut builder = ArenaBuilder::<Hast>::new(String::new());
    builder.open_node_raw(HastNodeType::Root as u8);
    emit(&nodes, &nodes[0].children, &mut builder, None, &[]);
    builder.close_node();
    builder.finish()
}

/// Reparse the raw HTML embedded in a HAST arena into real HAST nodes — the
/// Sätteri equivalent of `rehype-raw`.
///
/// Markdown HTML (blocks and inline) lands in the tree as `raw` nodes holding
/// literal HTML strings. This renders the whole tree back to HTML (raw nodes
/// pass through verbatim, everything else serialises) and reparses it as a
/// single fragment, so a tag opened in one raw node and closed in a later one
/// is resolved against the surrounding markup — exactly the interleaving
/// `rehype-raw` performs. The result is a fresh `root` with no synthesised
/// `<html>`/`<head>`/`<body>` wrapper, matching `rehype-raw`'s output.
///
/// MDX nodes (JSX elements and expressions) have no HTML form. Rather than
/// being dropped by the serialise/reparse round-trip, they are passed through:
/// each is serialised as a placeholder comment and swapped back into its
/// reparsed position afterwards, so raw tags that open before and close after an
/// MDX node still resolve around it. This mirrors `hast-util-raw`'s `passThrough`
/// (which `rehype-raw` uses for MDX), so `mdxToHast(.., { rawHtml: true })`
/// keeps its MDX content instead of destroying it.
///
/// Positions are not preserved: the reparse works from serialised HTML, so the
/// tree is rebuilt from scratch (as it effectively is under `rehype-raw` too).
pub fn raw_to_hast_arena(arena: &Arena<Hast>) -> Arena<Hast> {
    let mut builder = ArenaBuilder::<Hast>::new(String::new());
    builder.open_node_raw(HastNodeType::Root as u8);
    // Reparse the root's children. MDX nodes have no HTML form, so they are
    // serialised as placeholder comments and swapped back into their reparsed
    // positions (see `reparse_children_into`), mirroring `hast-util-raw`'s
    // passthrough — the surrounding raw HTML still resolves around them.
    reparse_children_into(arena, 0, &mut builder);
    builder.close_node();
    builder.finish()
}

/// Parse an HTML fragment in a `<body>` context, returning the flat node list
/// and the indices of the fragment's top-level nodes. html5ever's fragment
/// algorithm appends a synthesised `<html>` root to the document (node 0); the
/// real fragment content are that root's children, so callers get the body-level
/// nodes without any wrapper.
fn parse_fragment_nodes(html: &str) -> (Vec<Node>, Vec<usize>) {
    let context = QualName::new(
        None,
        Namespace::from(HTML_NAMESPACE),
        LocalName::from("body"),
    );
    let sink = parse_fragment(HtmlSink::new(), parse_opts(), context, Vec::new(), false).one(html);
    let nodes = sink.nodes.into_inner();
    let roots = match nodes[0].children.first() {
        Some(&html_root) => nodes[html_root].children.clone(),
        None => Vec::new(),
    };
    (nodes, roots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hast::codec::decode_element_tag;
    use crate::hast::hast_arena_to_html;

    fn render(html: &str) -> String {
        // The HTML renderer appends a trailing newline; trim it so the
        // expectations below stay focused on structure.
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
        // The stray <b> is foster-parented out of the table, and <tr> gets an
        // implied <tbody>. Exercises append_before_sibling.
        let out = render("<table><b>x</b><tr><td>y</td></tr></table>");
        assert!(out.contains("<b>x</b><table>"), "foster parenting: {out}");
        assert!(
            out.contains("<tbody><tr><td>y</td></tr></tbody>"),
            "implied tbody: {out}"
        );
    }

    #[test]
    fn handles_misnested_tags_via_adoption_agency() {
        // Canonical adoption-agency case; exercises reparent_children.
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
        // `<template>` children are parsed into a detached content document by
        // the tree builder. Emitting them as the element's children keeps the
        // content instead of dropping it.
        assert_eq!(
            render("<template><p>hi</p></template>"),
            "<html><head><template><p>hi</p></template></head><body></body></html>"
        );
        // Bare text content is preserved too.
        assert_eq!(
            render("<template>foo</template>"),
            "<html><head><template>foo</template></head><body></body></html>"
        );
    }

    #[test]
    fn parses_noscript_content_as_markup_with_scripting_disabled() {
        // `hast-util-from-html` parses with scripting disabled, so `<noscript>`
        // contents are a normal tree of nodes rather than a single raw-text
        // node (html5lib tree-construction `noscript01.dat`).
        let out = render("<head><noscript><link><!--c--></noscript>");
        assert!(
            out.contains("<noscript><link><!--c--></noscript>"),
            "noscript parsed as markup: {out}"
        );
    }

    #[test]
    fn deeply_nested_input_does_not_overflow_the_stack() {
        // html5ever imposes no depth cap, so untrusted HTML can build an
        // arbitrarily deep tree. The emitter must walk it iteratively; a
        // recursive walk would overflow the native stack and abort the process.
        // Count elements by scanning the flat arena — a recursive walk here
        // (like the `tags` helper) would itself overflow and defeat the test.
        // `<span>` nests without triggering html5ever's O(n^2) scope checks
        // (unlike `<div>`, which re-scans the open-element stack per token), so
        // the parse stays linear while still building a very deep tree.
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
        // `download` with a value stays a string; a non-numeric `width` too.
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

    /// Build a small HAST arena that splits a `<div>` across two `raw` nodes with
    /// a real element between them — the case `rehype-raw` exists to resolve.
    fn arena_with_split_raw() -> Arena<Hast> {
        let mut b = ArenaBuilder::<Hast>::new(String::new());
        b.open_node_raw(HastNodeType::Root as u8);

        fn add_raw(b: &mut ArenaBuilder<Hast>, html: &str) {
            let r = b.alloc_string(html);
            let leaf = b.add_leaf_raw(HastNodeType::Raw as u8);
            b.arena_mut().set_type_data(leaf, &r.as_bytes());
        }

        add_raw(&mut b, r#"<div class="n">"#);
        let tag = b.alloc_string("p");
        let el = b.open_node_raw(HastNodeType::Element as u8);
        let data = encode_element_data(tag, &[]);
        b.arena_mut().set_type_data(el, &data);
        let t = b.alloc_string("hi");
        let text = b.add_leaf_raw(HastNodeType::Text as u8);
        b.arena_mut().set_type_data(text, &t.as_bytes());
        b.close_node();
        add_raw(&mut b, "</div>");

        b.close_node();
        b.finish()
    }

    #[test]
    fn raw_reparse_resolves_tags_split_across_raw_nodes() {
        let reparsed = raw_to_hast_arena(&arena_with_split_raw());
        // The `<div>` opened in one raw node now wraps the `<p>` and is closed by
        // the second raw node — with no synthesised <html>/<head>/<body> wrapper.
        assert_eq!(
            hast_arena_to_html(&reparsed).trim_end(),
            r#"<div class="n"><p>hi</p></div>"#
        );
        // And the reparsed `<div>` carries a normalised `className`.
        assert_eq!(
            props_of(&reparsed, "div"),
            [("className".into(), PROP_SPACE_SEP, "n".into())]
        );
    }

    /// `rehype-raw`-style passthrough: an MDX node has no HTML form, so the raw
    /// reparse must preserve it — including resolving raw tags that open before
    /// and close after it (the `[<x>]</x>` case from `hast-util-raw`). Without
    /// passthrough the whole `<section>` subtree would be destroyed.
    #[cfg(feature = "mdx")]
    #[test]
    fn raw_reparse_preserves_mdx_nodes_and_wraps_them_in_surrounding_raw() {
        use crate::shared::MDX_ATTR_EXPRESSION_PROP;

        let mut b = ArenaBuilder::<Hast>::new(String::new());
        b.open_node_raw(HastNodeType::Root as u8);

        // A `<section>` opened in one raw node and closed in a later one, with a
        // preserved `<Foo bar={1}>hi</Foo>` MDX element parked between them.
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
        assert_eq!(reparsed.get_node(section).node_type, HastNodeType::Element as u8);
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
        assert_eq!(reparsed.get_str(decode_mdx_jsx_element_name(foo_data)), "Foo");
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

    /// Passthrough recurses: a `raw` node nested *inside* a preserved MDX element
    /// is itself reparsed into real nodes (matching `hast-util-raw`), not copied
    /// through verbatim.
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
}
