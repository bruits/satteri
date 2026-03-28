//! Lots of helpers for dealing with OXC, particularly from unist, and for
//! building its ES AST.

use tryckeri_mdast::mdx_types::{Location, Point, Position, id_cont, id_start};

use std::cell::Cell;

use oxc_allocator::{Allocator, Box as OxcBox, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, BindingIdentifier, BooleanLiteral, CallExpression, ComputedMemberExpression,
    Expression, IdentifierName, IdentifierReference, JSXAttributeName, JSXElementName,
    JSXIdentifier, JSXMemberExpression, JSXMemberExpressionObject, JSXNamespacedName,
    MemberExpression, NullLiteral, NumericLiteral, ObjectExpression, ObjectPropertyKind,
    PropertyKey, StaticMemberExpression, StringLiteral, ThisExpression,
};
use oxc_span::{Atom, SPAN, Span};
use oxc_syntax::node::NodeId;

/// Turn an OXC span, of two byte positions, into a unist position.
///
/// This assumes the span comes from a fixed tree, or is a dummy.
pub fn span_to_position(span: Span, location: Option<&Location>) -> Option<Position> {
    let lo = span.start as usize;
    let hi = span.end as usize;

    if lo > 0
        && hi > 0
        && let Some(location) = location
        && let Some(start) = location.to_point(lo - 1)
        && let Some(end) = location.to_point(hi - 1)
    {
        return Some(Position { start, end });
    }

    None
}

/// Turn an OXC byte position into a unist point.
///
/// This assumes the byte position comes from a fixed tree, or is a dummy.
pub fn u32_to_point(pos: u32, location: Option<&Location>) -> Option<Point> {
    let pos = pos as usize;

    if pos > 0
        && let Some(location) = location
    {
        return location.to_point(pos - 1);
    }

    None
}

/// Serialize a unist position for humans.
pub fn position_opt_to_string(position: Option<&Position>) -> String {
    if let Some(position) = position {
        position_to_string(position)
    } else {
        "0:0".into()
    }
}

/// Serialize a unist position for humans.
pub fn position_to_string(position: &Position) -> String {
    format!(
        "{}-{}",
        point_to_string(&position.start),
        point_to_string(&position.end)
    )
}

/// Serialize a unist point for humans.
pub fn point_to_string(point: &Point) -> String {
    format!("{}:{}", point.line, point.column)
}

/// Generate an ident name.
///
/// ```js
/// a
/// ```
pub fn create_ident_name<'a>(alloc: &'a Allocator, sym: &str) -> IdentifierName<'a> {
    IdentifierName {
        node_id: Cell::new(NodeId::DUMMY),
        span: SPAN,
        name: Atom::from(alloc.alloc_str(sym)).into(),
    }
}

/// Generate an identifier expression.
///
/// ```js
/// a
/// ```
pub fn create_ident_expression<'a>(alloc: &'a Allocator, sym: &str) -> Expression<'a> {
    Expression::Identifier(OxcBox::new_in(
        IdentifierReference {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            name: Atom::from(alloc.alloc_str(sym)).into(),
            reference_id: Cell::default(),
        },
        alloc,
    ))
}

/// Generate a binding identifier.
pub fn create_binding_ident<'a>(alloc: &'a Allocator, sym: &str) -> BindingIdentifier<'a> {
    BindingIdentifier {
        node_id: Cell::new(NodeId::DUMMY),
        span: SPAN,
        name: Atom::from(alloc.alloc_str(sym)).into(),
        symbol_id: Cell::default(),
    }
}

/// Generate a null expression.
pub fn create_null_expression(alloc: &Allocator) -> Expression<'_> {
    Expression::NullLiteral(OxcBox::new_in(
        NullLiteral {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
        },
        alloc,
    ))
}

/// Generate a str expression.
pub fn create_str_expression<'a>(alloc: &'a Allocator, value: &str) -> Expression<'a> {
    Expression::StringLiteral(OxcBox::new_in(
        StringLiteral {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            value: Atom::from(alloc.alloc_str(value)),
            raw: None,
            lone_surrogates: false,
        },
        alloc,
    ))
}

/// Generate a string literal.
pub fn create_string_literal<'a>(alloc: &'a Allocator, value: &str) -> StringLiteral<'a> {
    StringLiteral {
        node_id: Cell::new(NodeId::DUMMY),
        span: SPAN,
        value: Atom::from(alloc.alloc_str(value)),
        raw: None,
        lone_surrogates: false,
    }
}

