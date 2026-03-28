//! Turn an HAST binary buffer into a JavaScript AST.
//!
//! Reads directly from the HAST binary format (`MdastView`) — no intermediate
//! `hast::Node` tree is needed.

use crate::configuration::OptimizeStaticConfig;
use crate::oxc::{parse_esm_to_tree, parse_expression_to_tree, serialize};
use crate::oxc_utils::{
    create_jsx_attr_name_from_str, create_jsx_name_from_str, inter_element_whitespace,
    is_literal_name,
};
use core::str;
use std::cell::Cell;

use oxc_allocator::{Allocator, Box as OxcBox, Vec as OxcVec};
use oxc_ast::ast::{
    Expression, ExpressionStatement, JSXAttribute, JSXAttributeItem, JSXAttributeValue, JSXChild,
    JSXClosingElement, JSXClosingFragment, JSXElement, JSXEmptyExpression, JSXExpression,
    JSXExpressionContainer, JSXFragment, JSXOpeningElement, JSXOpeningFragment, JSXSpreadAttribute,
    Program, Statement, StringLiteral,
};
use oxc_span::{Atom, SPAN, Span};
use oxc_syntax::node::NodeId;
use rustc_hash::FxHashSet;
use tryckeri_hast::codec::{
    decode_element_prop, decode_element_prop_count, decode_element_tag, decode_mdx_jsx_attr,
    decode_mdx_jsx_attr_count, decode_mdx_jsx_element_name, decode_text_data,
};
use tryckeri_hast::node_types::{
    HAST_COMMENT, HAST_ELEMENT, HAST_MDX_ESM, HAST_MDX_EXPRESSION, HAST_MDX_JSX_ELEMENT,
    HAST_MDX_JSX_TEXT_ELEMENT, HAST_RAW, HAST_ROOT, HAST_TEXT, MDX_ATTR_BOOLEAN_PROP,
    MDX_ATTR_EXPRESSION_PROP, MDX_ATTR_LITERAL_PROP, MDX_ATTR_SPREAD, PROP_BOOL_TRUE,
    PROP_COMMA_SEP, PROP_SPACE_SEP, PROP_STRING,
};
use tryckeri_mdast::MdastView;
use tryckeri_mdast::mdx_types::{self as message, Location, MdxExpressionKind};

/// Get a Span from a HAST binary node's position data.
/// Uses offset+1 convention so that (0,0) remains SPAN (dummy).
fn node_span(view: &MdastView, node_id: u32) -> Span {
    let node = view.get_node(node_id);
    if node.start_offset == 0 && node.end_offset == 0 && node.start_line == 0 {
        SPAN
    } else {
        Span::new(node.start_offset + 1, node.end_offset + 1)
    }
}

/// Result.
pub struct MdxProgram<'a> {
    /// File path.
    pub path: Option<String>,
    /// Allocator that owns all AST data.
    pub allocator: &'a Allocator,
    /// JS AST.
    pub program: Program<'a>,
    /// Comments relating to AST (stored separately since OXC comments are on Program).
    pub comments: Vec<MdxComment>,
}

/// A comment stored outside the OXC AST.
#[derive(Debug, Clone)]
pub struct MdxComment {
    /// Block or line comment.
    pub kind: MdxCommentKind,
    /// Text of the comment.
    pub text: String,
    /// Span of the comment.
    pub span: Span,
}

/// Comment kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MdxCommentKind {
    Block,
    Line,
}

impl MdxProgram<'_> {
    /// Serialize to JS.
    pub fn serialize(&self) -> String {
        serialize(&self.program)
    }
}

/// Whether we're in HTML or SVG.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Space {
    Html,
    Svg,
}

/// Context used to compile hast into OXC's ES AST.
struct Context<'a> {
    space: Space,
    comments: Vec<MdxComment>,
    esm: Vec<Statement<'a>>,
    location: Option<&'a Location>,
    allocator: &'a Allocator,
    view: &'a MdastView<'a>,
    optimize_static: Option<&'a OptimizeStaticConfig>,
}

