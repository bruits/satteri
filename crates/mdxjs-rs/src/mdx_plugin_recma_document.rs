//! Turn a JavaScript AST, coming from MD(X), into a component.
//!
//! Port of <https://github.com/mdx-js/mdx/blob/main/packages/mdx/lib/plugin/recma-document.js>,
//! by the same author.

use crate::hast_util_to_oxc::{MdxComment, MdxCommentKind, MdxProgram};
use crate::oxc_utils::{
    create_binding_ident, create_call_expression, create_ident_expression, create_ident_name,
    create_jsx_name_from_str, create_null_expression, create_object_expression,
    create_string_literal, position_opt_to_string, span_to_position, u32_to_point,
};
use std::cell::Cell;

use oxc_allocator::{Allocator, Box as OxcBox, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, AssignmentPattern, BindingPattern, ConditionalExpression, Declaration,
    ExportDefaultDeclaration, ExportDefaultDeclarationKind, Expression, FormalParameter,
    FormalParameterKind, FormalParameters, Function, FunctionBody, FunctionType, ImportDeclaration,
    ImportDeclarationSpecifier, ImportDefaultSpecifier, ImportOrExportKind, ImportSpecifier,
    JSXAttributeItem, JSXChild, JSXClosingElement, JSXElement, JSXOpeningElement,
    JSXSpreadAttribute, ModuleDeclaration, ModuleExportName, ReturnStatement, Statement,
    VariableDeclaration, VariableDeclarationKind, VariableDeclarator,
};
use oxc_span::SPAN;
use oxc_syntax::node::NodeId;
use tryckeri_mdast::mdx_types as message;
use tryckeri_mdast::mdx_types::{Location, Point, Position};

/// JSX runtimes (default: `JsxRuntime::Automatic`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serializable", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serializable", serde(rename_all = "camelCase"))]
pub enum JsxRuntime {
    #[default]
    Automatic,
    Classic,
}

/// Configuration.
#[derive(Debug, PartialEq, Eq)]
pub struct Options {
    pub pragma: Option<String>,
    pub pragma_frag: Option<String>,
    pub pragma_import_source: Option<String>,
    pub jsx_import_source: Option<String>,
    pub jsx_runtime: Option<JsxRuntime>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            pragma: None,
            pragma_frag: None,
            pragma_import_source: None,
            jsx_import_source: None,
            jsx_runtime: Some(JsxRuntime::default()),
        }
    }
}