/// Generate a bool expression.
pub fn create_bool_expression(alloc: &Allocator, value: bool) -> Expression<'_> {
    Expression::BooleanLiteral(OxcBox::new_in(
        BooleanLiteral {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            value,
        },
        alloc,
    ))
}

/// Generate a num expression.
pub fn create_num_expression(alloc: &Allocator, value: f64) -> Expression<'_> {
    Expression::NumericLiteral(OxcBox::new_in(
        NumericLiteral {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            value,
            raw: None,
            base: oxc_syntax::number::NumberBase::Decimal,
        },
        alloc,
    ))
}

/// Generate an object expression.
pub fn create_object_expression<'a>(
    alloc: &'a Allocator,
    properties: OxcVec<'a, ObjectPropertyKind<'a>>,
) -> Expression<'a> {
    Expression::ObjectExpression(OxcBox::new_in(
        ObjectExpression {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            properties,
        },
        alloc,
    ))
}

/// Generate a call expression.
pub fn create_call_expression<'a>(
    alloc: &'a Allocator,
    callee: Expression<'a>,
    arguments: OxcVec<'a, Argument<'a>>,
) -> Expression<'a> {
    Expression::CallExpression(OxcBox::new_in(
        CallExpression {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            callee,
            type_arguments: None,
            arguments,
            optional: false,
            pure: false,
        },
        alloc,
    ))
}

/// Generate a member expression from a string.
///
/// ```js
/// a.b
/// a
/// ```
pub fn create_member_expression_from_str<'a>(alloc: &'a Allocator, name: &str) -> Expression<'a> {
    match parse_js_name(name) {
        // `a`
        JsName::Normal(name) => create_ident_expression(alloc, name),
        // `a.b.c`
        JsName::Member(parts) => {
            let mut expr = create_ident_expression(alloc, parts[0]);
            let mut index = 1;
            while index < parts.len() {
                expr = create_member(alloc, expr, parts[index]);
                index += 1;
            }
            expr
        }
    }
}

/// Generate a member expression from an object and string prop.
pub fn create_member<'a>(alloc: &'a Allocator, obj: Expression<'a>, prop: &str) -> Expression<'a> {
    if is_identifier_name(prop) {
        Expression::from(MemberExpression::StaticMemberExpression(OxcBox::new_in(
            StaticMemberExpression {
                node_id: Cell::new(NodeId::DUMMY),
                object: obj,
                property: create_ident_name(alloc, prop),
                optional: false,
                span: SPAN,
            },
            alloc,
        )))
    } else {
        Expression::from(MemberExpression::ComputedMemberExpression(OxcBox::new_in(
            ComputedMemberExpression {
                node_id: Cell::new(NodeId::DUMMY),
                object: obj,
                expression: create_str_expression(alloc, prop),
                optional: false,
                span: SPAN,
            },
            alloc,
        )))
    }
}

/// Generate a JSX element name from a string.
///
/// ```js
/// a.b-c
/// a
/// ```
pub fn create_jsx_name_from_str<'a>(alloc: &'a Allocator, name: &str) -> JSXElementName<'a> {
    match parse_jsx_name(name) {
        // `a`
        JsxName::Normal(name) => {
            if is_identifier_name(name)
                && name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
            {
                JSXElementName::Identifier(OxcBox::new_in(
                    JSXIdentifier {
                        node_id: Cell::new(NodeId::DUMMY),
                        span: SPAN,
                        name: Atom::from(alloc.alloc_str(name)),
                    },
                    alloc,
                ))
            } else {
                JSXElementName::IdentifierReference(OxcBox::new_in(
                    IdentifierReference {
                        node_id: Cell::new(NodeId::DUMMY),
                        span: SPAN,
                        name: Atom::from(alloc.alloc_str(name)).into(),
                        reference_id: Cell::default(),
                    },
                    alloc,
                ))
            }
        }
        // `a:b`
        JsxName::Namespace(ns, name) => JSXElementName::NamespacedName(OxcBox::new_in(
            JSXNamespacedName {
                node_id: Cell::new(NodeId::DUMMY),
                span: SPAN,
                namespace: JSXIdentifier {
                    node_id: Cell::new(NodeId::DUMMY),
                    span: SPAN,
                    name: Atom::from(alloc.alloc_str(ns)),
                },
                name: JSXIdentifier {
                    node_id: Cell::new(NodeId::DUMMY),
                    span: SPAN,
                    name: Atom::from(alloc.alloc_str(name)),
                },
            },
            alloc,
        )),
        // `a.b.c`
        JsxName::Member(parts) => {
            let mut obj = JSXMemberExpressionObject::IdentifierReference(OxcBox::new_in(
                IdentifierReference {
                    node_id: Cell::new(NodeId::DUMMY),
                    span: SPAN,
                    name: Atom::from(alloc.alloc_str(parts[0])).into(),
                    reference_id: Cell::default(),
                },
                alloc,
            ));
            let mut index = 1;
            while index < parts.len() - 1 {
                obj = JSXMemberExpressionObject::MemberExpression(OxcBox::new_in(
                    JSXMemberExpression {
                        node_id: Cell::new(NodeId::DUMMY),
                        span: SPAN,
                        object: obj,
                        property: JSXIdentifier {
                            node_id: Cell::new(NodeId::DUMMY),
                            span: SPAN,
                            name: Atom::from(alloc.alloc_str(parts[index])),
                        },
                    },
                    alloc,
                ));
                index += 1;
            }
            JSXElementName::MemberExpression(OxcBox::new_in(
                JSXMemberExpression {
                    node_id: Cell::new(NodeId::DUMMY),
                    span: SPAN,
                    object: obj,
                    property: JSXIdentifier {
                        node_id: Cell::new(NodeId::DUMMY),
                        span: SPAN,
                        name: Atom::from(alloc.alloc_str(parts[parts.len() - 1])),
                    },
                },
                alloc,
            ))
        }
    }
}