/// Compile a HAST binary buffer into OXC's ES AST.
pub fn hast_util_to_oxc<'a>(
    view: &'a MdastView<'a>,
    path: Option<String>,
    location: Option<&'a Location>,
    explicit_jsxs: &mut FxHashSet<Span>,
    allocator: &'a Allocator,
    optimize_static: Option<&'a OptimizeStaticConfig>,
) -> Result<MdxProgram<'a>, message::Message> {
    let mut context = Context {
        space: Space::Html,
        comments: vec![],
        esm: vec![],
        location,
        allocator,
        view,
        optimize_static,
    };
    let expr = match one(&mut context, 0, explicit_jsxs)? {
        Some(JSXChild::Fragment(x)) => Some(Expression::JSXFragment(x)),
        Some(JSXChild::Element(x)) => Some(Expression::JSXElement(x)),
        Some(child) => {
            let mut children = OxcVec::with_capacity_in(1, allocator);
            children.push(child);
            Some(Expression::JSXFragment(OxcBox::new_in(
                create_fragment(allocator, children, SPAN),
                allocator,
            )))
        }
        None => None,
    };

    // Add the ESM.
    let mut body = OxcVec::from_iter_in(context.esm, allocator);

    // We have some content, wrap it.
    if let Some(expr) = expr {
        body.push(Statement::ExpressionStatement(OxcBox::new_in(
            ExpressionStatement {
                node_id: Cell::new(NodeId::DUMMY),
                span: SPAN,
                expression: expr,
            },
            allocator,
        )));
    }

    let program = Program {
        node_id: Cell::new(NodeId::DUMMY),
        span: SPAN,
        source_type: oxc_span::SourceType::mjs().with_jsx(true),
        source_text: "",
        comments: OxcVec::new_in(allocator),
        hashbang: None,
        directives: OxcVec::new_in(allocator),
        body,
        scope_id: Cell::default(),
    };

    Ok(MdxProgram {
        path,
        allocator,
        program,
        comments: context.comments,
    })
}

/// Transform one node.
fn one<'a>(
    context: &mut Context<'a>,
    node_id: u32,
    explicit_jsxs: &mut FxHashSet<Span>,
) -> Result<Option<JSXChild<'a>>, message::Message> {
    let node = context.view.get_node(node_id);
    let raw_type = node.node_type;

    match raw_type {
        HAST_ROOT => transform_root(context, node_id, explicit_jsxs),
        HAST_ELEMENT => transform_element(context, node_id, explicit_jsxs),
        HAST_TEXT | HAST_RAW => Ok(transform_text(context, node_id)),
        HAST_COMMENT => Ok(Some(transform_comment(context, node_id))),
        HAST_MDX_JSX_ELEMENT | HAST_MDX_JSX_TEXT_ELEMENT => {
            transform_mdx_jsx_element(context, node_id, explicit_jsxs)
        }
        HAST_MDX_EXPRESSION => transform_mdx_expression(context, node_id),
        HAST_MDX_ESM => transform_mdxjs_esm(context, node_id),
        _ => Ok(None),
    }
}

/// Check if a HAST subtree is fully static (only HTML elements, text, comments, raw).
///
/// Returns `false` for any subtree containing MDX nodes (components, expressions, ESM)
/// or elements whose tag name is in the ignore list.
fn is_static_subtree(view: &MdastView, node_id: u32, config: &OptimizeStaticConfig) -> bool {
    let node = view.get_node(node_id);
    let raw_type = node.node_type;

    match raw_type {
        HAST_TEXT | HAST_RAW | HAST_COMMENT => true,
        HAST_ELEMENT => {
            let data = view.get_type_data(node_id);
            if data.len() < 16 {
                return false;
            }
            let tag_ref = decode_element_tag(data);
            let tag = view.get_str(tag_ref);
            // Must be a lowercase HTML tag and not in the ignore list
            if !is_literal_name(tag) {
                return false;
            }
            if config.ignore_elements.iter().any(|s| s == tag) {
                return false;
            }
            // All children must also be static
            view.get_children(node_id)
                .iter()
                .all(|&cid| is_static_subtree(view, cid, config))
        }
        // MDX nodes, root, or anything else → not static
        _ => false,
    }
}

/// Try to render a static subtree to an HTML string. Returns `None` if not static.
/// Combines the check and serialization in a single pass for efficiency.
fn try_render_static(
    view: &MdastView,
    node_id: u32,
    config: &OptimizeStaticConfig,
    out: &mut String,
) -> bool {
    if !is_static_subtree(view, node_id, config) {
        return false;
    }
    tryckeri_hast::render_node(node_id, view, out);
    true
}