/// Wrap the ES AST nodes coming from hast into a whole document.
pub fn mdx_plugin_recma_document<'a>(
    program: &mut MdxProgram<'a>,
    options: &Options,
    location: Option<&Location>,
    allocator: &'a Allocator,
) -> Result<(), message::Message> {
    let mut replacements: Vec<Statement<'a>> = vec![];

    // Inject JSX configuration comment.
    if let Some(runtime) = &options.jsx_runtime {
        let mut pragmas = vec![];
        let react = &"react".into();
        let create_element = &"React.createElement".into();
        let fragment = &"React.Fragment".into();

        if *runtime == JsxRuntime::Automatic {
            pragmas.push("@jsxRuntime automatic".into());
            pragmas.push(format!(
                "@jsxImportSource {}",
                if let Some(jsx_import_source) = &options.jsx_import_source {
                    jsx_import_source
                } else {
                    react
                }
            ));
        } else {
            pragmas.push("@jsxRuntime classic".into());
            pragmas.push(format!(
                "@jsx {}",
                if let Some(pragma) = &options.pragma {
                    pragma
                } else {
                    create_element
                }
            ));
            pragmas.push(format!(
                "@jsxFrag {}",
                if let Some(pragma_frag) = &options.pragma_frag {
                    pragma_frag
                } else {
                    fragment
                }
            ));
        }

        if !pragmas.is_empty() {
            program.comments.insert(
                0,
                MdxComment {
                    kind: MdxCommentKind::Block,
                    text: pragmas.join(" "),
                    span: SPAN,
                },
            );
        }
    }

    // Inject an import in the classic runtime for the pragma.
    if options.jsx_runtime == Some(JsxRuntime::Classic) {
        let pragma = if let Some(pragma) = &options.pragma {
            pragma
        } else {
            "React"
        };
        let sym = pragma.split('.').next().expect("first item always exists");

        let source_str = if let Some(source) = &options.pragma_import_source {
            source.as_str()
        } else {
            "react"
        };

        replacements.push(create_import_default(allocator, sym, source_str));
    }

    // Find the `export default`, the JSX expression, and leave the rest as it is.
    let body = std::mem::replace(&mut program.program.body, OxcVec::new_in(allocator));
    let mut input: Vec<Statement<'a>> = body.into_iter().collect();
    input.reverse();
    let mut layout = false;
    let mut layout_position = None;
    let mut content = false;

    while let Some(stmt) = input.pop() {
        match stmt {
            Statement::ExportDefaultDeclaration(decl) => {
                let decl = decl.unbox();
                err_for_double_layout(
                    layout,
                    layout_position.as_ref(),
                    u32_to_point(decl.span.start, location).as_ref(),
                )?;
                layout = true;
                layout_position = span_to_position(decl.span, location);

                match decl.declaration {
                    ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
                        let func = func.unbox();
                        replacements.push(create_layout_decl(
                            allocator,
                            Expression::FunctionExpression(OxcBox::new_in(func, allocator)),
                        ));
                    }
                    ExportDefaultDeclarationKind::ClassDeclaration(cls) => {
                        let cls = cls.unbox();
                        replacements.push(create_layout_decl(
                            allocator,
                            Expression::ClassExpression(OxcBox::new_in(cls, allocator)),
                        ));
                    }
                    ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => {
                        return Err(message::Message {
                            reason: "Cannot use TypeScript interface declarations as default export in MDX files. The default export is reserved for a layout, which must be a component".into(),
                            place: u32_to_point(decl.span.start, location)
                                .map(|p| Box::new(message::Place::Point(p))),
                            source: Box::new("mdxjs-rs".into()),
                            rule_id: Box::new("ts-interface".into()),
                        });
                    }
                    _ => {
                        // It's an expression
                        if let Some(expr) = export_default_kind_to_expression(decl.declaration) {
                            replacements.push(create_layout_decl(allocator, expr));
                        }
                    }
                }
            }
            Statement::ExportNamedDeclaration(named_export) => {
                let mut named_export = named_export.unbox();
                let mut index = 0;
                let mut id_name = None;

                while index < named_export.specifiers.len() {
                    let mut take = false;
                    let spec = &named_export.specifiers[index];
                    if let ModuleExportName::IdentifierName(ident) = &spec.exported
                        && ident.name.as_str() == "default"
                    {
                        match &spec.local {
                            ModuleExportName::IdentifierReference(local_ident) => {
                                err_for_double_layout(
                                    layout,
                                    layout_position.as_ref(),
                                    u32_to_point(local_ident.span.start, location).as_ref(),
                                )?;
                                layout = true;
                                layout_position = span_to_position(local_ident.span, location);
                                take = true;
                                id_name = Some(local_ident.name.to_string());
                            }
                            ModuleExportName::IdentifierName(local_ident) => {
                                err_for_double_layout(
                                    layout,
                                    layout_position.as_ref(),
                                    u32_to_point(local_ident.span.start, location).as_ref(),
                                )?;
                                layout = true;
                                layout_position = span_to_position(local_ident.span, location);
                                take = true;
                                id_name = Some(local_ident.name.to_string());
                            }
                            ModuleExportName::StringLiteral(_) => {}
                        }
                    }

                    if take {
                        named_export.specifiers.remove(index);
                    } else {
                        index += 1;
                    }
                }

                if let Some(name) = id_name {
                    let source = named_export.source.clone();

                    if !named_export.specifiers.is_empty() {
                        replacements.push(Statement::ExportNamedDeclaration(OxcBox::new_in(
                            named_export,
                            allocator,
                        )));
                    }

                    if let Some(source) = source {
                        // `import { name as MDXLayout } from 'source'`
                        replacements.push(create_import_named(
                            allocator,
                            "MDXLayout",
                            &name,
                            &source.value,
                        ));
                    } else {
                        replacements.push(create_layout_decl(
                            allocator,
                            create_ident_expression(allocator, &name),
                        ));
                    }
                } else {
                    replacements.push(Statement::ExportNamedDeclaration(OxcBox::new_in(
                        named_export,
                        allocator,
                    )));
                }
            }
            Statement::ExpressionStatement(ref expr_stmt) => {
                match &expr_stmt.expression {
                    Expression::JSXElement(_) => {
                        content = true;
                        let expr_stmt = if let Statement::ExpressionStatement(e) = stmt {
                            e.unbox().expression
                        } else {
                            unreachable!()
                        };
                        replacements.append(&mut create_mdx_content(
                            allocator,
                            Some(expr_stmt),
                            layout,
                        ));
                    }
                    Expression::JSXFragment(frag) => {
                        // Unwrap if possible.
                        if frag.children.len() == 1
                            && matches!(&frag.children[0], JSXChild::Element(_))
                        {
                            content = true;
                            let expr_stmt = if let Statement::ExpressionStatement(e) = stmt {
                                e.unbox().expression
                            } else {
                                unreachable!()
                            };
                            if let Expression::JSXFragment(mut frag) = expr_stmt {
                                let frag = &mut *frag;
                                let item = frag.children.remove(0);
                                if let JSXChild::Element(elem) = item {
                                    replacements.append(&mut create_mdx_content(
                                        allocator,
                                        Some(Expression::JSXElement(elem)),
                                        layout,
                                    ));
                                    continue;
                                }
                            }
                            unreachable!();
                        }

                        content = true;
                        let expr_stmt = if let Statement::ExpressionStatement(e) = stmt {
                            e.unbox().expression
                        } else {
                            unreachable!()
                        };
                        replacements.append(&mut create_mdx_content(
                            allocator,
                            Some(expr_stmt),
                            layout,
                        ));
                    }
                    _ => {
                        replacements.push(stmt);
                    }
                }
            }
            _ => {
                replacements.push(stmt);
            }
        }
    }

    // Generate an empty component.
    if !content {
        replacements.append(&mut create_mdx_content(allocator, None, layout));
    }

    // `export default MDXContent`
    replacements.push(Statement::ExportDefaultDeclaration(OxcBox::new_in(
        ExportDefaultDeclaration {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            declaration: ExportDefaultDeclarationKind::from(create_ident_expression(
                allocator,
                "MDXContent",
            )),
        },
        allocator,
    )));

    program.program.body = OxcVec::from_iter_in(replacements, allocator);

    Ok(())
}

