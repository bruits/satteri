//! Turn a HAST arena into a JavaScript AST.
//!
//! Reads from an `Arena` (owned arena).

use crate::configuration::{ElementAttributeNameCase, OptimizeStaticConfig, StylePropertyNameCase};
use crate::oxc::{parse_esm_to_tree, parse_expression_to_tree, serialize};
use crate::oxc_utils::{
    create_jsx_attr_name_from_str, create_jsx_name_from_str, create_object_expression,
    create_prop_name, create_string_literal, inter_element_whitespace, is_literal_name,
};
use core::str;
use std::cell::Cell;
use std::rc::Rc;

use oxc_allocator::{Allocator, Box as OxcBox, Vec as OxcVec};
use oxc_ast::ast::{
    BindingPattern, Declaration, Expression, ExpressionStatement, JSXAttribute, JSXAttributeItem,
    JSXAttributeValue, JSXChild, JSXClosingElement, JSXClosingFragment, JSXElement,
    JSXEmptyExpression, JSXExpression, JSXExpressionContainer, JSXFragment, JSXOpeningElement,
    JSXOpeningFragment, JSXSpreadAttribute, ObjectProperty, ObjectPropertyKind, Program,
    PropertyKey, PropertyKind, Statement, StringLiteral, VariableDeclarationKind,
};
use oxc_span::{Atom, SPAN, Span};
use oxc_syntax::node::NodeId;
use rustc_hash::{FxHashMap, FxHashSet};
use satteri_arena::mdx_types::{self as message, Location, MdxExpressionKind, Stop};
use satteri_arena::{Arena, Hast};
use satteri_ast::hast::HastNodeType;
use satteri_ast::hast::codec::{
    decode_element_prop, decode_element_prop_count, decode_element_tag, decode_text_data,
};
use satteri_ast::mdast::codec::{
    decode_mdx_jsx_attr, decode_mdx_jsx_attr_count, decode_mdx_jsx_element_name,
    decode_mdx_jsx_explicit,
};
use satteri_ast::shared::{
    MDX_ATTR_BOOLEAN_PROP, MDX_ATTR_EXPRESSION_PROP, MDX_ATTR_LITERAL_PROP, MDX_ATTR_SPREAD,
    PROP_BOOL_TRUE, PROP_COMMA_SEP, PROP_INT, PROP_SPACE_SEP, PROP_STRING,
};

/// Get a Span from a HAST binary node's position data.
/// Uses offset+1 convention so that (0,0) remains SPAN (dummy).
fn node_span(view: &Arena<Hast>, node_id: u32) -> Span {
    let node = view.get_node(node_id);
    if node.start_offset == 0 && node.end_offset == 0 && node.start_line == 0 {
        SPAN
    } else {
        Span::new(node.start_offset + 1, node.end_offset + 1)
    }
}

/// Absolute source byte offset where a node's verbatim text begins, for use as
/// an oxc parse stop (`relative 0 -> this offset`) so parse errors over that
/// text resolve to a source line/column. `None` for synthetic nodes that carry
/// no real span. Only valid when the node's stored text matches the source
/// byte-for-byte, as MDX ESM does.
fn node_source_offset(view: &Arena<Hast>, node_id: u32) -> Option<usize> {
    let node = view.get_node(node_id);
    if node.start_offset == 0 && node.end_offset == 0 && node.start_line == 0 {
        None
    } else {
        Some(node.start_offset as usize)
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
    view: &'a Arena<Hast>,
    /// Behind `Rc` because `all()` needs to hold the config while mutably
    /// re-borrowing the rest of Context for recursive `one()` calls.
    optimize_static: Option<Rc<OptimizeStaticConfig>>,
    /// Populated by the component-override prepass so `transform_mdxjs_esm`
    /// can reuse already-parsed programs instead of parsing the same source twice.
    pre_parsed_esm: FxHashMap<u32, Program<'a>>,
    element_attribute_name_case: ElementAttributeNameCase,
    style_property_name_case: StylePropertyNameCase,
}

/// Compile a HAST into OXC's ES AST.
#[allow(clippy::too_many_arguments)]
pub fn hast_util_to_oxc<'a>(
    view: &'a Arena<Hast>,
    path: Option<String>,
    location: Option<&'a Location>,
    explicit_jsxs: &mut FxHashSet<Span>,
    allocator: &'a Allocator,
    optimize_static: Option<&OptimizeStaticConfig>,
    element_attribute_name_case: ElementAttributeNameCase,
    style_property_name_case: StylePropertyNameCase,
) -> Result<MdxProgram<'a>, message::Message> {
    let (effective_optimize_static, pre_parsed_esm) =
        prepare_component_overrides(view, allocator, location, optimize_static)?;

    let mut context = Context {
        space: Space::Html,
        comments: vec![],
        esm: vec![],
        location,
        allocator,
        view,
        optimize_static: effective_optimize_static,
        pre_parsed_esm,
        element_attribute_name_case,
        style_property_name_case,
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

/// Pre-scan for `export const components = { … }` and merge detected keys
/// into `ignore_elements` so the optimizer doesn't collapse overridden elements.
///
/// Parsed programs are returned alongside the config so `transform_mdxjs_esm`
/// can consume them later, avoiding a second OXC parse of the same source.
type ComponentOverridePrepass<'a> = (
    Option<Rc<OptimizeStaticConfig>>,
    FxHashMap<u32, Program<'a>>,
);