/// Create a JSX node that injects raw HTML according to the optimization config.
fn create_raw_html_jsx<'a>(
    alloc: &'a Allocator,
    html: &str,
    config: &OptimizeStaticConfig,
) -> JSXChild<'a> {
    let html_str = StringLiteral {
        node_id: Cell::new(NodeId::DUMMY),
        span: SPAN,
        value: Atom::from(alloc.alloc_str(html)),
        raw: None,
        lone_surrogates: false,
    };

    let prop_value = if config.wrap_prop_value {
        // React-style: { __html: "..." }
        use oxc_ast::ast::{ObjectProperty, ObjectPropertyKind, PropertyKey, PropertyKind};
        let mut props = OxcVec::with_capacity_in(1, alloc);
        props.push(ObjectPropertyKind::ObjectProperty(OxcBox::new_in(
            ObjectProperty {
                span: SPAN,
                kind: PropertyKind::Init,
                key: PropertyKey::StaticIdentifier(OxcBox::new_in(
                    crate::oxc_utils::create_ident_name(alloc, "__html"),
                    alloc,
                )),
                value: Expression::StringLiteral(OxcBox::new_in(html_str, alloc)),
                method: false,
                shorthand: false,
                computed: false,
                node_id: Cell::new(NodeId::DUMMY),
            },
            alloc,
        )));
        JSXAttributeValue::ExpressionContainer(OxcBox::new_in(
            JSXExpressionContainer {
                node_id: Cell::new(NodeId::DUMMY),
                span: SPAN,
                expression: JSXExpression::from(crate::oxc_utils::create_object_expression(
                    alloc, props,
                )),
            },
            alloc,
        ))
    } else {
        // Plain string: prop="<html>"
        JSXAttributeValue::StringLiteral(OxcBox::new_in(html_str, alloc))
    };

    let mut attrs = OxcVec::with_capacity_in(1, alloc);
    attrs.push(JSXAttributeItem::Attribute(OxcBox::new_in(
        JSXAttribute {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            name: create_jsx_attr_name_from_str(alloc, &config.prop),
            value: Some(prop_value),
        },
        alloc,
    )));

    let children = OxcVec::new_in(alloc);
    JSXChild::Element(OxcBox::new_in(
        create_element(alloc, &config.component, attrs, children, SPAN),
        alloc,
    ))
}

/// Transform children of `parent`.
fn all<'a>(
    context: &mut Context<'a>,
    parent_id: u32,
    explicit_jsxs: &mut FxHashSet<Span>,
) -> Result<OxcVec<'a, JSXChild<'a>>, message::Message> {
    let mut result = OxcVec::new_in(context.allocator);
    let child_count = context.view.get_children(parent_id).len();

    if let Some(config) = context.optimize_static {
        // Optimization enabled: group consecutive static siblings into raw HTML.
        let mut i = 0;
        while i < child_count {
            let child_id = context.view.get_children(parent_id)[i];

            let mut html_buf = String::new();
            if try_render_static(context.view, child_id, config, &mut html_buf) {
                // Accumulate consecutive static siblings
                i += 1;
                while i < child_count {
                    let next_id = context.view.get_children(parent_id)[i];
                    if !try_render_static(context.view, next_id, config, &mut html_buf) {
                        break;
                    }
                    i += 1;
                }
                if !html_buf.is_empty() {
                    result.push(create_raw_html_jsx(context.allocator, &html_buf, config));
                }
            } else {
                if let Some(child) = one(context, child_id, explicit_jsxs)? {
                    result.push(child);
                }
                i += 1;
            }
        }
    } else {
        // No optimization: normal path.
        // Index-based loop needed: `context` borrows `view`, so can't iterate by ref.
        for i in 0..child_count {
            let child_id = context.view.get_children(parent_id)[i];
            if let Some(child) = one(context, child_id, explicit_jsxs)? {
                result.push(child);
            }
        }
    }

    Ok(result)
}

/// Comment node.
fn transform_comment<'a>(context: &mut Context<'a>, node_id: u32) -> JSXChild<'a> {
    let data = context.view.get_type_data(node_id);
    let value = if data.len() >= 8 {
        context.view.get_str(decode_text_data(data)).to_string()
    } else {
        String::new()
    };

    context.comments.push(MdxComment {
        kind: MdxCommentKind::Block,
        text: value,
        span: SPAN,
    });

    let alloc = context.allocator;
    JSXChild::ExpressionContainer(OxcBox::new_in(
        JSXExpressionContainer {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            expression: JSXExpression::EmptyExpression(JSXEmptyExpression {
                node_id: Cell::new(NodeId::DUMMY),
                span: SPAN,
            }),
        },
        alloc,
    ))
}