/// Create a JSX attribute name.
pub fn create_jsx_attr_name_from_str<'a>(alloc: &'a Allocator, name: &str) -> JSXAttributeName<'a> {
    match parse_jsx_name(name) {
        JsxName::Member(_) => {
            unreachable!("member expressions in attribute names are not supported")
        }
        // `<a b:c />`
        JsxName::Namespace(ns, name) => JSXAttributeName::NamespacedName(OxcBox::new_in(
            JSXNamespacedName {
                node_id: Cell::new(NodeId::DUMMY),
                span: SPAN,
                namespace: JSXIdentifier {
                    node_id: Cell::new(NodeId::DUMMY),
                    span: SPAN,
                    name: Atom::from(alloc.alloc_str(ns)),
                },
                name: JSXIdentifier {
                    node_id: Cell::new(NodeId::DUMMY),
                    span: SPAN,
                    name: Atom::from(alloc.alloc_str(name)),
                },
            },
            alloc,
        )),
        // `<a b />`
        JsxName::Normal(name) => JSXAttributeName::Identifier(OxcBox::new_in(
            JSXIdentifier {
                node_id: Cell::new(NodeId::DUMMY),
                span: SPAN,
                name: Atom::from(alloc.alloc_str(name)),
            },
            alloc,
        )),
    }
}

/// Turn a JSX element name into an expression.
pub fn jsx_element_name_to_expression<'a>(
    alloc: &'a Allocator,
    node: &JSXElementName<'a>,
) -> Expression<'a> {
    match node {
        JSXElementName::MemberExpression(member_expr) => {
            jsx_member_expression_to_expression(alloc, member_expr)
        }
        JSXElementName::NamespacedName(namespace_name) => create_str_expression(
            alloc,
            &format!(
                "{}:{}",
                namespace_name.namespace.name, namespace_name.name.name
            ),
        ),
        JSXElementName::Identifier(ident) => create_ident_or_literal(alloc, &ident.name),
        JSXElementName::IdentifierReference(ident) => create_ident_or_literal(alloc, &ident.name),
        JSXElementName::ThisExpression(_) => Expression::ThisExpression(OxcBox::new_in(
            ThisExpression {
                node_id: Cell::new(NodeId::DUMMY),
                span: SPAN,
            },
            alloc,
        )),
    }
}

/// Turn a JSX member expression name into a member expression.
pub fn jsx_member_expression_to_expression<'a>(
    alloc: &'a Allocator,
    node: &JSXMemberExpression<'a>,
) -> Expression<'a> {
    let obj = jsx_object_to_expression(alloc, &node.object);
    create_member(alloc, obj, node.property.name.as_str())
}

/// Turn a JSX attribute name into a property key.
pub fn jsx_attribute_name_to_prop_name<'a>(
    alloc: &'a Allocator,
    node: &JSXAttributeName<'a>,
) -> PropertyKey<'a> {
    match node {
        JSXAttributeName::NamespacedName(namespace_name) => create_prop_name(
            alloc,
            &format!(
                "{}:{}",
                namespace_name.namespace.name, namespace_name.name.name
            ),
        ),
        JSXAttributeName::Identifier(ident) => create_prop_name(alloc, &ident.name),
    }
}