fn export_default_kind_to_expression(
    kind: ExportDefaultDeclarationKind<'_>,
) -> Option<Expression<'_>> {
    // ExportDefaultDeclarationKind inherits Expression variants via `inherit_variants!`.
    // We match each variant explicitly to convert to the corresponding Expression variant.
    match kind {
        ExportDefaultDeclarationKind::FunctionDeclaration(_)
        | ExportDefaultDeclarationKind::ClassDeclaration(_)
        | ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => None,
        ExportDefaultDeclarationKind::BooleanLiteral(b) => Some(Expression::BooleanLiteral(b)),
        ExportDefaultDeclarationKind::NullLiteral(v) => Some(Expression::NullLiteral(v)),
        ExportDefaultDeclarationKind::NumericLiteral(v) => Some(Expression::NumericLiteral(v)),
        ExportDefaultDeclarationKind::BigIntLiteral(v) => Some(Expression::BigIntLiteral(v)),
        ExportDefaultDeclarationKind::RegExpLiteral(v) => Some(Expression::RegExpLiteral(v)),
        ExportDefaultDeclarationKind::StringLiteral(v) => Some(Expression::StringLiteral(v)),
        ExportDefaultDeclarationKind::TemplateLiteral(v) => Some(Expression::TemplateLiteral(v)),
        ExportDefaultDeclarationKind::Identifier(v) => Some(Expression::Identifier(v)),
        ExportDefaultDeclarationKind::MetaProperty(v) => Some(Expression::MetaProperty(v)),
        ExportDefaultDeclarationKind::Super(v) => Some(Expression::Super(v)),
        ExportDefaultDeclarationKind::ArrayExpression(v) => Some(Expression::ArrayExpression(v)),
        ExportDefaultDeclarationKind::ArrowFunctionExpression(v) => {
            Some(Expression::ArrowFunctionExpression(v))
        }
        ExportDefaultDeclarationKind::AssignmentExpression(v) => {
            Some(Expression::AssignmentExpression(v))
        }
        ExportDefaultDeclarationKind::AwaitExpression(v) => Some(Expression::AwaitExpression(v)),
        ExportDefaultDeclarationKind::BinaryExpression(v) => Some(Expression::BinaryExpression(v)),
        ExportDefaultDeclarationKind::CallExpression(v) => Some(Expression::CallExpression(v)),
        ExportDefaultDeclarationKind::ChainExpression(v) => Some(Expression::ChainExpression(v)),
        ExportDefaultDeclarationKind::ClassExpression(v) => Some(Expression::ClassExpression(v)),
        ExportDefaultDeclarationKind::ConditionalExpression(v) => {
            Some(Expression::ConditionalExpression(v))
        }
        ExportDefaultDeclarationKind::FunctionExpression(v) => {
            Some(Expression::FunctionExpression(v))
        }
        ExportDefaultDeclarationKind::ImportExpression(v) => Some(Expression::ImportExpression(v)),
        ExportDefaultDeclarationKind::LogicalExpression(v) => {
            Some(Expression::LogicalExpression(v))
        }
        ExportDefaultDeclarationKind::NewExpression(v) => Some(Expression::NewExpression(v)),
        ExportDefaultDeclarationKind::ObjectExpression(v) => Some(Expression::ObjectExpression(v)),
        ExportDefaultDeclarationKind::ParenthesizedExpression(v) => {
            Some(Expression::ParenthesizedExpression(v))
        }
        ExportDefaultDeclarationKind::SequenceExpression(v) => {
            Some(Expression::SequenceExpression(v))
        }
        ExportDefaultDeclarationKind::TaggedTemplateExpression(v) => {
            Some(Expression::TaggedTemplateExpression(v))
        }
        ExportDefaultDeclarationKind::ThisExpression(v) => Some(Expression::ThisExpression(v)),
        ExportDefaultDeclarationKind::UnaryExpression(v) => Some(Expression::UnaryExpression(v)),
        ExportDefaultDeclarationKind::UpdateExpression(v) => Some(Expression::UpdateExpression(v)),
        ExportDefaultDeclarationKind::YieldExpression(v) => Some(Expression::YieldExpression(v)),
        ExportDefaultDeclarationKind::PrivateInExpression(v) => {
            Some(Expression::PrivateInExpression(v))
        }
        ExportDefaultDeclarationKind::JSXElement(v) => Some(Expression::JSXElement(v)),
        ExportDefaultDeclarationKind::JSXFragment(v) => Some(Expression::JSXFragment(v)),
        ExportDefaultDeclarationKind::TSAsExpression(v) => Some(Expression::TSAsExpression(v)),
        ExportDefaultDeclarationKind::TSSatisfiesExpression(v) => {
            Some(Expression::TSSatisfiesExpression(v))
        }
        ExportDefaultDeclarationKind::TSTypeAssertion(v) => Some(Expression::TSTypeAssertion(v)),
        ExportDefaultDeclarationKind::TSNonNullExpression(v) => {
            Some(Expression::TSNonNullExpression(v))
        }
        ExportDefaultDeclarationKind::TSInstantiationExpression(v) => {
            Some(Expression::TSInstantiationExpression(v))
        }
        ExportDefaultDeclarationKind::V8IntrinsicExpression(v) => {
            Some(Expression::V8IntrinsicExpression(v))
        }
        ExportDefaultDeclarationKind::ComputedMemberExpression(v) => {
            Some(Expression::ComputedMemberExpression(v))
        }
        ExportDefaultDeclarationKind::StaticMemberExpression(v) => {
            Some(Expression::StaticMemberExpression(v))
        }
        ExportDefaultDeclarationKind::PrivateFieldExpression(v) => {
            Some(Expression::PrivateFieldExpression(v))
        }
    }
}