/// Element node.
fn transform_element<'a>(
    context: &mut Context<'a>,
    node_id: u32,
    explicit_jsxs: &mut FxHashSet<Span>,
) -> Result<Option<JSXChild<'a>>, message::Message> {
    let data = context.view.get_type_data(node_id);
    if data.len() < 16 {
        return Ok(None);
    }

    let tag_ref = decode_element_tag(data);
    let tag_name = context.view.get_str(tag_ref);

    let space = context.space;
    if space == Space::Html && tag_name == "svg" {
        context.space = Space::Svg;
    }

    let children = all(context, node_id, explicit_jsxs)?;
    context.space = space;

    let alloc = context.allocator;
    let mut attrs = OxcVec::new_in(alloc);

    let prop_count = decode_element_prop_count(data);
    for i in 0..prop_count {
        let (name_ref, value_kind, value_ref) = decode_element_prop(data, i);
        let name = context.view.get_str(name_ref);

        let value = match value_kind {
            PROP_BOOL_TRUE => None,
            PROP_STRING | PROP_SPACE_SEP | PROP_COMMA_SEP => {
                let v = context.view.get_str(value_ref);
                Some(JSXAttributeValue::StringLiteral(OxcBox::new_in(
                    StringLiteral {
                        node_id: Cell::new(NodeId::DUMMY),
                        span: SPAN,
                        value: Atom::from(alloc.alloc_str(v)),
                        raw: None,
                        lone_surrogates: false,
                    },
                    alloc,
                )))
            }
            _ => continue,
        };

        let attr_name = prop_to_attr_name(name);
        attrs.push(JSXAttributeItem::Attribute(OxcBox::new_in(
            JSXAttribute {
                node_id: Cell::new(NodeId::DUMMY),
                span: SPAN,
                name: create_jsx_attr_name_from_str(alloc, &attr_name),
                value,
            },
            alloc,
        )));
    }

    Ok(Some(JSXChild::Element(OxcBox::new_in(
        create_element(alloc, tag_name, attrs, children, SPAN),
        alloc,
    ))))
}

