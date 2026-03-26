//! Convert an MDAST arena directly into a `hast::Node` tree.

use crate::hast;
use crate::oxc_utils::inter_element_whitespace;
use mdast_arena::codec::{
    ColumnAlign, decode_code_data, decode_definition_data, decode_expression_data,
    decode_footnote_definition_data, decode_heading_data, decode_image_data, decode_link_data,
    decode_list_data, decode_list_item_data, decode_math_data, decode_mdx_jsx_element_data,
    decode_reference_data, decode_string_ref_data, decode_table_data,
};
use mdast_arena::mdx_types::{
    AttributeContent, AttributeValue, AttributeValueExpression, MdxJsxAttribute,
    MdxJsxExpressionAttribute, Point, Position, sanitize_uri as sanitize,
};
use mdast_arena::{NodeType, ReadMdast};
use rustc_hash::FxHashMap;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Convert MDAST (node 0 = Root) to a `hast::Node`.
/// If `syntax_highlight_theme` is Some, code blocks are highlighted with syntect.
pub fn mdast_to_hast(arena: &dyn ReadMdast, syntax_highlight_theme: Option<&str>) -> hast::Node {
    let mut ctx = Context::new(arena, syntax_highlight_theme);

    // Pre-pass: collect definitions and footnote definitions.
    visit_all(arena, 0, &mut |node_id| {
        let raw = arena.get_node(node_id);
        let nt = NodeType::from_u8(raw.node_type).unwrap_or(NodeType::Root);
        match nt {
            NodeType::Definition => {
                let data = arena.get_type_data(node_id);
                if !data.is_empty() {
                    let d = decode_definition_data(data);
                    let identifier = arena.get_str(d.identifier).to_string();
                    let url = arena.get_str(d.url).to_string();
                    let title = if d.title.len > 0 {
                        Some(arena.get_str(d.title).to_string())
                    } else {
                        None
                    };
                    ctx.definitions.insert(identifier, (url, title));
                }
            }
            NodeType::FootnoteDefinition => {
                let data = arena.get_type_data(node_id);
                let identifier = if data.is_empty() {
                    String::new()
                } else {
                    let d = decode_footnote_definition_data(data);
                    arena.get_str(d.identifier).to_string()
                };
                ctx.footnote_defs.insert(identifier, node_id);
            }
            _ => {}
        }
    });

    // Main conversion.
    let result = one(&mut ctx, 0, None);

    if ctx.footnote_calls.is_empty()
        && let NodeResult::Node(node) = result
    {
        return node;
    }

    // Build root and append footnote section if needed.
    let mut root = hast::Root {
        children: vec![],
        position: None,
    };

    match result {
        NodeResult::Fragment(children) => root.children = children,
        NodeResult::Node(node) => {
            if let hast::Node::Root(existing) = node {
                root = existing;
            } else {
                root.children.push(node);
            }
        }
        NodeResult::None => {}
    }

    if !ctx.footnote_calls.is_empty() {
        let mut items = vec![];

        let calls: Vec<(String, usize)> = ctx.footnote_calls.clone();
        let mut index = 0;
        while index < calls.len() {
            let (id, count) = &calls[index];
            let safe_id = sanitize(&id.to_lowercase());

            // Convert the footnote definition children.
            let def_node_id = ctx.footnote_defs.get(id.as_str()).copied();
            let content: Vec<hast::Node> = if let Some(def_id) = def_node_id {
                all_children(&mut ctx, def_id)
            } else {
                vec![]
            };

            let mut content = content;
            let mut reference_index = 0;
            let mut backreferences = vec![];

            while reference_index < *count {
                let mut backref_children = vec![hast::Node::Text(hast::Text {
                    value: "↩".into(),
                    position: None,
                })];

                if reference_index != 0 {
                    backreferences.push(hast::Node::Text(hast::Text {
                        value: " ".into(),
                        position: None,
                    }));

                    backref_children.push(hast::Node::Element(hast::Element {
                        tag_name: "sup".into(),
                        properties: vec![],
                        children: vec![hast::Node::Text(hast::Text {
                            value: (reference_index + 1).to_string(),
                            position: None,
                        })],
                        position: None,
                    }));
                }

                backreferences.push(hast::Node::Element(hast::Element {
                    tag_name: "a".into(),
                    properties: vec![
                        (
                            "href".into(),
                            hast::PropertyValue::String(format!(
                                "#fnref-{}{}",
                                safe_id,
                                if reference_index == 0 {
                                    String::new()
                                } else {
                                    format!("-{}", reference_index + 1)
                                }
                            )),
                        ),
                        (
                            "dataFootnoteBackref".into(),
                            hast::PropertyValue::Boolean(true),
                        ),
                        (
                            "ariaLabel".into(),
                            hast::PropertyValue::String("Back to content".into()),
                        ),
                        (
                            "class".into(),
                            hast::PropertyValue::SpaceSeparated(vec![
                                "data-footnote-backref".into(),
                            ]),
                        ),
                    ],
                    children: backref_children,
                    position: None,
                }));

                reference_index += 1;
            }

            let mut backreference_opt = Some(backreferences);

            if let Some(hast::Node::Element(tail_element)) = content.last_mut()
                && tail_element.tag_name == "p"
            {
                if let Some(hast::Node::Text(text)) = tail_element.children.last_mut() {
                    text.value.push(' ');
                } else {
                    tail_element.children.push(hast::Node::Text(hast::Text {
                        value: " ".into(),
                        position: None,
                    }));
                }

                if let Some(mut backreference) = backreference_opt {
                    backreference_opt = None;
                    tail_element.children.append(&mut backreference);
                }
            }

            if let Some(mut backreference) = backreference_opt {
                content.append(&mut backreference);
            }

            items.push(hast::Node::Element(hast::Element {
                tag_name: "li".into(),
                properties: vec![(
                    "id".into(),
                    hast::PropertyValue::String(format!("#fn-{safe_id}")),
                )],
                children: wrap(content, true),
                position: None,
            }));
            index += 1;
        }

        root.children.push(hast::Node::Text(hast::Text {
            value: "\n".into(),
            position: None,
        }));
        root.children.push(hast::Node::Element(hast::Element {
            tag_name: "section".into(),
            properties: vec![
                ("dataFootnotes".into(), hast::PropertyValue::Boolean(true)),
                (
                    "class".into(),
                    hast::PropertyValue::SpaceSeparated(vec!["footnotes".into()]),
                ),
            ],
            children: vec![
                hast::Node::Element(hast::Element {
                    tag_name: "h2".into(),
                    properties: vec![
                        (
                            "id".into(),
                            hast::PropertyValue::String("footnote-label".into()),
                        ),
                        (
                            "class".into(),
                            hast::PropertyValue::SpaceSeparated(vec!["sr-only".into()]),
                        ),
                    ],
                    children: vec![hast::Node::Text(hast::Text {
                        value: "Footnotes".into(),
                        position: None,
                    })],
                    position: None,
                }),
                hast::Node::Text(hast::Text {
                    value: "\n".into(),
                    position: None,
                }),
                hast::Node::Element(hast::Element {
                    tag_name: "ol".into(),
                    properties: vec![],
                    children: wrap(items, true),
                    position: None,
                }),
                hast::Node::Text(hast::Text {
                    value: "\n".into(),
                    position: None,
                }),
            ],
            position: None,
        }));
        root.children.push(hast::Node::Text(hast::Text {
            value: "\n".into(),
            position: None,
        }));
    }

    hast::Node::Root(root)
}

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