/// Create a content component.
fn create_mdx_content<'a>(
    alloc: &'a Allocator,
    expr: Option<Expression<'a>>,
    has_internal_layout: bool,
) -> Vec<Statement<'a>> {
    // `<MDXLayout {...props}><_createMdxContent {...props}/></MDXLayout>`
    let mut layout_children = OxcVec::with_capacity_in(1, alloc);
    layout_children.push(JSXChild::Element(OxcBox::new_in(
        JSXElement {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            opening_element: OxcBox::new_in(
                JSXOpeningElement {
                    node_id: Cell::new(NodeId::DUMMY),
                    span: SPAN,
                    name: create_jsx_name_from_str(alloc, "_createMdxContent"),
                    attributes: {
                        let mut attrs = OxcVec::with_capacity_in(1, alloc);
                        attrs.push(JSXAttributeItem::SpreadAttribute(OxcBox::new_in(
                            JSXSpreadAttribute {
                                node_id: Cell::new(NodeId::DUMMY),
                                span: SPAN,
                                argument: create_ident_expression(alloc, "props"),
                            },
                            alloc,
                        )));
                        attrs
                    },
                    type_arguments: None,
                },
                alloc,
            ),
            closing_element: None,
            children: OxcVec::new_in(alloc),
        },
        alloc,
    )));

    let layout_attrs = {
        let mut attrs = OxcVec::with_capacity_in(1, alloc);
        attrs.push(JSXAttributeItem::SpreadAttribute(OxcBox::new_in(
            JSXSpreadAttribute {
                node_id: Cell::new(NodeId::DUMMY),
                span: SPAN,
                argument: create_ident_expression(alloc, "props"),
            },
            alloc,
        )));
        attrs
    };

    let mut result = Expression::JSXElement(OxcBox::new_in(
        JSXElement {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            opening_element: OxcBox::new_in(
                JSXOpeningElement {
                    node_id: Cell::new(NodeId::DUMMY),
                    span: SPAN,
                    name: create_jsx_name_from_str(alloc, "MDXLayout"),
                    attributes: layout_attrs,
                    type_arguments: None,
                },
                alloc,
            ),
            closing_element: Some(OxcBox::new_in(
                JSXClosingElement {
                    node_id: Cell::new(NodeId::DUMMY),
                    span: SPAN,
                    name: create_jsx_name_from_str(alloc, "MDXLayout"),
                },
                alloc,
            )),
            children: layout_children,
        },
        alloc,
    ));

    if !has_internal_layout {
        // `MDXLayout ? <MDXLayout>xxx</MDXLayout> : _createMdxContent(props)`
        let mut args = OxcVec::with_capacity_in(1, alloc);
        args.push(Argument::from(create_ident_expression(alloc, "props")));
        result = Expression::ConditionalExpression(OxcBox::new_in(
            ConditionalExpression {
                node_id: Cell::new(NodeId::DUMMY),
                span: SPAN,
                test: create_ident_expression(alloc, "MDXLayout"),
                consequent: result,
                alternate: create_call_expression(
                    alloc,
                    create_ident_expression(alloc, "_createMdxContent"),
                    args,
                ),
            },
            alloc,
        ));
    }

    // `function _createMdxContent(props) { return xxx }`
    let create_mdx_content = create_fn_decl(
        alloc,
        "_createMdxContent",
        &["props"],
        expr.unwrap_or_else(|| create_null_expression(alloc)),
    );

    // `function MDXContent(props = {}) { return ... }`
    let mdx_content = create_fn_decl_with_default(alloc, "MDXContent", result);

    vec![create_mdx_content, mdx_content]
}