/// MDX JSX element node.
fn transform_mdx_jsx_element<'a>(
    context: &mut Context<'a>,
    node_id: u32,
    explicit_jsxs: &mut FxHashSet<Span>,
) -> Result<Option<JSXChild<'a>>, message::Message> {
    let data = context.view.get_type_data(node_id);
    if data.len() < 16 {
        return Ok(None);
    }

    let name_ref = decode_mdx_jsx_element_name(data);
    let name_str = context.view.get_str(name_ref);
    let name = if name_str.is_empty() {
        None
    } else {
        Some(name_str)
    };

    let space = context.space;
    if let Some(n) = name
        && space == Space::Html
        && n == "svg"
    {
        context.space = Space::Svg;
    }

    let children = all(context, node_id, explicit_jsxs)?;
    context.space = space;

    let alloc = context.allocator;
    let mut attrs = OxcVec::new_in(alloc);

    let attr_count = decode_mdx_jsx_attr_count(data);
    for i in 0..attr_count {
        let (kind, attr_name_ref, attr_value_ref) = decode_mdx_jsx_attr(data, i);

        let attr = match kind {
            MDX_ATTR_BOOLEAN_PROP => {
                let attr_name = context.view.get_str(attr_name_ref);
                JSXAttributeItem::Attribute(OxcBox::new_in(
                    JSXAttribute {
                        node_id: Cell::new(NodeId::DUMMY),
                        span: SPAN,
                        name: create_jsx_attr_name_from_str(alloc, attr_name),
                        value: None,
                    },
                    alloc,
                ))
            }
            MDX_ATTR_LITERAL_PROP => {
                let attr_name = context.view.get_str(attr_name_ref);
                let attr_value = context.view.get_str(attr_value_ref);
                JSXAttributeItem::Attribute(OxcBox::new_in(
                    JSXAttribute {
                        node_id: Cell::new(NodeId::DUMMY),
                        span: SPAN,
                        name: create_jsx_attr_name_from_str(alloc, attr_name),
                        value: Some(JSXAttributeValue::StringLiteral(OxcBox::new_in(
                            StringLiteral {
                                node_id: Cell::new(NodeId::DUMMY),
                                span: SPAN,
                                value: Atom::from(alloc.alloc_str(attr_value)),
                                raw: None,
                                lone_surrogates: false,
                            },
                            alloc,
                        ))),
                    },
                    alloc,
                ))
            }
            MDX_ATTR_EXPRESSION_PROP => {
                let attr_name = context.view.get_str(attr_name_ref);
                let expr_value = context.view.get_str(attr_value_ref);
                let expr = parse_expression_to_tree(
                    expr_value,
                    &MdxExpressionKind::AttributeValueExpression,
                    &[],
                    context.location,
                    alloc,
                )?
                .unwrap();
                JSXAttributeItem::Attribute(OxcBox::new_in(
                    JSXAttribute {
                        node_id: Cell::new(NodeId::DUMMY),
                        span: SPAN,
                        name: create_jsx_attr_name_from_str(alloc, attr_name),
                        value: Some(JSXAttributeValue::ExpressionContainer(OxcBox::new_in(
                            JSXExpressionContainer {
                                node_id: Cell::new(NodeId::DUMMY),
                                span: SPAN,
                                expression: JSXExpression::from(expr),
                            },
                            alloc,
                        ))),
                    },
                    alloc,
                ))
            }
            MDX_ATTR_SPREAD => {
                let expr_value = context.view.get_str(attr_value_ref);
                let expr = parse_expression_to_tree(
                    expr_value,
                    &MdxExpressionKind::AttributeExpression,
                    &[],
                    context.location,
                    alloc,
                )?;
                JSXAttributeItem::SpreadAttribute(OxcBox::new_in(
                    JSXSpreadAttribute {
                        node_id: Cell::new(NodeId::DUMMY),
                        span: SPAN,
                        argument: expr.unwrap(),
                    },
                    alloc,
                ))
            }
            _ => continue,
        };

        attrs.push(attr);
    }

    let span = node_span(context.view, node_id);
    Ok(Some(if let Some(n) = name {
        explicit_jsxs.insert(span);
        JSXChild::Element(OxcBox::new_in(
            create_element(alloc, n, attrs, children, span),
            alloc,
        ))
    } else {
        JSXChild::Fragment(OxcBox::new_in(
            create_fragment(alloc, children, span),
            alloc,
        ))
    }))
}

/// MDX expression node.
fn transform_mdx_expression<'a>(
    context: &mut Context<'a>,
    node_id: u32,
) -> Result<Option<JSXChild<'a>>, message::Message> {
    let data = context.view.get_type_data(node_id);
    let value = if data.len() >= 8 {
        context.view.get_str(decode_text_data(data))
    } else {
        ""
    };

    let alloc = context.allocator;
    let span = node_span(context.view, node_id);
    let expr = parse_expression_to_tree(
        value,
        &MdxExpressionKind::Expression,
        &[],
        context.location,
        alloc,
    )?;
    let child = if let Some(expr) = expr {
        JSXExpression::from(expr)
    } else {
        JSXExpression::EmptyExpression(JSXEmptyExpression {
            node_id: Cell::new(NodeId::DUMMY),
            span,
        })
    };

    Ok(Some(JSXChild::ExpressionContainer(OxcBox::new_in(
        JSXExpressionContainer {
            node_id: Cell::new(NodeId::DUMMY),
            expression: child,
            span,
        },
        alloc,
    ))))
}

/// MDX ESM node.
fn transform_mdxjs_esm<'a>(
    context: &mut Context<'a>,
    node_id: u32,
) -> Result<Option<JSXChild<'a>>, message::Message> {
    let data = context.view.get_type_data(node_id);
    let value = if data.len() >= 8 {
        context.view.get_str(decode_text_data(data))
    } else {
        ""
    };

    let alloc = context.allocator;
    let mut program = parse_esm_to_tree(value, &[], context.location, alloc)?;
    let body = std::mem::replace(&mut program.body, OxcVec::new_in(alloc));
    for stmt in body {
        context.esm.push(stmt);
    }
    Ok(None)
}