struct Context<'a> {
    arena: &'a dyn ReadMdast,
    /// identifier (lowercase) → (url, title)
    definitions: FxHashMap<String, (String, Option<String>)>,
    /// footnote call list: (identifier, count), in encounter order.
    footnote_calls: Vec<(String, usize)>,
    /// identifier → arena `node_id` of the `FootnoteDefinition`.
    footnote_defs: FxHashMap<String, u32>,
    /// Syntect theme name for syntax highlighting (None = no highlighting).
    syntax_highlight_theme: Option<String>,
}

impl<'a> Context<'a> {
    fn new(arena: &'a dyn ReadMdast, syntax_highlight_theme: Option<&str>) -> Self {
        Context {
            arena,
            definitions: FxHashMap::default(),
            footnote_calls: Vec::new(),
            footnote_defs: FxHashMap::default(),
            syntax_highlight_theme: syntax_highlight_theme.map(String::from),
        }
    }
}

// ---------------------------------------------------------------------------
// Result enum
// ---------------------------------------------------------------------------

enum NodeResult {
    Fragment(Vec<hast::Node>),
    Node(hast::Node),
    None,
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

#[allow(clippy::unnecessary_wraps)]
fn to_position(node: &mdast_arena::ArenaNode) -> Option<Position> {
    Some(Position {
        start: Point {
            line: node.start_line as usize,
            column: node.start_column as usize,
            offset: node.start_offset as usize,
        },
        end: Point {
            line: node.end_line as usize,
            column: node.end_column as usize,
            offset: node.end_offset as usize,
        },
    })
}

/// Recursively visit every node in document order.
fn visit_all(arena: &dyn ReadMdast, node_id: u32, visitor: &mut impl FnMut(u32)) {
    visitor(node_id);
    for &child_id in arena.get_children(node_id) {
        visit_all(arena, child_id, visitor);
    }
}

/// Convert one arena node to hast.
fn one(ctx: &mut Context<'_>, node_id: u32, parent_id: Option<u32>) -> NodeResult {
    let raw = ctx.arena.get_node(node_id);
    let nt = NodeType::from_u8(raw.node_type).unwrap_or(NodeType::Root);
    let position = to_position(raw);

    match nt {
        NodeType::Root => transform_root(ctx, node_id, position),
        NodeType::Paragraph => transform_paragraph(ctx, node_id, position),
        NodeType::Heading => transform_heading(ctx, node_id, position),
        NodeType::ThematicBreak => transform_thematic_break(position),
        NodeType::Blockquote => transform_blockquote(ctx, node_id, position),
        NodeType::List => transform_list(ctx, node_id, position),
        NodeType::ListItem => transform_list_item(ctx, node_id, parent_id, position),
        // Raw HTML ignored; Definition collected in pre-pass; Frontmatter is metadata-only.
        NodeType::Html | NodeType::Definition | NodeType::Yaml | NodeType::Toml => NodeResult::None,
        NodeType::Code => transform_code(ctx, node_id, position),
        NodeType::Text => transform_text(ctx, node_id, position),
        NodeType::Emphasis => transform_emphasis(ctx, node_id, position),
        NodeType::Strong => transform_strong(ctx, node_id, position),
        NodeType::Delete => transform_delete(ctx, node_id, position),
        NodeType::InlineCode => transform_inline_code(ctx, node_id, position),
        NodeType::Break => transform_break(position),
        NodeType::Link => transform_link(ctx, node_id, position),
        NodeType::Image => transform_image(ctx, node_id, position),
        NodeType::LinkReference => transform_link_reference(ctx, node_id, position),
        NodeType::ImageReference => transform_image_reference(ctx, node_id, position),
        NodeType::FootnoteDefinition => transform_footnote_definition(ctx, node_id),
        NodeType::FootnoteReference => transform_footnote_reference(ctx, node_id, position),
        NodeType::Table => transform_table(ctx, node_id, position),
        NodeType::TableRow => {
            // Only reached when a TableRow is a standalone node (not inside Table).
            transform_table_row(ctx, node_id, false, &[], position)
        }
        NodeType::TableCell => {
            // Only reached standalone.
            transform_table_cell(ctx, node_id, false, ColumnAlign::None, position)
        }
        NodeType::MdxJsxFlowElement => transform_mdx_jsx_flow_element(ctx, node_id, position),
        NodeType::MdxJsxTextElement => transform_mdx_jsx_text_element(ctx, node_id, position),
        NodeType::MdxFlowExpression | NodeType::MdxTextExpression => {
            transform_mdx_expression(ctx, node_id, position)
        }
        NodeType::MdxjsEsm => transform_mdxjs_esm(ctx, node_id, position),
        NodeType::Math => transform_math(ctx, node_id, position),
        NodeType::InlineMath => transform_inline_math(ctx, node_id, position),
    }
}

/// Convert all children of `parent_id`.
fn all_children(ctx: &mut Context<'_>, parent_id: u32) -> Vec<hast::Node> {
    let child_ids: Vec<u32> = ctx.arena.get_children(parent_id).to_vec();
    let mut nodes = vec![];
    for child_id in child_ids {
        let result = one(ctx, child_id, Some(parent_id));
        append_result(&mut nodes, result);
    }
    nodes
}

fn append_result(list: &mut Vec<hast::Node>, result: NodeResult) {
    match result {
        NodeResult::Fragment(mut fragment) => list.append(&mut fragment),
        NodeResult::Node(node) => list.push(node),
        NodeResult::None => {}
    }
}

/// Wrap nodes with `\n` text nodes between them. If `loose`, also adds `\n`
/// at start and end (when non-empty).
fn wrap(mut nodes: Vec<hast::Node>, loose: bool) -> Vec<hast::Node> {
    let mut result = vec![];
    let was_empty = nodes.is_empty();
    let mut head = true;

    nodes.reverse();

    if loose {
        result.push(hast::Node::Text(hast::Text {
            value: "\n".into(),
            position: None,
        }));
    }

    while let Some(item) = nodes.pop() {
        if !head {
            result.push(hast::Node::Text(hast::Text {
                value: "\n".into(),
                position: None,
            }));
        }
        head = false;
        result.push(item);
    }

    if loose && !was_empty {
        result.push(hast::Node::Text(hast::Text {
            value: "\n".into(),
            position: None,
        }));
    }

    result
}

/// Replace CR, LF, CRLF with spaces (for inline code / inline math).
fn replace_eols_with_spaces(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0;
    let mut start = 0;

    while index < bytes.len() {
        let byte = bytes[index];

        if byte == b'\r' || byte == b'\n' {
            result.push_str(&value[start..index]);
            result.push(' ');

            if index + 1 < bytes.len() && byte == b'\r' && bytes[index + 1] == b'\n' {
                index += 1;
            }

            start = index + 1;
        }

        index += 1;
    }

    result.push_str(&value[start..]);
    result
}

// ---------------------------------------------------------------------------
// List-loose helpers
// ---------------------------------------------------------------------------

fn list_loose(ctx: &Context<'_>, list_node_id: u32) -> bool {
    let raw = ctx.arena.get_node(list_node_id);
    let nt = NodeType::from_u8(raw.node_type).unwrap_or(NodeType::Root);
    if nt != NodeType::List {
        return false;
    }

    let data = ctx.arena.get_type_data(list_node_id);
    if !data.is_empty() {
        let d = decode_list_data(data);
        if d.spread {
            return true;
        }
    }

    for &child_id in ctx.arena.get_children(list_node_id) {
        if list_item_loose(ctx, child_id) {
            return true;
        }
    }

    false
}

fn list_item_loose(ctx: &Context<'_>, node_id: u32) -> bool {
    let raw = ctx.arena.get_node(node_id);
    let nt = NodeType::from_u8(raw.node_type).unwrap_or(NodeType::Root);
    if nt != NodeType::ListItem {
        return false;
    }
    let data = ctx.arena.get_type_data(node_id);
    if data.is_empty() {
        return false;
    }
    let d = decode_list_item_data(data);
    d.spread
}

// ---------------------------------------------------------------------------
// Node transforms
// ---------------------------------------------------------------------------

fn transform_root(ctx: &mut Context<'_>, node_id: u32, position: Option<Position>) -> NodeResult {
    let children = all_children(ctx, node_id);
    NodeResult::Node(hast::Node::Root(hast::Root {
        children: wrap(children, false),
        position,
    }))
}

fn transform_paragraph(
    ctx: &mut Context<'_>,
    node_id: u32,
    position: Option<Position>,
) -> NodeResult {
    let children = all_children(ctx, node_id);
    let mut all_mdx = true;
    let mut one_or_more = false;
    let mut index = 0;

    while index < children.len() {
        match &children[index] {
            hast::Node::MdxJsxElement(_)
            | hast::Node::MdxJsxTextElement(_)
            | hast::Node::MdxExpression(_) => {
                one_or_more = true;
                index += 1;
                continue;
            }
            hast::Node::Text(node) => {
                if inter_element_whitespace(&node.value) {
                    index += 1;
                    continue;
                }
            }
            _ => {}
        }

        all_mdx = false;
        break;
    }

    if all_mdx && one_or_more {
        NodeResult::Fragment(children)
    } else {
        NodeResult::Node(hast::Node::Element(hast::Element {
            tag_name: "p".into(),
            properties: vec![],
            children,
            position,
        }))
    }
}

fn transform_heading(
    ctx: &mut Context<'_>,
    node_id: u32,
    position: Option<Position>,
) -> NodeResult {
    let data = ctx.arena.get_type_data(node_id);
    let depth = if data.is_empty() {
        1
    } else {
        decode_heading_data(data).depth
    };
    let children = all_children(ctx, node_id);
    NodeResult::Node(hast::Node::Element(hast::Element {
        tag_name: format!("h{depth}"),
        properties: vec![],
        children,
        position,
    }))
}

fn transform_thematic_break(position: Option<Position>) -> NodeResult {
    NodeResult::Node(hast::Node::Element(hast::Element {
        tag_name: "hr".into(),
        properties: vec![],
        children: vec![],
        position,
    }))
}

fn transform_blockquote(
    ctx: &mut Context<'_>,
    node_id: u32,
    position: Option<Position>,
) -> NodeResult {
    let children = all_children(ctx, node_id);
    NodeResult::Node(hast::Node::Element(hast::Element {
        tag_name: "blockquote".into(),
        properties: vec![],
        children: wrap(children, true),
        position,
    }))
}

fn transform_list(ctx: &mut Context<'_>, node_id: u32, position: Option<Position>) -> NodeResult {
    let data = ctx.arena.get_type_data(node_id);
    let (ordered, start) = if data.is_empty() {
        (false, None::<u32>)
    } else {
        let d = decode_list_data(data);
        let start = if d.ordered { Some(d.start) } else { None };
        (d.ordered, start)
    };

    // Check for task list items.
    let mut contains_task_list = false;
    for &child_id in ctx.arena.get_children(node_id) {
        let raw = ctx.arena.get_node(child_id);
        let nt = NodeType::from_u8(raw.node_type).unwrap_or(NodeType::Root);
        if nt == NodeType::ListItem {
            let item_data = ctx.arena.get_type_data(child_id);
            if !item_data.is_empty() {
                let d = decode_list_item_data(item_data);
                if d.checked != 2 {
                    contains_task_list = true;
                    break;
                }
            }
        }
    }

    let mut properties = vec![];

    if let Some(s) = start
        && ordered
        && s != 1
    {
        properties.push(("start".into(), hast::PropertyValue::String(s.to_string())));
    }

    if contains_task_list {
        properties.push((
            "class".into(),
            hast::PropertyValue::SpaceSeparated(vec!["contains-task-list".into()]),
        ));
    }

    let children = all_children(ctx, node_id);
    NodeResult::Node(hast::Node::Element(hast::Element {
        tag_name: if ordered { "ol".into() } else { "ul".into() },
        properties,
        children: wrap(children, true),
        position,
    }))
}

fn transform_list_item(
    ctx: &mut Context<'_>,
    node_id: u32,
    parent_id: Option<u32>,
    position: Option<Position>,
) -> NodeResult {
    let data = ctx.arena.get_type_data(node_id);
    let (checked, _spread) = if data.is_empty() {
        (None, false)
    } else {
        let d = decode_list_item_data(data);
        let checked = match d.checked {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        };
        (checked, d.spread)
    };

    let mut children = all_children(ctx, node_id);
    let mut loose = list_item_loose(ctx, node_id);
    if let Some(pid) = parent_id {
        let praw = ctx.arena.get_node(pid);
        let pnt = NodeType::from_u8(praw.node_type).unwrap_or(NodeType::Root);
        if pnt == NodeType::List {
            loose = list_loose(ctx, pid);
        }
    }

    let mut properties = vec![];

    if let Some(checked_val) = checked {
        properties.push((
            "class".into(),
            hast::PropertyValue::SpaceSeparated(vec!["task-list-item".into()]),
        ));

        let mut input = Some(hast::Node::Element(hast::Element {
            tag_name: "input".into(),
            properties: vec![
                (
                    "type".into(),
                    hast::PropertyValue::String("checkbox".into()),
                ),
                ("checked".into(), hast::PropertyValue::Boolean(checked_val)),
                ("disabled".into(), hast::PropertyValue::Boolean(true)),
            ],
            children: vec![],
            position: None,
        }));

        if let Some(hast::Node::Element(x)) = children.first_mut()
            && x.tag_name == "p"
        {
            if !x.children.is_empty() {
                x.children.insert(
                    0,
                    hast::Node::Text(hast::Text {
                        value: " ".into(),
                        position: None,
                    }),
                );
            }
            x.children.insert(0, input.take().unwrap());
        }

        if let Some(input) = input {
            children.insert(
                0,
                hast::Node::Element(hast::Element {
                    tag_name: "p".into(),
                    properties: vec![],
                    children: vec![input],
                    position: None,
                }),
            );
        }
    }

    // Strip trailing whitespace-only text nodes (artifact of parser not wrapping
    // list item content in paragraphs inside JSX elements).
    while let Some(last) = children.last() {
        if let hast::Node::Text(t) = last {
            if inter_element_whitespace(&t.value) {
                children.pop();
                continue;
            }
        }
        break;
    }

    children.reverse();
    let mut result = vec![];
    let mut head = true;
    let empty = children.is_empty();
    let mut tail_p = false;

    while let Some(child) = children.pop() {
        let mut is_p = false;
        if let hast::Node::Element(el) = &child
            && el.tag_name == "p"
        {
            is_p = true;
        }

        // For tight list items whose text content isn't wrapped in <p> (e.g. inside
        // JSX elements), treat the first inline child the same as an unwrapped
        // paragraph: skip the leading \n.
        let is_inline = matches!(&child, hast::Node::Text(_));
        let treat_as_p = is_p || (head && !loose && is_inline);

        if loose || !head || !treat_as_p {
            result.push(hast::Node::Text(hast::Text {
                value: "\n".into(),
                position: None,
            }));
        }

        if is_p && !loose {
            if let hast::Node::Element(mut el) = child {
                result.append(&mut el.children);
            }
        } else {
            result.push(child);
        }

        head = false;
        tail_p = treat_as_p;
    }

    if !empty && (loose || !tail_p) {
        result.push(hast::Node::Text(hast::Text {
            value: "\n".into(),
            position: None,
        }));
    }

    NodeResult::Node(hast::Node::Element(hast::Element {
        tag_name: "li".into(),
        properties,
        children: result,
        position,
    }))
}

fn transform_code(ctx: &mut Context<'_>, node_id: u32, position: Option<Position>) -> NodeResult {
    let data = ctx.arena.get_type_data(node_id);
    let (value, lang) = if data.is_empty() {
        (String::new(), None)
    } else {
        let d = decode_code_data(data);
        let value = if d.value.len > 0 {
            ctx.arena.get_str(d.value).to_string()
        } else {
            String::new()
        };
        let lang = if d.lang.len > 0 {
            Some(ctx.arena.get_str(d.lang).to_string())
        } else {
            None
        };
        (value, lang)
    };

    // Try syntect highlighting if a theme is configured
    #[cfg(feature = "syntect")]
    if let Some(theme_name) = &ctx.syntax_highlight_theme {
        if let Some(ref lang_str) = lang {
            if let Some(html) = highlight_with_syntect(&value, lang_str, theme_name) {
                // Emit as a Fragment with set:html so the highlighted HTML passes through
                return NodeResult::Node(hast::Node::MdxJsxElement(hast::MdxJsxElement {
                    name: Some("Fragment".into()),
                    attributes: vec![AttributeContent::Property(MdxJsxAttribute {
                        name: "set:html".into(),
                        value: Some(AttributeValue::Expression(
                            AttributeValueExpression {
                                value: js_string_literal(&html),
                                stops: vec![],
                            },
                        )),
                    })],
                    children: vec![],
                    position,
                }));
            }
        }
    }

    let mut code_value = value;
    if !code_value.ends_with('\n') {
        code_value.push('\n');
    }

    let mut properties = vec![];
    if let Some(lang) = lang {
        properties.push((
            "class".into(),
            hast::PropertyValue::SpaceSeparated(vec![format!("language-{lang}")]),
        ));
    }

    NodeResult::Node(hast::Node::Element(hast::Element {
        tag_name: "pre".into(),
        properties: vec![],
        children: vec![hast::Node::Element(hast::Element {
            tag_name: "code".into(),
            properties,
            children: vec![hast::Node::Text(hast::Text {
                value: code_value,
                position: None,
            })],
            position: position.clone(),
        })],
        position,
    }))
}

/// Escape a string for embedding as a JS string literal (with double quotes).
#[cfg(feature = "syntect")]
fn js_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

#[cfg(feature = "syntect")]
fn highlight_with_syntect(code: &str, lang: &str, theme_name: &str) -> Option<String> {
    use std::sync::OnceLock;
    use syntect::highlighting::ThemeSet;
    use syntect::html::highlighted_html_for_string;
    use syntect::parsing::SyntaxSet;

    static SS: OnceLock<SyntaxSet> = OnceLock::new();
    static TS: OnceLock<ThemeSet> = OnceLock::new();

    let ss = SS.get_or_init(SyntaxSet::load_defaults_newlines);
    let ts = TS.get_or_init(ThemeSet::load_defaults);

    let theme = ts.themes.get(theme_name)?;
    let syntax = ss.find_syntax_by_token(lang)
        .unwrap_or_else(|| ss.find_syntax_plain_text());

    highlighted_html_for_string(code, ss, syntax, theme).ok()
}

fn transform_text(ctx: &mut Context<'_>, node_id: u32, position: Option<Position>) -> NodeResult {
    let data = ctx.arena.get_type_data(node_id);
    let value = if data.is_empty() {
        String::new()
    } else {
        ctx.arena.get_str(decode_string_ref_data(data)).to_string()
    };
    NodeResult::Node(hast::Node::Text(hast::Text { value, position }))
}

fn transform_emphasis(
    ctx: &mut Context<'_>,
    node_id: u32,
    position: Option<Position>,
) -> NodeResult {
    let children = all_children(ctx, node_id);
    NodeResult::Node(hast::Node::Element(hast::Element {
        tag_name: "em".into(),
        properties: vec![],
        children,
        position,
    }))
}

fn transform_strong(ctx: &mut Context<'_>, node_id: u32, position: Option<Position>) -> NodeResult {
    let children = all_children(ctx, node_id);
    NodeResult::Node(hast::Node::Element(hast::Element {
        tag_name: "strong".into(),
        properties: vec![],
        children,
        position,
    }))
}

fn transform_delete(ctx: &mut Context<'_>, node_id: u32, position: Option<Position>) -> NodeResult {
    let children = all_children(ctx, node_id);
    NodeResult::Node(hast::Node::Element(hast::Element {
        tag_name: "del".into(),
        properties: vec![],
        children,
        position,
    }))
}

fn transform_inline_code(
    ctx: &mut Context<'_>,
    node_id: u32,
    position: Option<Position>,
) -> NodeResult {
    let data = ctx.arena.get_type_data(node_id);
    let value = if data.is_empty() {
        String::new()
    } else {
        ctx.arena.get_str(decode_string_ref_data(data)).to_string()
    };
    NodeResult::Node(hast::Node::Element(hast::Element {
        tag_name: "code".into(),
        properties: vec![],
        children: vec![hast::Node::Text(hast::Text {
            value: replace_eols_with_spaces(&value),
            position: None,
        })],
        position,
    }))
}

fn transform_math(ctx: &mut Context<'_>, node_id: u32, position: Option<Position>) -> NodeResult {
    let data = ctx.arena.get_type_data(node_id);
    let value = if data.is_empty() {
        String::new()
    } else {
        let d = decode_math_data(data);
        if d.value.len > 0 {
            ctx.arena.get_str(d.value).to_string()
        } else {
            String::new()
        }
    };
    let mut code_value = value;
    code_value.push('\n');
    NodeResult::Node(hast::Node::Element(hast::Element {
        tag_name: "pre".into(),
        properties: vec![],
        children: vec![hast::Node::Element(hast::Element {
            tag_name: "code".into(),
            properties: vec![(
                "class".into(),
                hast::PropertyValue::SpaceSeparated(vec![
                    "language-math".into(),
                    "math-display".into(),
                ]),
            )],
            children: vec![hast::Node::Text(hast::Text {
                value: code_value,
                position: None,
            })],
            position: position.clone(),
        })],
        position,
    }))
}

fn transform_inline_math(
    ctx: &mut Context<'_>,
    node_id: u32,
    position: Option<Position>,
) -> NodeResult {
    let data = ctx.arena.get_type_data(node_id);
    let value = if data.is_empty() {
        String::new()
    } else {
        ctx.arena.get_str(decode_string_ref_data(data)).to_string()
    };
    NodeResult::Node(hast::Node::Element(hast::Element {
        tag_name: "code".into(),
        properties: vec![(
            "class".into(),
            hast::PropertyValue::SpaceSeparated(vec!["language-math".into(), "math-inline".into()]),
        )],
        children: vec![hast::Node::Text(hast::Text {
            value: replace_eols_with_spaces(&value),
            position: None,
        })],
        position,
    }))
}

fn transform_break(position: Option<Position>) -> NodeResult {
    NodeResult::Fragment(vec![
        hast::Node::Element(hast::Element {
            tag_name: "br".into(),
            properties: vec![],
            children: vec![],
            position,
        }),
        hast::Node::Text(hast::Text {
            value: "\n".into(),
            position: None,
        }),
    ])
}

fn transform_link(ctx: &mut Context<'_>, node_id: u32, position: Option<Position>) -> NodeResult {
    let data = ctx.arena.get_type_data(node_id);
    let (url, title) = if data.is_empty() {
        (String::new(), None)
    } else {
        let d = decode_link_data(data);
        let url = ctx.arena.get_str(d.url).to_string();
        let title = if d.title.len > 0 {
            Some(ctx.arena.get_str(d.title).to_string())
        } else {
            None
        };
        (url, title)
    };

    let mut properties = vec![("href".into(), hast::PropertyValue::String(sanitize(&url)))];
    if let Some(t) = title {
        properties.push(("title".into(), hast::PropertyValue::String(t)));
    }

    let children = all_children(ctx, node_id);
    NodeResult::Node(hast::Node::Element(hast::Element {
        tag_name: "a".into(),
        properties,
        children,
        position,
    }))
}

fn transform_image(ctx: &mut Context<'_>, node_id: u32, position: Option<Position>) -> NodeResult {
    let data = ctx.arena.get_type_data(node_id);
    let (url, alt, title) = if data.is_empty() {
        (String::new(), String::new(), None)
    } else {
        let d = decode_image_data(data);
        let url = ctx.arena.get_str(d.url).to_string();
        let alt = ctx.arena.get_str(d.alt).to_string();
        let title = if d.title.len > 0 {
            Some(ctx.arena.get_str(d.title).to_string())
        } else {
            None
        };
        (url, alt, title)
    };

    let mut properties = vec![
        ("src".into(), hast::PropertyValue::String(sanitize(&url))),
        ("alt".into(), hast::PropertyValue::String(alt)),
    ];
    if let Some(t) = title {
        properties.push(("title".into(), hast::PropertyValue::String(t)));
    }

    NodeResult::Node(hast::Node::Element(hast::Element {
        tag_name: "img".into(),
        properties,
        children: vec![],
        position,
    }))
}

fn transform_link_reference(
    ctx: &mut Context<'_>,
    node_id: u32,
    position: Option<Position>,
) -> NodeResult {
    let data = ctx.arena.get_type_data(node_id);
    if data.is_empty() {
        // No identifier — can't resolve.
        let children = all_children(ctx, node_id);
        return NodeResult::Fragment(children);
    }
    let d = decode_reference_data(data);
    let identifier = ctx.arena.get_str(d.identifier).to_lowercase();

    let def = ctx.definitions.get(&identifier).cloned();
    if let Some((url, title)) = def {
        let mut properties = vec![("href".into(), hast::PropertyValue::String(sanitize(&url)))];
        if let Some(t) = title {
            properties.push(("title".into(), hast::PropertyValue::String(t)));
        }
        let children = all_children(ctx, node_id);
        NodeResult::Node(hast::Node::Element(hast::Element {
            tag_name: "a".into(),
            properties,
            children,
            position,
        }))
    } else {
        // Unresolved — return children.
        let children = all_children(ctx, node_id);
        NodeResult::Fragment(children)
    }
}

fn transform_image_reference(
    ctx: &mut Context<'_>,
    node_id: u32,
    position: Option<Position>,
) -> NodeResult {
    let data = ctx.arena.get_type_data(node_id);
    if data.is_empty() {
        return NodeResult::None;
    }
    let d = decode_reference_data(data);
    let identifier = ctx.arena.get_str(d.identifier).to_lowercase();

    let def = ctx.definitions.get(&identifier).cloned();
    if let Some((url, title)) = def {
        // alt comes from the label or identifier field in ReferenceData.
        // In the original mdast, ImageReference has an `alt` field. In the
        // arena codec, ImageReference uses ReferenceData which only has
        // identifier+label+reference_kind. The alt text is whatever the label
        // says. We use the label if present, otherwise identifier.
        let alt = if d.label.len > 0 {
            ctx.arena.get_str(d.label).to_string()
        } else {
            ctx.arena.get_str(d.identifier).to_string()
        };

        let mut properties = vec![
            ("src".into(), hast::PropertyValue::String(sanitize(&url))),
            ("alt".into(), hast::PropertyValue::String(alt)),
        ];
        if let Some(t) = title {
            properties.push(("title".into(), hast::PropertyValue::String(t)));
        }
        NodeResult::Node(hast::Node::Element(hast::Element {
            tag_name: "img".into(),
            properties,
            children: vec![],
            position,
        }))
    } else {
        NodeResult::None
    }
}

fn transform_footnote_definition(ctx: &mut Context<'_>, node_id: u32) -> NodeResult {
    // Children are converted lazily when building the footnote section.
    // We just need to ensure the node_id is recorded (done in pre-pass).
    // But we still need to convert children for the footnote section. The
    // pre-pass records the node_id; actual conversion happens in mdast_to_hast
    // when building the footer. Return None here.
    let _ = node_id;
    let _ = ctx;
    NodeResult::None
}

fn transform_footnote_reference(
    ctx: &mut Context<'_>,
    node_id: u32,
    position: Option<Position>,
) -> NodeResult {
    let data = ctx.arena.get_type_data(node_id);
    let identifier = if data.is_empty() {
        String::new()
    } else {
        let d = decode_reference_data(data);
        ctx.arena.get_str(d.identifier).to_string()
    };

    let safe_id = sanitize(&identifier.to_lowercase());

    // Find or add call entry.
    let mut call_index = 0;
    while call_index < ctx.footnote_calls.len() {
        if ctx.footnote_calls[call_index].0 == identifier {
            break;
        }
        call_index += 1;
    }

    if call_index == ctx.footnote_calls.len() {
        ctx.footnote_calls.push((identifier.clone(), 0));
    }

    ctx.footnote_calls[call_index].1 += 1;
    let reuse_counter = ctx.footnote_calls[call_index].1;

    NodeResult::Node(hast::Node::Element(hast::Element {
        tag_name: "sup".into(),
        properties: vec![],
        children: vec![hast::Node::Element(hast::Element {
            tag_name: "a".into(),
            properties: vec![
                (
                    "href".into(),
                    hast::PropertyValue::String(format!("#fn-{safe_id}")),
                ),
                (
                    "id".into(),
                    hast::PropertyValue::String(format!(
                        "fnref-{}{}",
                        safe_id,
                        if reuse_counter > 1 {
                            format!("-{reuse_counter}")
                        } else {
                            String::new()
                        }
                    )),
                ),
                ("dataFootnoteRef".into(), hast::PropertyValue::Boolean(true)),
                (
                    "ariaDescribedBy".into(),
                    hast::PropertyValue::String("footnote-label".into()),
                ),
            ],
            children: vec![hast::Node::Text(hast::Text {
                value: (call_index + 1).to_string(),
                position: None,
            })],
            position: None,
        })],
        position,
    }))
}

fn transform_table(ctx: &mut Context<'_>, node_id: u32, position: Option<Position>) -> NodeResult {
    let data = ctx.arena.get_type_data(node_id);
    let alignments: Vec<ColumnAlign> = if data.is_empty() {
        vec![]
    } else {
        let (_header, aligns) = decode_table_data(data);
        aligns
    };

    let child_ids: Vec<u32> = ctx.arena.get_children(node_id).to_vec();
    let mut rows: Vec<hast::Node> = vec![];

    for (idx, &child_id) in child_ids.iter().enumerate() {
        let child_raw = ctx.arena.get_node(child_id);
        let child_nt = NodeType::from_u8(child_raw.node_type).unwrap_or(NodeType::Root);
        if child_nt != NodeType::TableRow {
            continue;
        }
        let child_pos = to_position(child_raw);
        let result = transform_table_row(ctx, child_id, idx == 0, &alignments, child_pos);
        append_result(&mut rows, result);
    }

    let body_rows = rows.split_off(1);
    let head_row = rows.pop();
    let mut children = vec![];

    if let Some(row) = head_row {
        let pos = row.position().cloned();
        children.push(hast::Node::Element(hast::Element {
            tag_name: "thead".into(),
            properties: vec![],
            children: wrap(vec![row], true),
            position: pos,
        }));
    }

    if !body_rows.is_empty() {
        let mut tbody_position = None;
        if let Some(position_start) = body_rows.first().and_then(hast::Node::position)
            && let Some(position_end) = body_rows.last().and_then(hast::Node::position)
        {
            tbody_position = Some(Position {
                start: position_start.start.clone(),
                end: position_end.end.clone(),
            });
        }

        children.push(hast::Node::Element(hast::Element {
            tag_name: "tbody".into(),
            properties: vec![],
            children: wrap(body_rows, true),
            position: tbody_position,
        }));
    }

    NodeResult::Node(hast::Node::Element(hast::Element {
        tag_name: "table".into(),
        properties: vec![],
        children: wrap(children, true),
        position,
    }))
}

fn transform_table_row(
    ctx: &mut Context<'_>,
    node_id: u32,
    head: bool,
    alignments: &[ColumnAlign],
    position: Option<Position>,
) -> NodeResult {
    let child_ids: Vec<u32> = ctx.arena.get_children(node_id).to_vec();
    let len = if alignments.is_empty() {
        child_ids.len()
    } else {
        alignments.len()
    };

    let mut children = vec![];
    let mut index = 0;
    while index < len {
        let align = alignments.get(index).copied().unwrap_or(ColumnAlign::None);
        let cell_pos;
        let cell_id_opt = child_ids.get(index).copied();

        let result = if let Some(cell_id) = cell_id_opt {
            let cell_raw = ctx.arena.get_node(cell_id);
            cell_pos = to_position(cell_raw);
            transform_table_cell(ctx, cell_id, head, align, cell_pos)
        } else {
            // Empty cell placeholder.
            NodeResult::Node(hast::Node::Element(hast::Element {
                tag_name: if head { "th".into() } else { "td".into() },
                properties: vec![],
                children: vec![],
                position: None,
            }))
        };
        append_result(&mut children, result);
        index += 1;
    }

    NodeResult::Node(hast::Node::Element(hast::Element {
        tag_name: "tr".into(),
        properties: vec![],
        children: wrap(children, true),
        position,
    }))
}

fn transform_table_cell(
    ctx: &mut Context<'_>,
    node_id: u32,
    head: bool,
    align: ColumnAlign,
    position: Option<Position>,
) -> NodeResult {
    let align_value = match align {
        ColumnAlign::None => None,
        ColumnAlign::Left => Some("left"),
        ColumnAlign::Right => Some("right"),
        ColumnAlign::Center => Some("center"),
    };

    let mut properties = vec![];
    if let Some(v) = align_value {
        properties.push(("align".into(), hast::PropertyValue::String(v.into())));
    }

    let children = all_children(ctx, node_id);
    NodeResult::Node(hast::Node::Element(hast::Element {
        tag_name: if head { "th".into() } else { "td".into() },
        properties,
        children,
        position,
    }))
}

fn transform_mdx_jsx_flow_element(
    ctx: &mut Context<'_>,
    node_id: u32,
    position: Option<Position>,
) -> NodeResult {
    let name = get_mdx_jsx_name(ctx.arena, node_id);
    let attributes = get_mdx_jsx_attributes(ctx.arena, node_id);
    let children = all_children(ctx, node_id);
    NodeResult::Node(hast::Node::MdxJsxElement(hast::MdxJsxElement {
        name,
        attributes,
        children,
        position,
    }))
}

fn transform_mdx_jsx_text_element(
    ctx: &mut Context<'_>,
    node_id: u32,
    position: Option<Position>,
) -> NodeResult {
    let name = get_mdx_jsx_name(ctx.arena, node_id);
    let attributes = get_mdx_jsx_attributes(ctx.arena, node_id);
    let children = all_children(ctx, node_id);
    NodeResult::Node(hast::Node::MdxJsxTextElement(hast::MdxJsxElement {
        name,
        attributes,
        children,
        position,
    }))
}

fn transform_mdx_expression(
    ctx: &mut Context<'_>,
    node_id: u32,
    position: Option<Position>,
) -> NodeResult {
    let data = ctx.arena.get_type_data(node_id);
    let value = if data.is_empty() {
        String::new()
    } else {
        let d = decode_expression_data(data);
        ctx.arena.get_str(d.value).to_string()
    };
    NodeResult::Node(hast::Node::MdxExpression(hast::MdxExpression {
        value,
        position,
        stops: vec![],
    }))
}

fn transform_mdxjs_esm(
    ctx: &mut Context<'_>,
    node_id: u32,
    position: Option<Position>,
) -> NodeResult {
    let raw = ctx.arena.get_node(node_id);
    let value = ctx.arena.source()[raw.start_offset as usize..raw.end_offset as usize].to_string();
    NodeResult::Node(hast::Node::MdxjsEsm(hast::MdxjsEsm {
        value,
        position,
        stops: vec![],
    }))
}

fn get_mdx_jsx_name(arena: &dyn ReadMdast, node_id: u32) -> Option<String> {
    let data = arena.get_type_data(node_id);
    if data.is_empty() {
        return None;
    }
    let d = decode_mdx_jsx_element_data(data);
    if d.name.len > 0 {
        Some(arena.get_str(d.name).to_string())
    } else {
        None
    }
}

/// Extract JSX attributes from the raw source text of an MDX JSX element node.
fn get_mdx_jsx_attributes(arena: &dyn ReadMdast, node_id: u32) -> Vec<AttributeContent> {
    let raw = arena.get_node(node_id);
    let source = arena.source();
    let text = &source[raw.start_offset as usize..raw.end_offset as usize];

    // Extract just the opening tag: everything up to the first `>` that isn't inside
    // braces or strings. For self-closing `<Foo ... />` the whole thing is the opening tag.
    // For `<Foo ...>children</Foo>` we want just `<Foo ...>`.
    let opening_tag = extract_opening_tag(text);
    parse_jsx_attributes(opening_tag)
}

/// Extract the opening tag portion from a JSX element's raw source.
/// Handles brace/string nesting so `>` inside expressions doesn't terminate early.
fn extract_opening_tag(text: &str) -> &str {
    let mut depth = 0i32;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_backtick = false;
    let mut prev = '\0';

    for (i, ch) in text.char_indices() {
        if in_single_quote {
            if ch == '\'' && prev != '\\' {
                in_single_quote = false;
            }
        } else if in_double_quote {
            if ch == '"' && prev != '\\' {
                in_double_quote = false;
            }
        } else if in_backtick {
            if ch == '`' && prev != '\\' {
                in_backtick = false;
            }
        } else {
            match ch {
                '\'' => in_single_quote = true,
                '"' => in_double_quote = true,
                '`' => in_backtick = true,
                '{' => depth += 1,
                '}' => depth -= 1,
                '>' if depth == 0 => return &text[..=i],
                _ => {}
            }
        }
        prev = ch;
    }
    text
}

/// Parse JSX attributes from an opening tag string like `<Foo bar="baz" qux={expr} />`.
fn parse_jsx_attributes(tag: &str) -> Vec<AttributeContent> {
    let mut attrs = Vec::new();
    let bytes = tag.as_bytes();
    let len = bytes.len();

    // Skip past `<` and the tag name.
    let mut i = 1; // skip '<'

    // Skip optional `/` for closing tags (shouldn't happen here, but be safe)
    if i < len && bytes[i] == b'/' {
        i += 1;
    }

    // Skip whitespace
    while i < len && bytes[i].is_ascii_whitespace() {
        i += 1;
    }

    // Skip tag name: identifier chars (letters, digits, `.`, `-`, `:`, `_`)
    while i < len && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'.' | b'-' | b':' | b'_')) {
        i += 1;
    }

    // Now parse attributes until we hit `>` or `/>` or end
    loop {
        // Skip whitespace
        while i < len && bytes[i].is_ascii_whitespace() {
            i += 1;
        }

        if i >= len {
            break;
        }

        // End of tag?
        if bytes[i] == b'>' || (bytes[i] == b'/' && i + 1 < len && bytes[i + 1] == b'>') {
            break;
        }

        // Spread expression attribute: `{...expr}`
        if bytes[i] == b'{' {
            let start = i + 1; // skip `{`
            i += 1;
            let mut depth = 1i32;
            while i < len && depth > 0 {
                match bytes[i] {
                    b'{' => depth += 1,
                    b'}' => depth -= 1,
                    b'\'' | b'"' | b'`' => {
                        let quote = bytes[i];
                        i += 1;
                        while i < len && bytes[i] != quote {
                            if bytes[i] == b'\\' {
                                i += 1; // skip escaped char
                            }
                            i += 1;
                        }
                        // i now points at closing quote, loop will advance
                    }
                    _ => {}
                }
                i += 1;
            }
            // start..i-1 is the content inside { }
            let value = tag[start..i.saturating_sub(1)].trim().to_string();
            attrs.push(AttributeContent::Expression(MdxJsxExpressionAttribute {
                value,
                stops: vec![],
            }));
            continue;
        }

        // Named attribute: `name`, `name="value"`, `name={expr}`
        let name_start = i;
        while i < len && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'-' | b':' | b'_')) {
            i += 1;
        }

        if i == name_start {
            // Not a valid attribute start, skip this byte to avoid infinite loop
            i += 1;
            continue;
        }

        let name = tag[name_start..i].to_string();

        // Skip whitespace
        while i < len && bytes[i].is_ascii_whitespace() {
            i += 1;
        }

        // Check for `=`
        if i < len && bytes[i] == b'=' {
            i += 1; // skip `=`

            // Skip whitespace
            while i < len && bytes[i].is_ascii_whitespace() {
                i += 1;
            }

            if i >= len {
                // Attribute with `=` but no value (malformed), treat as boolean
                attrs.push(AttributeContent::Property(MdxJsxAttribute { name, value: None }));
                continue;
            }

            if bytes[i] == b'"' || bytes[i] == b'\'' {
                // String literal value
                let quote = bytes[i];
                i += 1;
                let val_start = i;
                while i < len && bytes[i] != quote {
                    if bytes[i] == b'\\' {
                        i += 1; // skip escaped char
                    }
                    i += 1;
                }
                let value = tag[val_start..i].to_string();
                if i < len {
                    i += 1; // skip closing quote
                }
                attrs.push(AttributeContent::Property(MdxJsxAttribute {
                    name,
                    value: Some(AttributeValue::Literal(value)),
                }));
            } else if bytes[i] == b'{' {
                // Expression value
                i += 1; // skip `{`
                let val_start = i;
                let mut depth = 1i32;
                while i < len && depth > 0 {
                    match bytes[i] {
                        b'{' => depth += 1,
                        b'}' => depth -= 1,
                        b'\'' | b'"' | b'`' => {
                            let q = bytes[i];
                            i += 1;
                            while i < len && bytes[i] != q {
                                if bytes[i] == b'\\' {
                                    i += 1;
                                }
                                i += 1;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                let value = tag[val_start..i.saturating_sub(1)].to_string();
                attrs.push(AttributeContent::Property(MdxJsxAttribute {
                    name,
                    value: Some(AttributeValue::Expression(AttributeValueExpression {
                        value,
                        stops: vec![],
                    })),
                }));
            } else {
                // Unquoted value — shouldn't happen in valid JSX, treat as boolean
                attrs.push(AttributeContent::Property(MdxJsxAttribute { name, value: None }));
            }
        } else {
            // Boolean attribute (no `=`)
            attrs.push(AttributeContent::Property(MdxJsxAttribute { name, value: None }));
        }
    }

    attrs
}