fn create_fn_decl<'a>(
    alloc: &'a Allocator,
    name: &str,
    params: &[&str],
    return_expr: Expression<'a>,
) -> Statement<'a> {
    let mut formal_params = OxcVec::with_capacity_in(params.len(), alloc);
    for p in params {
        formal_params.push(FormalParameter {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            decorators: OxcVec::new_in(alloc),
            pattern: BindingPattern::BindingIdentifier(OxcBox::new_in(
                create_binding_ident(alloc, p),
                alloc,
            )),
            type_annotation: None,
            initializer: None,
            optional: false,
            accessibility: None,
            readonly: false,
            r#override: false,
        });
    }

    let mut stmts = OxcVec::with_capacity_in(1, alloc);
    stmts.push(Statement::ReturnStatement(OxcBox::new_in(
        ReturnStatement {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            argument: Some(return_expr),
        },
        alloc,
    )));

    Statement::from(Declaration::FunctionDeclaration(OxcBox::new_in(
        Function {
            node_id: Cell::new(NodeId::DUMMY),
            r#type: FunctionType::FunctionDeclaration,
            span: SPAN,
            id: Some(create_binding_ident(alloc, name)),
            generator: false,
            r#async: false,
            declare: false,
            type_parameters: None,
            this_param: None,
            params: OxcBox::new_in(
                FormalParameters {
                    node_id: Cell::new(NodeId::DUMMY),
                    span: SPAN,
                    kind: FormalParameterKind::FormalParameter,
                    items: formal_params,
                    rest: None,
                },
                alloc,
            ),
            return_type: None,
            body: Some(OxcBox::new_in(
                FunctionBody {
                    node_id: Cell::new(NodeId::DUMMY),
                    span: SPAN,
                    directives: OxcVec::new_in(alloc),
                    statements: stmts,
                },
                alloc,
            )),
            scope_id: Cell::default(),
            pure: false,
            pife: false,
        },
        alloc,
    )))
}