/// Turn a JSX object into an expression.
pub fn jsx_object_to_expression<'a>(
    alloc: &'a Allocator,
    node: &JSXMemberExpressionObject<'a>,
) -> Expression<'a> {
    match node {
        JSXMemberExpressionObject::IdentifierReference(ident) => {
            create_ident_or_literal(alloc, &ident.name)
        }
        JSXMemberExpressionObject::MemberExpression(member_expr) => {
            jsx_member_expression_to_expression(alloc, member_expr)
        }
        JSXMemberExpressionObject::ThisExpression(_) => Expression::ThisExpression(OxcBox::new_in(
            ThisExpression {
                node_id: Cell::new(NodeId::DUMMY),
                span: SPAN,
            },
            alloc,
        )),
    }
}

/// Create either an ident expression or a literal expression.
pub fn create_ident_or_literal<'a>(alloc: &'a Allocator, name: &str) -> Expression<'a> {
    if is_identifier_name(name) {
        create_ident_expression(alloc, name)
    } else {
        create_str_expression(alloc, name)
    }
}

/// Create a property key.
pub fn create_prop_name<'a>(alloc: &'a Allocator, name: &str) -> PropertyKey<'a> {
    if is_identifier_name(name) {
        PropertyKey::StaticIdentifier(OxcBox::new_in(create_ident_name(alloc, name), alloc))
    } else {
        PropertyKey::StringLiteral(OxcBox::new_in(create_string_literal(alloc, name), alloc))
    }
}

/// Check if a name is a literal tag name or an identifier to a component.
pub fn is_literal_name(name: &str) -> bool {
    matches!(name.as_bytes().first(), Some(b'a'..=b'z')) || !is_identifier_name(name)
}

/// Check if a name is a valid identifier name.
pub fn is_identifier_name(name: &str) -> bool {
    for (index, char) in name.chars().enumerate() {
        if if index == 0 {
            !id_start(char)
        } else {
            !id_cont(char, false)
        } {
            return false;
        }
    }

    true
}

/// Different kinds of JS names.
pub enum JsName<'a> {
    /// Member: `a.b.c`
    Member(Vec<&'a str>),
    /// Name: `a`
    Normal(&'a str),
}

/// Different kinds of JSX names.
pub enum JsxName<'a> {
    /// Member: `a.b.c`
    Member(Vec<&'a str>),
    /// Namespace: `a:b`
    Namespace(&'a str, &'a str),
    /// Name: `a`
    Normal(&'a str),
}

/// Parse a JavaScript member expression or name.
pub fn parse_js_name(name: &str) -> JsName<'_> {
    let bytes = name.as_bytes();
    let mut index = 0;
    let mut start = 0;
    let mut parts = vec![];

    while index < bytes.len() {
        if bytes[index] == b'.' {
            parts.push(&name[start..index]);
            start = index + 1;
        }

        index += 1;
    }

    // `a`
    if parts.is_empty() {
        JsName::Normal(name)
    }
    // `a.b.c`
    else {
        parts.push(&name[start..]);
        JsName::Member(parts)
    }
}

/// Parse a JSX name from a string.
pub fn parse_jsx_name(name: &str) -> JsxName<'_> {
    match parse_js_name(name) {
        // `<a.b.c />`
        JsName::Member(parts) => JsxName::Member(parts),
        JsName::Normal(name) => {
            // `<a:b />`
            if let Some(colon) = name.as_bytes().iter().position(|d| matches!(d, b':')) {
                JsxName::Namespace(&name[0..colon], &name[(colon + 1)..])
            }
            // `<a />`
            else {
                JsxName::Normal(name)
            }
        }
    }
}

/// Get the identifiers used in a JSX member expression.
///
/// `Foo.Bar` -> `vec!["Foo", "Bar"]`
pub fn jsx_member_to_parts<'a>(node: &'a JSXMemberExpression<'a>) -> Vec<&'a str> {
    let mut parts = vec![];
    let mut member_opt = Some(node);

    while let Some(member) = member_opt {
        parts.push(member.property.name.as_str());
        match &member.object {
            JSXMemberExpressionObject::IdentifierReference(d) => {
                parts.push(d.name.as_str());
                member_opt = None;
            }
            JSXMemberExpressionObject::MemberExpression(node) => {
                member_opt = Some(node);
            }
            JSXMemberExpressionObject::ThisExpression(_) => {
                parts.push("this");
                member_opt = None;
            }
        }
    }

    parts.reverse();
    parts
}

/// Check if a text value is inter-element whitespace.
///
/// See: <https://github.com/syntax-tree/hast-util-whitespace>.
pub fn inter_element_whitespace(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'\t' | 0x0C | b'\r' | b'\n' | b' ' => {}
            _ => return false,
        }
        index += 1;
    }

    true
}