fn prepare_component_overrides<'a>(
    view: &'a Arena<Hast>,
    allocator: &'a Allocator,
    location: Option<&Location>,
    optimize_static: Option<&OptimizeStaticConfig>,
) -> Result<ComponentOverridePrepass<'a>, message::Message> {
    let mut pre_parsed: FxHashMap<u32, Program<'a>> = FxHashMap::default();
    let Some(config) = optimize_static else {
        return Ok((None, pre_parsed));
    };
    if !view.source().contains("export const components") {
        return Ok((Some(Rc::new(config.clone())), pre_parsed));
    }

    let mut collected_keys: Vec<String> = Vec::new();
    let mut found_declaration = false;

    for &child_id in view.get_children(0) {
        let node = view.get_node(child_id);
        if HastNodeType::from_u8(node.node_type) != Some(HastNodeType::MdxEsm) {
            continue;
        }
        let data = view.get_type_data(child_id);
        if data.len() < 8 {
            continue;
        }
        let value = view.get_str(decode_text_data(data));
        if !value.contains("export const components") {
            continue;
        }

        let stops_buf;
        let stops: &[Stop] = match node_source_offset(view, child_id) {
            Some(off) => {
                stops_buf = [(0, off)];
                &stops_buf
            }
            None => &[],
        };
        let program = parse_esm_to_tree(value, stops, location, allocator)?;
        if !found_declaration {
            found_declaration = extract_component_override_keys(&program, &mut collected_keys);
        }
        pre_parsed.insert(child_id, program);
    }

    if collected_keys.is_empty() {
        return Ok((Some(Rc::new(config.clone())), pre_parsed));
    }

    let mut merged = config.clone();
    merged.ignore_elements.extend(collected_keys);
    Ok((Some(Rc::new(merged)), pre_parsed))
}

/// Extract identifier keys from `export const components = { … }`.
///
/// Returns `true` if a `components` declarator was found (even with zero
/// usable keys, e.g. only spreads) so the caller can stop scanning further
/// ESM blocks.
fn extract_component_override_keys(program: &Program<'_>, keys: &mut Vec<String>) -> bool {
    for stmt in &program.body {
        let Statement::ExportNamedDeclaration(export_decl) = stmt else {
            continue;
        };
        let Some(Declaration::VariableDeclaration(var_decl)) = &export_decl.declaration else {
            continue;
        };
        if var_decl.kind != VariableDeclarationKind::Const {
            continue;
        }
        for declarator in &var_decl.declarations {
            let BindingPattern::BindingIdentifier(ident) = &declarator.id else {
                continue;
            };
            if ident.name.as_str() != "components" {
                continue;
            }
            let Some(Expression::ObjectExpression(obj)) = &declarator.init else {
                continue;
            };
            for prop in &obj.properties {
                let ObjectPropertyKind::ObjectProperty(p) = prop else {
                    continue;
                };
                if p.computed {
                    continue;
                }
                let PropertyKey::StaticIdentifier(key_ident) = &p.key else {
                    continue;
                };
                keys.push(key_ident.name.to_string());
            }
            return true;
        }
    }
    false
}