fn create_fn_decl_with_default<'a>(
    alloc: &'a Allocator,
    name: &str,
    return_expr: Expression<'a>,
) -> Statement<'a> {
    // `props = {}`
    let mut formal_params = OxcVec::with_capacity_in(1, alloc);
    formal_params.push(FormalParameter {
        node_id: Cell::new(NodeId::DUMMY),
        span: SPAN,
        decorators: OxcVec::new_in(alloc),
        pattern: BindingPattern::AssignmentPattern(OxcBox::new_in(
            AssignmentPattern {
                node_id: Cell::new(NodeId::DUMMY),
                span: SPAN,
                left: BindingPattern::BindingIdentifier(OxcBox::new_in(
                    create_binding_ident(alloc, "props"),
                    alloc,
                )),
                right: create_object_expression(alloc, OxcVec::new_in(alloc)),
            },
            alloc,
        )),
        type_annotation: None,
        initializer: None,
        optional: false,
        accessibility: None,
        readonly: false,
        r#override: false,
    });

    let mut stmts = OxcVec::with_capacity_in(1, alloc);
    stmts.push(Statement::ReturnStatement(OxcBox::new_in(
        ReturnStatement {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            argument: Some(return_expr),
        },
        alloc,
    )));

    Statement::from(Declaration::FunctionDeclaration(OxcBox::new_in(
        Function {
            node_id: Cell::new(NodeId::DUMMY),
            r#type: FunctionType::FunctionDeclaration,
            span: SPAN,
            id: Some(create_binding_ident(alloc, name)),
            generator: false,
            r#async: false,
            declare: false,
            type_parameters: None,
            this_param: None,
            params: OxcBox::new_in(
                FormalParameters {
                    node_id: Cell::new(NodeId::DUMMY),
                    span: SPAN,
                    kind: FormalParameterKind::FormalParameter,
                    items: formal_params,
                    rest: None,
                },
                alloc,
            ),
            return_type: None,
            body: Some(OxcBox::new_in(
                FunctionBody {
                    node_id: Cell::new(NodeId::DUMMY),
                    span: SPAN,
                    directives: OxcVec::new_in(alloc),
                    statements: stmts,
                },
                alloc,
            )),
            scope_id: Cell::default(),
            pure: false,
            pife: false,
        },
        alloc,
    )))
}