/// Root node.
fn transform_root<'a>(
    context: &mut Context<'a>,
    node_id: u32,
    explicit_jsxs: &mut FxHashSet<Span>,
) -> Result<Option<JSXChild<'a>>, message::Message> {
    let alloc = context.allocator;
    let children_vec = all(context, node_id, explicit_jsxs)?;
    let mut children: Vec<JSXChild<'a>> = children_vec.into_iter().collect();
    let mut queue = vec![];
    let mut nodes = vec![];
    let mut seen = false;

    children.reverse();

    // Remove initial/final whitespace.
    while let Some(child) = children.pop() {
        let mut stash = false;

        if let JSXChild::ExpressionContainer(container) = &child
            && let JSXExpression::StringLiteral(str_lit) = &container.expression
            && inter_element_whitespace(str_lit.value.as_str())
        {
            stash = true;
        }

        if stash {
            if seen {
                queue.push(child);
            }
        } else {
            if !queue.is_empty() {
                nodes.append(&mut queue);
            }
            nodes.push(child);
            seen = true;
        }
    }

    let nodes = OxcVec::from_iter_in(nodes, alloc);

    Ok(Some(JSXChild::Fragment(OxcBox::new_in(
        create_fragment(alloc, nodes, SPAN),
        alloc,
    ))))
}

/// Text node (also used for raw HTML in JS context).
fn transform_text<'a>(context: &mut Context<'a>, node_id: u32) -> Option<JSXChild<'a>> {
    let data = context.view.get_type_data(node_id);
    let value = if data.len() >= 8 {
        context.view.get_str(decode_text_data(data))
    } else {
        return None;
    };

    if value.is_empty() {
        return None;
    }

    let alloc = context.allocator;
    Some(JSXChild::ExpressionContainer(OxcBox::new_in(
        JSXExpressionContainer {
            node_id: Cell::new(NodeId::DUMMY),
            expression: JSXExpression::StringLiteral(OxcBox::new_in(
                StringLiteral {
                    node_id: Cell::new(NodeId::DUMMY),
                    span: SPAN,
                    value: Atom::from(alloc.alloc_str(value)),
                    raw: None,
                    lone_surrogates: false,
                },
                alloc,
            )),
            span: SPAN,
        },
        alloc,
    )))
}

/// Create an element.
fn create_element<'a>(
    alloc: &'a Allocator,
    name: &str,
    attrs: OxcVec<'a, JSXAttributeItem<'a>>,
    children: OxcVec<'a, JSXChild<'a>>,
    span: Span,
) -> JSXElement<'a> {
    let self_closing = children.is_empty();

    JSXElement {
        node_id: Cell::new(NodeId::DUMMY),
        span,
        opening_element: OxcBox::new_in(
            JSXOpeningElement {
                node_id: Cell::new(NodeId::DUMMY),
                span: SPAN,
                name: create_jsx_name_from_str(alloc, name),
                attributes: attrs,
                type_arguments: None,
            },
            alloc,
        ),
        closing_element: if self_closing {
            None
        } else {
            Some(OxcBox::new_in(
                JSXClosingElement {
                    node_id: Cell::new(NodeId::DUMMY),
                    span: SPAN,
                    name: create_jsx_name_from_str(alloc, name),
                },
                alloc,
            ))
        },
        children,
    }
}

/// Create a fragment.
fn create_fragment<'a>(
    _alloc: &'a Allocator,
    children: OxcVec<'a, JSXChild<'a>>,
    span: Span,
) -> JSXFragment<'a> {
    JSXFragment {
        node_id: Cell::new(NodeId::DUMMY),
        span,
        opening_fragment: JSXOpeningFragment {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
        },
        closing_fragment: JSXClosingFragment {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
        },
        children,
    }
}

