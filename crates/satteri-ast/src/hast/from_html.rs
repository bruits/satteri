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
//! Attribute names are stored verbatim as `PROP_STRING`; the renderer maps
//! unknown names straight back to HTML, so they round-trip. Full
//! `property-information` normalisation (className arrays, boolean/number
//! coercion) is intentionally left for a follow-up.
//!
//! `<template>` content is parsed into a detached content document by the tree
//! builder. Standard hast models this as a separate `content` root, which the
//! arena has no field for, so the content is emitted as the template's
//! `children` instead of being dropped. This keeps Sätteri's own round-trip
//! lossless; a third-party `hast-util-to-html` won't re-serialise it.

use std::cell::{Cell, Ref, RefCell};

use html5ever::interface::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::{parse_document, tree_builder::TreeBuilderOpts, Attribute, ParseOpts, QualName};
use satteri_arena::{Arena, ArenaBuilder, Hast, StringRef};

use crate::hast::codec::encode_element_data;
use crate::hast::HastNodeType;
use crate::shared::PROP_STRING;

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

/// Emit the intermediate node `id` (and its subtree) into the HAST builder.
fn emit(nodes: &[Node], id: usize, builder: &mut ArenaBuilder<Hast>) {
    match &nodes[id].data {
        NodeData::Document => {
            for &child in &nodes[id].children {
                emit(nodes, child, builder);
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
            let props: Vec<(StringRef, u8, StringRef)> = attrs
                .iter()
                .map(|attr| {
                    let name_ref = builder.alloc_string(&attr.name.local);
                    let value_ref = builder.alloc_string(&attr.value);
                    (name_ref, PROP_STRING, value_ref)
                })
                .collect();
            let element = builder.open_node_raw(HastNodeType::Element as u8);
            let data = encode_element_data(tag_ref, &props);
            builder.arena_mut().set_type_data(element, &data);
            for &child in &nodes[id].children {
                emit(nodes, child, builder);
            }
            // `<template>` content is parsed into a detached document node
            // (`template_contents`), not the element's own children. HAST models
            // this as a separate `content` root, which Sätteri's arena has no
            // field for, so emit the content as the template's children rather
            // than dropping it. `children` is otherwise empty for templates.
            if let Some(contents) = template_contents {
                for &child in &nodes[*contents].children {
                    emit(nodes, child, builder);
                }
            }
            builder.close_node();
        }
    }
}

/// Parse an HTML document string into a HAST arena.
///
/// Mirrors `hast-util-from-html` in document mode: the result is a `root`
/// whose children are the parsed document (the doctype, if any, and the
/// implied `<html>` subtree).
pub fn html_to_hast_arena(html: &str) -> Arena<Hast> {
    // Match `hast-util-from-html`, which parses with scripting disabled. This
    // makes `<noscript>` contents parse as a normal tree of nodes rather than a
    // single raw-text node, matching the WHATWG "no scripting" default used by
    // tools that transform HTML without executing scripts.
    let opts = ParseOpts {
        tree_builder: TreeBuilderOpts {
            scripting_enabled: false,
            ..TreeBuilderOpts::default()
        },
        ..ParseOpts::default()
    };
    let sink = parse_document(HtmlSink::new(), opts).one(html);
    let nodes = sink.nodes.into_inner();

    let mut builder = ArenaBuilder::<Hast>::new(String::new());
    builder.open_node_raw(HastNodeType::Root as u8);
    for &child in &nodes[0].children {
        emit(&nodes, child, &mut builder);
    }
    builder.close_node();
    builder.finish()
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
}