/// Transform one node.
fn one<'a>(
    context: &mut Context<'a>,
    node_id: u32,
    explicit_jsxs: &mut FxHashSet<Span>,
) -> Result<Option<JSXChild<'a>>, message::Message> {
    let node = context.view.get_node(node_id);
    let raw_type = node.node_type;

    match HastNodeType::from_u8(raw_type) {
        Some(HastNodeType::Root) => transform_root(context, node_id, explicit_jsxs),
        Some(HastNodeType::Element) => transform_element(context, node_id, explicit_jsxs),
        Some(HastNodeType::Text | HastNodeType::Raw) => Ok(transform_text(context, node_id)),
        Some(HastNodeType::Comment) => Ok(Some(transform_comment(context, node_id))),
        Some(HastNodeType::MdxJsxElement | HastNodeType::MdxJsxTextElement) => {
            transform_mdx_jsx_element(context, node_id, explicit_jsxs)
        }
        Some(HastNodeType::MdxFlowExpression | HastNodeType::MdxTextExpression) => {
            transform_mdx_expression(context, node_id)
        }
        Some(HastNodeType::MdxEsm) => transform_mdxjs_esm(context, node_id),
        _ => Ok(None),
    }
}

/// Check if a HAST subtree is fully static (only HTML elements, text, comments, raw).
///
/// Returns `false` for any subtree containing MDX nodes (components, expressions, ESM)
/// or elements whose tag name is in the ignore list.
fn is_static_subtree(view: &Arena<Hast>, node_id: u32, config: &OptimizeStaticConfig) -> bool {
    let node = view.get_node(node_id);
    let raw_type = node.node_type;

    match HastNodeType::from_u8(raw_type) {
        Some(HastNodeType::Text | HastNodeType::Raw | HastNodeType::Comment) => true,
        Some(HastNodeType::Element) => {
            let data = view.get_type_data(node_id);
            if data.len() < 16 {
                return false;
            }
            let tag_ref = decode_element_tag(data);
            let tag = view.get_str(tag_ref);
            if !is_literal_name(tag) {
                return false;
            }
            if config.ignore_elements.iter().any(|s| s == tag) {
                return false;
            }
            view.get_children(node_id)
                .iter()
                .all(|&cid| is_static_subtree(view, cid, config))
        }
        // MDX nodes, root, or anything else → not static
        // (Plain MDX JSX elements like <kbd> should be simplified to HAST_ELEMENT
        // by simplify_plain_mdx_nodes() before reaching this point.)
        _ => false,
    }
}