/// Turn a hast property into something that particularly React understands.
fn prop_to_attr_name(prop: &str) -> String {
    // Arbitrary data props, kebab case them.
    if prop.len() > 4 && prop.starts_with("data") {
        let mut result = String::with_capacity(prop.len() + 2);
        let bytes = prop.as_bytes();
        let mut index = 4;
        let mut start = index;
        let mut valid = true;

        result.push_str("data");

        while index < bytes.len() {
            let byte = bytes[index];
            let mut dash = index == 4;

            match byte {
                b'A'..=b'Z' => dash = true,
                b'-' | b'.' | b':' | b'0'..=b'9' | b'a'..=b'z' => {}
                _ => {
                    valid = false;
                    break;
                }
            }

            if dash {
                result.push_str(&prop[start..index]);
                if byte != b'-' {
                    result.push('-');
                }
                result.push(byte.to_ascii_lowercase().into());
                start = index + 1;
            }

            index += 1;
        }

        if valid {
            result.push_str(&prop[start..]);
            return result;
        }
    }

    PROP_TO_REACT_PROP
        .iter()
        .find(|d| d.0 == prop)
        .or_else(|| PROP_TO_ATTR_EXCEPTIONS_SHARED.iter().find(|d| d.0 == prop))
        .map_or_else(|| prop.into(), |d| d.1.into())
}

const PROP_TO_REACT_PROP: [(&str, &str); 17] = [
    ("classId", "classID"),
    ("dataType", "datatype"),
    ("itemId", "itemID"),
    ("strokeDashArray", "strokeDasharray"),
    ("strokeDashOffset", "strokeDashoffset"),
    ("strokeLineCap", "strokeLinecap"),
    ("strokeLineJoin", "strokeLinejoin"),
    ("strokeMiterLimit", "strokeMiterlimit"),
    ("typeOf", "typeof"),
    ("xLinkActuate", "xlinkActuate"),
    ("xLinkArcRole", "xlinkArcrole"),
    ("xLinkHref", "xlinkHref"),
    ("xLinkRole", "xlinkRole"),
    ("xLinkShow", "xlinkShow"),
    ("xLinkTitle", "xlinkTitle"),
    ("xLinkType", "xlinkType"),
    ("xmlnsXLink", "xmlnsXlink"),
];

const PROP_TO_ATTR_EXCEPTIONS_SHARED: [(&str, &str); 48] = [
    ("ariaActiveDescendant", "aria-activedescendant"),
    ("ariaAtomic", "aria-atomic"),
    ("ariaAutoComplete", "aria-autocomplete"),
    ("ariaBusy", "aria-busy"),
    ("ariaChecked", "aria-checked"),
    ("ariaColCount", "aria-colcount"),
    ("ariaColIndex", "aria-colindex"),
    ("ariaColSpan", "aria-colspan"),
    ("ariaControls", "aria-controls"),
    ("ariaCurrent", "aria-current"),
    ("ariaDescribedBy", "aria-describedby"),
    ("ariaDetails", "aria-details"),
    ("ariaDisabled", "aria-disabled"),
    ("ariaDropEffect", "aria-dropeffect"),
    ("ariaErrorMessage", "aria-errormessage"),
    ("ariaExpanded", "aria-expanded"),
    ("ariaFlowTo", "aria-flowto"),
    ("ariaGrabbed", "aria-grabbed"),
    ("ariaHasPopup", "aria-haspopup"),
    ("ariaHidden", "aria-hidden"),
    ("ariaInvalid", "aria-invalid"),
    ("ariaKeyShortcuts", "aria-keyshortcuts"),
    ("ariaLabel", "aria-label"),
    ("ariaLabelledBy", "aria-labelledby"),
    ("ariaLevel", "aria-level"),
    ("ariaLive", "aria-live"),
    ("ariaModal", "aria-modal"),
    ("ariaMultiLine", "aria-multiline"),
    ("ariaMultiSelectable", "aria-multiselectable"),
    ("ariaOrientation", "aria-orientation"),
    ("ariaOwns", "aria-owns"),
    ("ariaPlaceholder", "aria-placeholder"),
    ("ariaPosInSet", "aria-posinset"),
    ("ariaPressed", "aria-pressed"),
    ("ariaReadOnly", "aria-readonly"),
    ("ariaRelevant", "aria-relevant"),
    ("ariaRequired", "aria-required"),
    ("ariaRoleDescription", "aria-roledescription"),
    ("ariaRowCount", "aria-rowcount"),
    ("ariaRowIndex", "aria-rowindex"),
    ("ariaRowSpan", "aria-rowspan"),
    ("ariaSelected", "aria-selected"),
    ("ariaSetSize", "aria-setsize"),
    ("ariaSort", "aria-sort"),
    ("ariaValueMax", "aria-valuemax"),
    ("ariaValueMin", "aria-valuemin"),
    ("ariaValueNow", "aria-valuenow"),
    ("ariaValueText", "aria-valuetext"),
];