fn create_layout_decl<'a>(alloc: &'a Allocator, expr: Expression<'a>) -> Statement<'a> {
    let mut decls = OxcVec::with_capacity_in(1, alloc);
    decls.push(VariableDeclarator {
        node_id: Cell::new(NodeId::DUMMY),
        span: SPAN,
        kind: VariableDeclarationKind::Const,
        id: BindingPattern::BindingIdentifier(OxcBox::new_in(
            create_binding_ident(alloc, "MDXLayout"),
            alloc,
        )),
        type_annotation: None,
        init: Some(expr),
        definite: false,
    });

    Statement::from(Declaration::VariableDeclaration(OxcBox::new_in(
        VariableDeclaration {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            kind: VariableDeclarationKind::Const,
            declarations: decls,
            declare: false,
        },
        alloc,
    )))
}

fn create_import_default<'a>(alloc: &'a Allocator, local: &str, source: &str) -> Statement<'a> {
    let mut specifiers = OxcVec::with_capacity_in(1, alloc);
    specifiers.push(ImportDeclarationSpecifier::ImportDefaultSpecifier(
        OxcBox::new_in(
            ImportDefaultSpecifier {
                node_id: Cell::new(NodeId::DUMMY),
                span: SPAN,
                local: create_binding_ident(alloc, local),
            },
            alloc,
        ),
    ));

    Statement::from(ModuleDeclaration::ImportDeclaration(OxcBox::new_in(
        ImportDeclaration {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            specifiers: Some(specifiers),
            source: create_string_literal(alloc, source),
            phase: None,
            with_clause: None,
            import_kind: ImportOrExportKind::Value,
        },
        alloc,
    )))
}

fn create_import_named<'a>(
    alloc: &'a Allocator,
    local: &str,
    imported: &str,
    source: &str,
) -> Statement<'a> {
    let mut specifiers = OxcVec::with_capacity_in(1, alloc);
    specifiers.push(ImportDeclarationSpecifier::ImportSpecifier(OxcBox::new_in(
        ImportSpecifier {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            imported: ModuleExportName::IdentifierName(create_ident_name(alloc, imported)),
            local: create_binding_ident(alloc, local),
            import_kind: ImportOrExportKind::Value,
        },
        alloc,
    )));

    Statement::from(ModuleDeclaration::ImportDeclaration(OxcBox::new_in(
        ImportDeclaration {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            specifiers: Some(specifiers),
            source: create_string_literal(alloc, source),
            phase: None,
            with_clause: None,
            import_kind: ImportOrExportKind::Value,
        },
        alloc,
    )))
}

/// Create an error about multiple layouts.
fn err_for_double_layout(
    layout: bool,
    previous: Option<&Position>,
    at: Option<&Point>,
) -> Result<(), message::Message> {
    if layout {
        Err(message::Message {
            reason: format!(
                "Cannot specify multiple layouts (previous: {})",
                position_opt_to_string(previous)
            ),
            place: at.map(|p| Box::new(message::Place::Point(p.clone()))),
            source: Box::new("mdxjs-rs".into()),
            rule_id: Box::new("double-layout".into()),
        })
    } else {
        Ok(())
    }
}