/// Try to render a static subtree to an HTML string. Returns false if not static.
fn try_render_static(
    view: &Arena<Hast>,
    node_id: u32,
    config: &OptimizeStaticConfig,
    out: &mut String,
    space: Space,
) -> bool {
    if !is_static_subtree(view, node_id, config) {
        return false;
    }
    satteri_ast::hast::render_node(node_id, view, out, false, space == Space::Svg);
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

    if let Some(config) = context.optimize_static.clone() {
        // Optimization enabled: group consecutive static siblings into raw HTML.
        let config = &*config;
        let mut i = 0;
        while i < child_count {
            let child_id = context.view.get_children(parent_id)[i];

            let mut html_buf = String::new();
            if try_render_static(context.view, child_id, config, &mut html_buf, context.space) {
                // Accumulate consecutive static siblings
                i += 1;
                while i < child_count {
                    let next_id = context.view.get_children(parent_id)[i];
                    if !try_render_static(
                        context.view,
                        next_id,
                        config,
                        &mut html_buf,
                        context.space,
                    ) {
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
    let in_svg = context.space == Space::Svg;
    let attr_case = context.element_attribute_name_case;
    let style_case = context.style_property_name_case;
    for i in 0..prop_count {
        let (name_ref, value_kind, value_ref) = decode_element_prop(data, i);
        let name = context.view.get_str(name_ref);

        // `style="…"` parses into a JSX expression object regardless of
        // attribute-name casing; key casing is controlled separately.
        if name == "style" && matches!(value_kind, PROP_STRING | PROP_SPACE_SEP | PROP_COMMA_SEP) {
            let raw = context.view.get_str(value_ref);
            let object = build_style_object(alloc, raw, style_case);
            attrs.push(JSXAttributeItem::Attribute(OxcBox::new_in(
                JSXAttribute {
                    node_id: Cell::new(NodeId::DUMMY),
                    span: SPAN,
                    name: create_jsx_attr_name_from_str(alloc, "style"),
                    value: Some(JSXAttributeValue::ExpressionContainer(OxcBox::new_in(
                        JSXExpressionContainer {
                            node_id: Cell::new(NodeId::DUMMY),
                            span: SPAN,
                            expression: JSXExpression::from(object),
                        },
                        alloc,
                    ))),
                },
                alloc,
            )));
            continue;
        }

        let value = match value_kind {
            PROP_BOOL_TRUE => None,
            PROP_STRING | PROP_INT | PROP_SPACE_SEP | PROP_COMMA_SEP => {
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

        let attr_name = match attr_case {
            ElementAttributeNameCase::React => prop_to_attr_name(name),
            ElementAttributeNameCase::Html => {
                satteri_ast::hast::properties::property_to_attribute(name, in_svg).into_owned()
            }
        };
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
                let raw_value = context.view.get_str(attr_value_ref);
                // Drop phantom-space sentinels (U+F002, see
                // `satteri-pulldown-cmark::mdx::PHANTOM_SPACE`) before parsing
                // so they don't bleed into template-literal cooked values.
                let owned_buf;
                let expr_value: &str = if raw_value.contains('\u{F002}') {
                    owned_buf = raw_value.replace('\u{F002}', "");
                    &owned_buf
                } else {
                    raw_value
                };
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
    // Fast-path mirror of `node.data._mdxExplicitJsx`; see codec.rs.
    let explicit_jsx = decode_mdx_jsx_explicit(data);
    Ok(Some(if let Some(n) = name {
        if explicit_jsx {
            explicit_jsxs.insert(span);
        }
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
    let raw_value = if data.len() >= 8 {
        context.view.get_str(decode_text_data(data))
    } else {
        ""
    };
    // Drop phantom-space sentinels (U+F002) before handing the expression
    // body to oxc — see `satteri-pulldown-cmark::mdx::PHANTOM_SPACE`.
    let owned_buf;
    let value: &str = if raw_value.contains('\u{F002}') {
        owned_buf = raw_value.replace('\u{F002}', "");
        &owned_buf
    } else {
        raw_value
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
    let alloc = context.allocator;
    let mut program = if let Some(pre) = context.pre_parsed_esm.remove(&node_id) {
        pre
    } else {
        let data = context.view.get_type_data(node_id);
        let value = if data.len() >= 8 {
            context.view.get_str(decode_text_data(data))
        } else {
            ""
        };
        let stops_buf;
        let stops: &[Stop] = match node_source_offset(context.view, node_id) {
            Some(off) => {
                stops_buf = [(0, off)];
                &stops_buf
            }
            None => &[],
        };
        parse_esm_to_tree(value, stops, context.location, alloc)?
    };

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

/// Parse a `style="…"` string into a JSX object expression with one
/// property per declaration. Keys are emitted in DOM casing (default) or
/// CSS casing depending on `case`.
fn build_style_object<'a>(
    alloc: &'a Allocator,
    raw: &str,
    case: StylePropertyNameCase,
) -> Expression<'a> {
    let mut properties = OxcVec::new_in(alloc);
    for (key_css, value) in parse_style_declarations(raw) {
        let key_str = match case {
            StylePropertyNameCase::Dom => css_to_dom_case(&key_css),
            StylePropertyNameCase::Css => key_css,
        };
        let key = create_prop_name(alloc, &key_str);
        let value_expr =
            Expression::StringLiteral(OxcBox::new_in(create_string_literal(alloc, &value), alloc));
        properties.push(ObjectPropertyKind::ObjectProperty(OxcBox::new_in(
            ObjectProperty {
                node_id: Cell::new(NodeId::DUMMY),
                span: SPAN,
                kind: PropertyKind::Init,
                key,
                value: value_expr,
                shorthand: false,
                method: false,
                computed: false,
            },
            alloc,
        )));
    }
    create_object_expression(alloc, properties)
}

/// Split a CSS declaration list into `(property, value)` pairs in their CSS
/// (kebab-cased / vendor-prefixed) form. Respects single/double quotes and
/// parenthesised groups (e.g. `url(...)`) so semicolons inside them aren't
/// treated as separators.
fn parse_style_declarations(input: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut buf = String::new();
    let mut quote: Option<char> = None;
    let mut paren_depth: u32 = 0;
    let bytes = input.as_bytes();

    let push_decl = |buf: &mut String, out: &mut Vec<(String, String)>| {
        let decl = buf.trim();
        if !decl.is_empty()
            && let Some(colon) = decl.find(':')
        {
            let name = decl[..colon].trim();
            // Standard CSS property names are case-insensitive, but custom
            // properties (`--*`) are case-sensitive — `--tmLabel` must not
            // become `--tmlabel`, or `var(--tmLabel)` references break.
            let property = if name.starts_with("--") {
                name.to_string()
            } else {
                name.to_ascii_lowercase()
            };
            let value = decl[colon + 1..].trim().to_string();
            if !property.is_empty() && !value.is_empty() {
                out.push((property, value));
            }
        }
        buf.clear();
    };

    for &b in bytes {
        let c = b as char;
        if let Some(q) = quote {
            buf.push(c);
            if c == q {
                quote = None;
            }
        } else {
            match c {
                '"' | '\'' => {
                    quote = Some(c);
                    buf.push(c);
                }
                '(' => {
                    paren_depth += 1;
                    buf.push(c);
                }
                ')' => {
                    paren_depth = paren_depth.saturating_sub(1);
                    buf.push(c);
                }
                ';' if paren_depth == 0 => {
                    push_decl(&mut buf, &mut out);
                }
                _ => buf.push(c),
            }
        }
    }
    push_decl(&mut buf, &mut out);
    out
}

/// CSS kebab/vendor-prefixed property name to DOM (React) casing.
/// `background-color` becomes `backgroundColor`, `-webkit-line-clamp` becomes
/// `WebkitLineClamp`, `-ms-transform` becomes `msTransform` (lowercase `ms`
/// is a React quirk), and `--my-var` is preserved.
fn css_to_dom_case(s: &str) -> String {
    if s.starts_with("--") {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    let mut capitalize_next;
    if let Some(rest) = s.strip_prefix("-ms-") {
        // React quirk: -ms-* keeps lowercase `ms`, not `Ms`.
        out.push_str("ms");
        chars = rest.chars();
        capitalize_next = true;
    } else if let Some(rest) = s.strip_prefix('-') {
        chars = rest.chars();
        capitalize_next = true;
    } else {
        capitalize_next = false;
    }
    for c in chars {
        if c == '-' {
            capitalize_next = true;
        } else if capitalize_next {
            for u in c.to_uppercase() {
                out.push(u);
            }
            capitalize_next = false;
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_to_dom_basic() {
        assert_eq!(css_to_dom_case("color"), "color");
        assert_eq!(css_to_dom_case("background-color"), "backgroundColor");
        assert_eq!(css_to_dom_case("text-align"), "textAlign");
    }

    #[test]
    fn css_to_dom_vendor_prefixes() {
        // `-webkit-`, `-moz-`, `-o-` capitalize the prefix.
        assert_eq!(css_to_dom_case("-webkit-line-clamp"), "WebkitLineClamp");
        assert_eq!(css_to_dom_case("-moz-user-select"), "MozUserSelect");
        // `-ms-` is the React quirk: lowercase `ms`.
        assert_eq!(css_to_dom_case("-ms-transform"), "msTransform");
    }

    #[test]
    fn css_to_dom_custom_property() {
        assert_eq!(css_to_dom_case("--my-var"), "--my-var");
        assert_eq!(css_to_dom_case("--theme-color-1"), "--theme-color-1");
    }

    #[test]
    fn parse_style_simple() {
        let pairs = parse_style_declarations("color: red; font-size: 14px");
        assert_eq!(
            pairs,
            vec![
                ("color".to_string(), "red".to_string()),
                ("font-size".to_string(), "14px".to_string())
            ]
        );
    }

    #[test]
    fn parse_style_trailing_semicolon_and_whitespace() {
        let pairs = parse_style_declarations("  color : red ;  ");
        assert_eq!(pairs, vec![("color".to_string(), "red".to_string())]);
    }

    #[test]
    fn parse_style_respects_quotes_and_parens() {
        // Semicolons inside `url(...)` and quoted strings must not split.
        let pairs =
            parse_style_declarations(r#"background: url("a;b.png"); content: ";"; color: red"#);
        assert_eq!(
            pairs,
            vec![
                ("background".to_string(), r#"url("a;b.png")"#.to_string()),
                ("content".to_string(), r#"";""#.to_string()),
                ("color".to_string(), "red".to_string())
            ]
        );
    }

    #[test]
    fn parse_style_lowercases_property() {
        let pairs = parse_style_declarations("COLOR: red");
        assert_eq!(pairs, vec![("color".to_string(), "red".to_string())]);
    }

    #[test]
    fn parse_style_preserves_custom_property_case() {
        let pairs = parse_style_declarations("--tmLabel: 'a'; COLOR: red");
        assert_eq!(
            pairs,
            vec![
                ("--tmLabel".to_string(), "'a'".to_string()),
                ("color".to_string(), "red".to_string())
            ]
        );
    }
}
