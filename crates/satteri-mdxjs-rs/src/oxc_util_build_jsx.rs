//! Turn JSX into function calls.

use crate::hast_util_to_oxc::{MdxComment, MdxCommentKind, MdxProgram};
use crate::mdx_plugin_recma_document::JsxRuntime;
use crate::oxc_utils::{
    create_binding_ident, create_bool_expression, create_call_expression, create_ident_expression,
    create_ident_name, create_member_expression_from_str, create_null_expression,
    create_num_expression, create_object_expression, create_str_expression, create_string_literal,
    jsx_attribute_name_to_prop_name, jsx_element_name_to_expression, span_to_position,
    u32_to_point,
};
use core::str;
use oxc_allocator::{Allocator, Box as OxcBox, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, ArrayExpression, ArrayExpressionElement, CallExpression, ChainElement, Class,
    ClassElement, Declaration, ExportDefaultDeclarationKind, Expression, ForStatementInit,
    ImportDeclaration, ImportDeclarationSpecifier, ImportOrExportKind, ImportSpecifier,
    JSXAttributeItem, JSXAttributeName, JSXAttributeValue, JSXChild, JSXElement, JSXExpression,
    JSXFragment, ModuleDeclaration, ModuleExportName, ObjectProperty, ObjectPropertyKind,
    PropertyKey, PropertyKind, Statement, ThisExpression,
};
use oxc_span::{SPAN, Span};
use oxc_syntax::node::NodeId;
use satteri_arena::mdx_types::{self as message, Location, Message};
use satteri_pulldown_cmark::utils::decode_html_entities;
use std::cell::Cell;

/// Configuration.
#[derive(Debug, Default, Clone)]
pub struct Options {
    /// Whether to add extra information to error messages in generated code.
    pub development: bool,
}

/// Compile JSX away to function calls.
pub fn oxc_util_build_jsx<'a>(
    program: &mut MdxProgram<'a>,
    options: &Options,
    location: Option<&Location>,
    allocator: &'a Allocator,
) -> Result<(), Message> {
    let directives = find_directives(&program.comments, location)?;

    let automatic = !matches!(directives.runtime, Some(JsxRuntime::Classic));
    let development = options.development;
    let filepath = program.path.clone();
    let create_element_name = directives
        .pragma
        .unwrap_or_else(|| "React.createElement".into());
    let fragment_name = directives
        .pragma_frag
        .unwrap_or_else(|| "React.Fragment".into());

    let mut import_fragment = false;
    let mut import_jsx = false;
    let mut import_jsxs = false;
    let mut import_jsx_dev = false;

    // Process expressions in the body
    let body_len = program.program.body.len();
    for i in 0..body_len {
        let stmt = &mut program.program.body[i];
        process_statement_jsx(
            stmt,
            allocator,
            automatic,
            development,
            filepath.as_ref(),
            location,
            &create_element_name,
            &fragment_name,
            &mut import_fragment,
            &mut import_jsx,
            &mut import_jsxs,
            &mut import_jsx_dev,
        )?;
    }

    // Generate imports for automatic runtime
    if automatic {
        let mut specifiers = OxcVec::new_in(allocator);

        if import_fragment {
            specifiers.push(ImportDeclarationSpecifier::ImportSpecifier(OxcBox::new_in(
                ImportSpecifier {
                    span: SPAN,
                    imported: ModuleExportName::IdentifierName(create_ident_name(
                        allocator, "Fragment",
                    )),
                    local: create_binding_ident(allocator, "_Fragment"),
                    import_kind: ImportOrExportKind::Value,
                    node_id: Cell::new(NodeId::DUMMY),
                },
                allocator,
            )));
        }

        if import_jsx {
            specifiers.push(ImportDeclarationSpecifier::ImportSpecifier(OxcBox::new_in(
                ImportSpecifier {
                    span: SPAN,
                    imported: ModuleExportName::IdentifierName(create_ident_name(allocator, "jsx")),
                    local: create_binding_ident(allocator, "_jsx"),
                    import_kind: ImportOrExportKind::Value,
                    node_id: Cell::new(NodeId::DUMMY),
                },
                allocator,
            )));
        }

        if import_jsxs {
            specifiers.push(ImportDeclarationSpecifier::ImportSpecifier(OxcBox::new_in(
                ImportSpecifier {
                    span: SPAN,
                    imported: ModuleExportName::IdentifierName(create_ident_name(
                        allocator, "jsxs",
                    )),
                    local: create_binding_ident(allocator, "_jsxs"),
                    import_kind: ImportOrExportKind::Value,
                    node_id: Cell::new(NodeId::DUMMY),
                },
                allocator,
            )));
        }

        if import_jsx_dev {
            specifiers.push(ImportDeclarationSpecifier::ImportSpecifier(OxcBox::new_in(
                ImportSpecifier {
                    span: SPAN,
                    imported: ModuleExportName::IdentifierName(create_ident_name(
                        allocator, "jsxDEV",
                    )),
                    local: create_binding_ident(allocator, "_jsxDEV"),
                    import_kind: ImportOrExportKind::Value,
                    node_id: Cell::new(NodeId::DUMMY),
                },
                allocator,
            )));
        }

        if !specifiers.is_empty() {
            let import_source = format!(
                "{}{}",
                directives.import_source.unwrap_or_else(|| "react".into()),
                if development {
                    "/jsx-dev-runtime"
                } else {
                    "/jsx-runtime"
                }
            );

            let import_stmt =
                Statement::from(ModuleDeclaration::ImportDeclaration(OxcBox::new_in(
                    ImportDeclaration {
                        span: SPAN,
                        specifiers: Some(specifiers),
                        source: create_string_literal(allocator, &import_source),
                        phase: None,
                        with_clause: None,
                        import_kind: ImportOrExportKind::Value,
                        node_id: Cell::new(NodeId::DUMMY),
                    },
                    allocator,
                )));

            program.program.body.insert(0, import_stmt);
        }
    }

    Ok(())
}

/// Process JSX in a statement recursively.
#[allow(clippy::too_many_arguments)]
fn process_statement_jsx<'a>(
    stmt: &mut Statement<'a>,
    alloc: &'a Allocator,
    automatic: bool,
    development: bool,
    filepath: Option<&String>,
    location: Option<&Location>,
    create_element_name: &str,
    fragment_name: &str,
    import_fragment: &mut bool,
    import_jsx: &mut bool,
    import_jsxs: &mut bool,
    import_jsx_dev: &mut bool,
) -> Result<(), Message> {
    match stmt {
        Statement::ExpressionStatement(expr_stmt) => {
            process_expression_jsx(
                &mut expr_stmt.expression,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
        }
        Statement::ReturnStatement(ret) => {
            if let Some(ref mut arg) = ret.argument {
                process_expression_jsx(
                    arg,
                    alloc,
                    automatic,
                    development,
                    filepath,
                    location,
                    create_element_name,
                    fragment_name,
                    import_fragment,
                    import_jsx,
                    import_jsxs,
                    import_jsx_dev,
                )?;
            }
        }
        Statement::VariableDeclaration(decl) => {
            for d in &mut decl.declarations {
                if let Some(ref mut init) = d.init {
                    process_expression_jsx(
                        init,
                        alloc,
                        automatic,
                        development,
                        filepath,
                        location,
                        create_element_name,
                        fragment_name,
                        import_fragment,
                        import_jsx,
                        import_jsxs,
                        import_jsx_dev,
                    )?;
                }
            }
        }
        Statement::IfStatement(if_stmt) => {
            process_expression_jsx(
                &mut if_stmt.test,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
            process_statement_jsx(
                &mut if_stmt.consequent,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
            if let Some(ref mut alt) = if_stmt.alternate {
                process_statement_jsx(
                    alt,
                    alloc,
                    automatic,
                    development,
                    filepath,
                    location,
                    create_element_name,
                    fragment_name,
                    import_fragment,
                    import_jsx,
                    import_jsxs,
                    import_jsx_dev,
                )?;
            }
        }
        Statement::SwitchStatement(switch) => {
            process_expression_jsx(
                &mut switch.discriminant,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
            for case in &mut switch.cases {
                if let Some(test) = &mut case.test {
                    process_expression_jsx(
                        test,
                        alloc,
                        automatic,
                        development,
                        filepath,
                        location,
                        create_element_name,
                        fragment_name,
                        import_fragment,
                        import_jsx,
                        import_jsxs,
                        import_jsx_dev,
                    )?;
                }
                for s in &mut case.consequent {
                    process_statement_jsx(
                        s,
                        alloc,
                        automatic,
                        development,
                        filepath,
                        location,
                        create_element_name,
                        fragment_name,
                        import_fragment,
                        import_jsx,
                        import_jsxs,
                        import_jsx_dev,
                    )?;
                }
            }
        }
        Statement::TryStatement(try_stmt) => {
            for s in &mut try_stmt.block.body {
                process_statement_jsx(
                    s,
                    alloc,
                    automatic,
                    development,
                    filepath,
                    location,
                    create_element_name,
                    fragment_name,
                    import_fragment,
                    import_jsx,
                    import_jsxs,
                    import_jsx_dev,
                )?;
            }
            if let Some(handler) = &mut try_stmt.handler {
                for s in &mut handler.body.body {
                    process_statement_jsx(
                        s,
                        alloc,
                        automatic,
                        development,
                        filepath,
                        location,
                        create_element_name,
                        fragment_name,
                        import_fragment,
                        import_jsx,
                        import_jsxs,
                        import_jsx_dev,
                    )?;
                }
            }
            if let Some(finalizer) = &mut try_stmt.finalizer {
                for s in &mut finalizer.body {
                    process_statement_jsx(
                        s,
                        alloc,
                        automatic,
                        development,
                        filepath,
                        location,
                        create_element_name,
                        fragment_name,
                        import_fragment,
                        import_jsx,
                        import_jsxs,
                        import_jsx_dev,
                    )?;
                }
            }
        }
        Statement::WhileStatement(while_stmt) => {
            process_expression_jsx(
                &mut while_stmt.test,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
            process_statement_jsx(
                &mut while_stmt.body,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
        }
        Statement::DoWhileStatement(do_while) => {
            process_statement_jsx(
                &mut do_while.body,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
            process_expression_jsx(
                &mut do_while.test,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
        }
        Statement::ForStatement(for_stmt) => {
            if let Some(init) = &mut for_stmt.init {
                match init {
                    ForStatementInit::VariableDeclaration(decl) => {
                        for d in &mut decl.declarations {
                            if let Some(ref mut init) = d.init {
                                process_expression_jsx(
                                    init,
                                    alloc,
                                    automatic,
                                    development,
                                    filepath,
                                    location,
                                    create_element_name,
                                    fragment_name,
                                    import_fragment,
                                    import_jsx,
                                    import_jsxs,
                                    import_jsx_dev,
                                )?;
                            }
                        }
                    }
                    _ => {
                        if let Some(expr) = init.as_expression_mut() {
                            process_expression_jsx(
                                expr,
                                alloc,
                                automatic,
                                development,
                                filepath,
                                location,
                                create_element_name,
                                fragment_name,
                                import_fragment,
                                import_jsx,
                                import_jsxs,
                                import_jsx_dev,
                            )?;
                        }
                    }
                }
            }
            if let Some(test) = &mut for_stmt.test {
                process_expression_jsx(
                    test,
                    alloc,
                    automatic,
                    development,
                    filepath,
                    location,
                    create_element_name,
                    fragment_name,
                    import_fragment,
                    import_jsx,
                    import_jsxs,
                    import_jsx_dev,
                )?;
            }
            if let Some(update) = &mut for_stmt.update {
                process_expression_jsx(
                    update,
                    alloc,
                    automatic,
                    development,
                    filepath,
                    location,
                    create_element_name,
                    fragment_name,
                    import_fragment,
                    import_jsx,
                    import_jsxs,
                    import_jsx_dev,
                )?;
            }
            process_statement_jsx(
                &mut for_stmt.body,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
        }
        Statement::ForInStatement(for_in) => {
            process_expression_jsx(
                &mut for_in.right,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
            process_statement_jsx(
                &mut for_in.body,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
        }
        Statement::ForOfStatement(for_of) => {
            process_expression_jsx(
                &mut for_of.right,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
            process_statement_jsx(
                &mut for_of.body,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
        }
        Statement::LabeledStatement(labeled) => {
            process_statement_jsx(
                &mut labeled.body,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
        }
        Statement::WithStatement(with) => {
            process_expression_jsx(
                &mut with.object,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
            process_statement_jsx(
                &mut with.body,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
        }
        Statement::ClassDeclaration(class) => {
            process_class_jsx(
                class,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
        }
        Statement::BlockStatement(block) => {
            for s in &mut block.body {
                process_statement_jsx(
                    s,
                    alloc,
                    automatic,
                    development,
                    filepath,
                    location,
                    create_element_name,
                    fragment_name,
                    import_fragment,
                    import_jsx,
                    import_jsxs,
                    import_jsx_dev,
                )?;
            }
        }
        Statement::FunctionDeclaration(func) => {
            if let Some(ref mut body) = func.body {
                for s in &mut body.statements {
                    process_statement_jsx(
                        s,
                        alloc,
                        automatic,
                        development,
                        filepath,
                        location,
                        create_element_name,
                        fragment_name,
                        import_fragment,
                        import_jsx,
                        import_jsxs,
                        import_jsx_dev,
                    )?;
                }
            }
        }
        Statement::ExportDefaultDeclaration(decl) => match &mut decl.declaration {
            ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
                if let Some(ref mut body) = func.body {
                    for s in &mut body.statements {
                        process_statement_jsx(
                            s,
                            alloc,
                            automatic,
                            development,
                            filepath,
                            location,
                            create_element_name,
                            fragment_name,
                            import_fragment,
                            import_jsx,
                            import_jsxs,
                            import_jsx_dev,
                        )?;
                    }
                }
            }
            ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                process_class_jsx(
                    class,
                    alloc,
                    automatic,
                    development,
                    filepath,
                    location,
                    create_element_name,
                    fragment_name,
                    import_fragment,
                    import_jsx,
                    import_jsxs,
                    import_jsx_dev,
                )?;
            }
            other => {
                if let Some(expr) = other.as_expression_mut() {
                    process_expression_jsx(
                        expr,
                        alloc,
                        automatic,
                        development,
                        filepath,
                        location,
                        create_element_name,
                        fragment_name,
                        import_fragment,
                        import_jsx,
                        import_jsxs,
                        import_jsx_dev,
                    )?;
                }
            }
        },
        Statement::ExportNamedDeclaration(decl) => match &mut decl.declaration {
            Some(Declaration::FunctionDeclaration(func)) => {
                if let Some(ref mut body) = func.body {
                    for s in &mut body.statements {
                        process_statement_jsx(
                            s,
                            alloc,
                            automatic,
                            development,
                            filepath,
                            location,
                            create_element_name,
                            fragment_name,
                            import_fragment,
                            import_jsx,
                            import_jsxs,
                            import_jsx_dev,
                        )?;
                    }
                }
            }
            Some(Declaration::VariableDeclaration(var_decl)) => {
                for d in &mut var_decl.declarations {
                    if let Some(ref mut init) = d.init {
                        process_expression_jsx(
                            init,
                            alloc,
                            automatic,
                            development,
                            filepath,
                            location,
                            create_element_name,
                            fragment_name,
                            import_fragment,
                            import_jsx,
                            import_jsxs,
                            import_jsx_dev,
                        )?;
                    }
                }
            }
            Some(Declaration::ClassDeclaration(class)) => {
                process_class_jsx(
                    class,
                    alloc,
                    automatic,
                    development,
                    filepath,
                    location,
                    create_element_name,
                    fragment_name,
                    import_fragment,
                    import_jsx,
                    import_jsxs,
                    import_jsx_dev,
                )?;
            }
            _ => {}
        },
        Statement::ThrowStatement(throw) => {
            process_expression_jsx(
                &mut throw.argument,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
        }
        _ => {}
    }
    Ok(())
}

/// Process an expression, replacing JSX with function calls.
#[allow(clippy::too_many_arguments)]
fn process_expression_jsx<'a>(
    expr: &mut Expression<'a>,
    alloc: &'a Allocator,
    automatic: bool,
    development: bool,
    filepath: Option<&String>,
    location: Option<&Location>,
    create_element_name: &str,
    fragment_name: &str,
    import_fragment: &mut bool,
    import_jsx: &mut bool,
    import_jsxs: &mut bool,
    import_jsx_dev: &mut bool,
) -> Result<(), Message> {
    // First recurse into sub-expressions
    process_sub_expressions(
        expr,
        alloc,
        automatic,
        development,
        filepath,
        location,
        create_element_name,
        fragment_name,
        import_fragment,
        import_jsx,
        import_jsxs,
        import_jsx_dev,
    )?;

    // Then replace JSX at this level
    match expr {
        Expression::JSXElement(elem) => {
            let replacement = jsx_element_to_call(
                elem,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
            *expr = replacement;
        }
        Expression::JSXFragment(frag) => {
            let replacement = jsx_fragment_to_call(
                frag,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
            *expr = replacement;
        }
        _ => {}
    }

    Ok(())
}

/// Process sub-expressions recursively.
#[allow(clippy::too_many_arguments)]
fn process_sub_expressions<'a>(
    expr: &mut Expression<'a>,
    alloc: &'a Allocator,
    automatic: bool,
    development: bool,
    filepath: Option<&String>,
    location: Option<&Location>,
    create_element_name: &str,
    fragment_name: &str,
    import_fragment: &mut bool,
    import_jsx: &mut bool,
    import_jsxs: &mut bool,
    import_jsx_dev: &mut bool,
) -> Result<(), Message> {
    match expr {
        Expression::CallExpression(call) => {
            process_expression_jsx(
                &mut call.callee,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
            for arg in &mut call.arguments {
                if let Argument::SpreadElement(spread) = arg {
                    process_expression_jsx(
                        &mut spread.argument,
                        alloc,
                        automatic,
                        development,
                        filepath,
                        location,
                        create_element_name,
                        fragment_name,
                        import_fragment,
                        import_jsx,
                        import_jsxs,
                        import_jsx_dev,
                    )?;
                } else if let Some(expr) = arg.as_expression_mut() {
                    process_expression_jsx(
                        expr,
                        alloc,
                        automatic,
                        development,
                        filepath,
                        location,
                        create_element_name,
                        fragment_name,
                        import_fragment,
                        import_jsx,
                        import_jsxs,
                        import_jsx_dev,
                    )?;
                }
            }
        }
        Expression::ConditionalExpression(cond) => {
            process_expression_jsx(
                &mut cond.test,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
            process_expression_jsx(
                &mut cond.consequent,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
            process_expression_jsx(
                &mut cond.alternate,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
        }
        Expression::ObjectExpression(obj) => {
            for prop in &mut obj.properties {
                match prop {
                    ObjectPropertyKind::ObjectProperty(p) => {
                        process_expression_jsx(
                            &mut p.value,
                            alloc,
                            automatic,
                            development,
                            filepath,
                            location,
                            create_element_name,
                            fragment_name,
                            import_fragment,
                            import_jsx,
                            import_jsxs,
                            import_jsx_dev,
                        )?;
                    }
                    ObjectPropertyKind::SpreadProperty(s) => {
                        process_expression_jsx(
                            &mut s.argument,
                            alloc,
                            automatic,
                            development,
                            filepath,
                            location,
                            create_element_name,
                            fragment_name,
                            import_fragment,
                            import_jsx,
                            import_jsxs,
                            import_jsx_dev,
                        )?;
                    }
                }
            }
        }
        Expression::ArrayExpression(arr) => {
            for elem in &mut arr.elements {
                match elem {
                    ArrayExpressionElement::SpreadElement(s) => {
                        process_expression_jsx(
                            &mut s.argument,
                            alloc,
                            automatic,
                            development,
                            filepath,
                            location,
                            create_element_name,
                            fragment_name,
                            import_fragment,
                            import_jsx,
                            import_jsxs,
                            import_jsx_dev,
                        )?;
                    }
                    ArrayExpressionElement::Elision(_) => {}
                    _ => {
                        if let Some(inner) = elem.as_expression_mut() {
                            process_expression_jsx(
                                inner,
                                alloc,
                                automatic,
                                development,
                                filepath,
                                location,
                                create_element_name,
                                fragment_name,
                                import_fragment,
                                import_jsx,
                                import_jsxs,
                                import_jsx_dev,
                            )?;
                        }
                    }
                }
            }
        }
        Expression::ParenthesizedExpression(paren) => {
            process_expression_jsx(
                &mut paren.expression,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
        }
        Expression::AssignmentExpression(assign) => {
            process_expression_jsx(
                &mut assign.right,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
        }
        Expression::LogicalExpression(logical) => {
            process_expression_jsx(
                &mut logical.left,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
            process_expression_jsx(
                &mut logical.right,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
        }
        Expression::BinaryExpression(bin) => {
            process_expression_jsx(
                &mut bin.left,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
            process_expression_jsx(
                &mut bin.right,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
        }
        Expression::ArrowFunctionExpression(arrow) => {
            for s in &mut arrow.body.statements {
                process_statement_jsx(
                    s,
                    alloc,
                    automatic,
                    development,
                    filepath,
                    location,
                    create_element_name,
                    fragment_name,
                    import_fragment,
                    import_jsx,
                    import_jsxs,
                    import_jsx_dev,
                )?;
            }
        }
        Expression::FunctionExpression(func) => {
            if let Some(body) = &mut func.body {
                for s in &mut body.statements {
                    process_statement_jsx(
                        s,
                        alloc,
                        automatic,
                        development,
                        filepath,
                        location,
                        create_element_name,
                        fragment_name,
                        import_fragment,
                        import_jsx,
                        import_jsxs,
                        import_jsx_dev,
                    )?;
                }
            }
        }
        Expression::SequenceExpression(seq) => {
            for e in &mut seq.expressions {
                process_expression_jsx(
                    e,
                    alloc,
                    automatic,
                    development,
                    filepath,
                    location,
                    create_element_name,
                    fragment_name,
                    import_fragment,
                    import_jsx,
                    import_jsxs,
                    import_jsx_dev,
                )?;
            }
        }
        Expression::UnaryExpression(unary) => {
            process_expression_jsx(
                &mut unary.argument,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
        }
        Expression::AwaitExpression(await_expr) => {
            process_expression_jsx(
                &mut await_expr.argument,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
        }
        Expression::YieldExpression(yield_expr) => {
            if let Some(arg) = &mut yield_expr.argument {
                process_expression_jsx(
                    arg,
                    alloc,
                    automatic,
                    development,
                    filepath,
                    location,
                    create_element_name,
                    fragment_name,
                    import_fragment,
                    import_jsx,
                    import_jsxs,
                    import_jsx_dev,
                )?;
            }
        }
        Expression::NewExpression(new_expr) => {
            process_expression_jsx(
                &mut new_expr.callee,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
            for arg in &mut new_expr.arguments {
                if let Argument::SpreadElement(spread) = arg {
                    process_expression_jsx(
                        &mut spread.argument,
                        alloc,
                        automatic,
                        development,
                        filepath,
                        location,
                        create_element_name,
                        fragment_name,
                        import_fragment,
                        import_jsx,
                        import_jsxs,
                        import_jsx_dev,
                    )?;
                } else if let Some(expr) = arg.as_expression_mut() {
                    process_expression_jsx(
                        expr,
                        alloc,
                        automatic,
                        development,
                        filepath,
                        location,
                        create_element_name,
                        fragment_name,
                        import_fragment,
                        import_jsx,
                        import_jsxs,
                        import_jsx_dev,
                    )?;
                }
            }
        }
        Expression::TemplateLiteral(tmpl) => {
            for e in &mut tmpl.expressions {
                process_expression_jsx(
                    e,
                    alloc,
                    automatic,
                    development,
                    filepath,
                    location,
                    create_element_name,
                    fragment_name,
                    import_fragment,
                    import_jsx,
                    import_jsxs,
                    import_jsx_dev,
                )?;
            }
        }
        Expression::TaggedTemplateExpression(tagged) => {
            process_expression_jsx(
                &mut tagged.tag,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
            for e in &mut tagged.quasi.expressions {
                process_expression_jsx(
                    e,
                    alloc,
                    automatic,
                    development,
                    filepath,
                    location,
                    create_element_name,
                    fragment_name,
                    import_fragment,
                    import_jsx,
                    import_jsxs,
                    import_jsx_dev,
                )?;
            }
        }
        Expression::ChainExpression(chain) => match &mut chain.expression {
            ChainElement::CallExpression(call) => {
                process_expression_jsx(
                    &mut call.callee,
                    alloc,
                    automatic,
                    development,
                    filepath,
                    location,
                    create_element_name,
                    fragment_name,
                    import_fragment,
                    import_jsx,
                    import_jsxs,
                    import_jsx_dev,
                )?;
                for arg in &mut call.arguments {
                    if let Argument::SpreadElement(spread) = arg {
                        process_expression_jsx(
                            &mut spread.argument,
                            alloc,
                            automatic,
                            development,
                            filepath,
                            location,
                            create_element_name,
                            fragment_name,
                            import_fragment,
                            import_jsx,
                            import_jsxs,
                            import_jsx_dev,
                        )?;
                    } else if let Some(expr) = arg.as_expression_mut() {
                        process_expression_jsx(
                            expr,
                            alloc,
                            automatic,
                            development,
                            filepath,
                            location,
                            create_element_name,
                            fragment_name,
                            import_fragment,
                            import_jsx,
                            import_jsxs,
                            import_jsx_dev,
                        )?;
                    }
                }
            }
            ChainElement::ComputedMemberExpression(member) => {
                process_expression_jsx(
                    &mut member.object,
                    alloc,
                    automatic,
                    development,
                    filepath,
                    location,
                    create_element_name,
                    fragment_name,
                    import_fragment,
                    import_jsx,
                    import_jsxs,
                    import_jsx_dev,
                )?;
                process_expression_jsx(
                    &mut member.expression,
                    alloc,
                    automatic,
                    development,
                    filepath,
                    location,
                    create_element_name,
                    fragment_name,
                    import_fragment,
                    import_jsx,
                    import_jsxs,
                    import_jsx_dev,
                )?;
            }
            ChainElement::StaticMemberExpression(member) => {
                process_expression_jsx(
                    &mut member.object,
                    alloc,
                    automatic,
                    development,
                    filepath,
                    location,
                    create_element_name,
                    fragment_name,
                    import_fragment,
                    import_jsx,
                    import_jsxs,
                    import_jsx_dev,
                )?;
            }
            ChainElement::PrivateFieldExpression(member) => {
                process_expression_jsx(
                    &mut member.object,
                    alloc,
                    automatic,
                    development,
                    filepath,
                    location,
                    create_element_name,
                    fragment_name,
                    import_fragment,
                    import_jsx,
                    import_jsxs,
                    import_jsx_dev,
                )?;
            }
            ChainElement::TSNonNullExpression(_) => {}
        },
        Expression::ImportExpression(imp) => {
            process_expression_jsx(
                &mut imp.source,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
            if let Some(opt) = &mut imp.options {
                process_expression_jsx(
                    opt,
                    alloc,
                    automatic,
                    development,
                    filepath,
                    location,
                    create_element_name,
                    fragment_name,
                    import_fragment,
                    import_jsx,
                    import_jsxs,
                    import_jsx_dev,
                )?;
            }
        }
        Expression::ClassExpression(class) => {
            process_class_jsx(
                class,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
        }
        Expression::PrivateInExpression(priv_in) => {
            process_expression_jsx(
                &mut priv_in.right,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
        }
        Expression::ComputedMemberExpression(member) => {
            process_expression_jsx(
                &mut member.object,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
            process_expression_jsx(
                &mut member.expression,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
        }
        Expression::StaticMemberExpression(member) => {
            process_expression_jsx(
                &mut member.object,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
        }
        Expression::PrivateFieldExpression(member) => {
            process_expression_jsx(
                &mut member.object,
                alloc,
                automatic,
                development,
                filepath,
                location,
                create_element_name,
                fragment_name,
                import_fragment,
                import_jsx,
                import_jsxs,
                import_jsx_dev,
            )?;
        }
        _ => {}
    }
    Ok(())
}

/// Process JSX inside a class body.
#[allow(clippy::too_many_arguments)]
fn process_class_jsx<'a>(
    class: &mut OxcBox<'a, Class<'a>>,
    alloc: &'a Allocator,
    automatic: bool,
    development: bool,
    filepath: Option<&String>,
    location: Option<&Location>,
    create_element_name: &str,
    fragment_name: &str,
    import_fragment: &mut bool,
    import_jsx: &mut bool,
    import_jsxs: &mut bool,
    import_jsx_dev: &mut bool,
) -> Result<(), Message> {
    for element in &mut class.body.body {
        match element {
            ClassElement::MethodDefinition(method) => {
                if method.computed
                    && let Some(key) = method.key.as_expression_mut()
                {
                    process_expression_jsx(
                        key,
                        alloc,
                        automatic,
                        development,
                        filepath,
                        location,
                        create_element_name,
                        fragment_name,
                        import_fragment,
                        import_jsx,
                        import_jsxs,
                        import_jsx_dev,
                    )?;
                }
                if let Some(body) = &mut method.value.body {
                    for s in &mut body.statements {
                        process_statement_jsx(
                            s,
                            alloc,
                            automatic,
                            development,
                            filepath,
                            location,
                            create_element_name,
                            fragment_name,
                            import_fragment,
                            import_jsx,
                            import_jsxs,
                            import_jsx_dev,
                        )?;
                    }
                }
            }
            ClassElement::PropertyDefinition(prop) => {
                if prop.computed
                    && let Some(key) = prop.key.as_expression_mut()
                {
                    process_expression_jsx(
                        key,
                        alloc,
                        automatic,
                        development,
                        filepath,
                        location,
                        create_element_name,
                        fragment_name,
                        import_fragment,
                        import_jsx,
                        import_jsxs,
                        import_jsx_dev,
                    )?;
                }
                if let Some(value) = &mut prop.value {
                    process_expression_jsx(
                        value,
                        alloc,
                        automatic,
                        development,
                        filepath,
                        location,
                        create_element_name,
                        fragment_name,
                        import_fragment,
                        import_jsx,
                        import_jsxs,
                        import_jsx_dev,
                    )?;
                }
            }
            ClassElement::AccessorProperty(acc) => {
                if acc.computed
                    && let Some(key) = acc.key.as_expression_mut()
                {
                    process_expression_jsx(
                        key,
                        alloc,
                        automatic,
                        development,
                        filepath,
                        location,
                        create_element_name,
                        fragment_name,
                        import_fragment,
                        import_jsx,
                        import_jsxs,
                        import_jsx_dev,
                    )?;
                }
                if let Some(value) = &mut acc.value {
                    process_expression_jsx(
                        value,
                        alloc,
                        automatic,
                        development,
                        filepath,
                        location,
                        create_element_name,
                        fragment_name,
                        import_fragment,
                        import_jsx,
                        import_jsxs,
                        import_jsx_dev,
                    )?;
                }
            }
            ClassElement::StaticBlock(block) => {
                for s in &mut block.body {
                    process_statement_jsx(
                        s,
                        alloc,
                        automatic,
                        development,
                        filepath,
                        location,
                        create_element_name,
                        fragment_name,
                        import_fragment,
                        import_jsx,
                        import_jsxs,
                        import_jsx_dev,
                    )?;
                }
            }
            ClassElement::TSIndexSignature(_) => {}
        }
    }
    Ok(())
}

/// Convert a JSX element to a function call.
#[allow(clippy::too_many_arguments)]
fn jsx_element_to_call<'a>(
    elem: &mut OxcBox<'a, JSXElement<'a>>,
    alloc: &'a Allocator,
    automatic: bool,
    development: bool,
    filepath: Option<&String>,
    location: Option<&Location>,
    create_element_name: &str,
    fragment_name: &str,
    import_fragment: &mut bool,
    import_jsx: &mut bool,
    import_jsxs: &mut bool,
    import_jsx_dev: &mut bool,
) -> Result<Expression<'a>, Message> {
    let span = elem.span;

    // Process children first
    let children = jsx_children_to_expressions(
        &mut elem.children,
        alloc,
        automatic,
        development,
        filepath,
        location,
        create_element_name,
        fragment_name,
        import_fragment,
        import_jsx,
        import_jsxs,
        import_jsx_dev,
    )?;

    // Get name expression
    let mut name = jsx_element_name_to_expression(alloc, &elem.opening_element.name);

    // Lowercase identifiers become string literals
    if let Expression::Identifier(ident) = &name {
        let head = ident.name.as_bytes();
        if matches!(head.first(), Some(b'a'..=b'z')) {
            name = create_str_expression(alloc, &ident.name);
        }
    }

    // Process attributes
    let attrs: Vec<_> = elem.opening_element.attributes.drain(..).collect();
    jsx_to_call(
        span,
        name,
        Some(attrs),
        children,
        alloc,
        automatic,
        development,
        filepath,
        location,
        create_element_name,
        fragment_name,
        import_fragment,
        import_jsx,
        import_jsxs,
        import_jsx_dev,
    )
}

/// Convert a JSX fragment to a function call.
#[allow(clippy::too_many_arguments)]
fn jsx_fragment_to_call<'a>(
    frag: &mut OxcBox<'a, JSXFragment<'a>>,
    alloc: &'a Allocator,
    automatic: bool,
    development: bool,
    filepath: Option<&String>,
    location: Option<&Location>,
    create_element_name: &str,
    fragment_name: &str,
    import_fragment: &mut bool,
    import_jsx: &mut bool,
    import_jsxs: &mut bool,
    import_jsx_dev: &mut bool,
) -> Result<Expression<'a>, Message> {
    let span = frag.span;
    let name = if automatic {
        *import_fragment = true;
        create_ident_expression(alloc, "_Fragment")
    } else {
        create_member_expression_from_str(alloc, fragment_name)
    };

    let children = jsx_children_to_expressions(
        &mut frag.children,
        alloc,
        automatic,
        development,
        filepath,
        location,
        create_element_name,
        fragment_name,
        import_fragment,
        import_jsx,
        import_jsxs,
        import_jsx_dev,
    )?;

    jsx_to_call(
        span,
        name,
        None,
        children,
        alloc,
        automatic,
        development,
        filepath,
        location,
        create_element_name,
        fragment_name,
        import_fragment,
        import_jsx,
        import_jsxs,
        import_jsx_dev,
    )
}

/// Common logic for JSX element/fragment to call.
#[allow(clippy::too_many_arguments)]
fn jsx_to_call<'a>(
    span: Span,
    name: Expression<'a>,
    attributes: Option<Vec<JSXAttributeItem<'a>>>,
    mut children: Vec<Expression<'a>>,
    alloc: &'a Allocator,
    automatic: bool,
    development: bool,
    filepath: Option<&String>,
    location: Option<&Location>,
    create_element_name: &str,
    fragment_name: &str,
    import_fragment: &mut bool,
    import_jsx: &mut bool,
    import_jsxs: &mut bool,
    import_jsx_dev: &mut bool,
) -> Result<Expression<'a>, Message> {
    let (callee, parameters) = if automatic {
        let is_static_children = children.len() > 1;
        let (props, key) = jsx_attributes_to_props(
            alloc,
            attributes,
            Some(&mut children),
            location,
            automatic,
            development,
            filepath,
            create_element_name,
            fragment_name,
            import_fragment,
            import_jsx,
            import_jsxs,
            import_jsx_dev,
        )?;

        let mut parameters = OxcVec::with_capacity_in(6, alloc);
        parameters.push(Argument::from(name));
        parameters.push(Argument::from(props.unwrap_or_else(|| {
            create_object_expression(alloc, OxcVec::new_in(alloc))
        })));

        if let Some(key) = key {
            parameters.push(Argument::from(key));
        } else if development {
            parameters.push(Argument::from(create_ident_expression(alloc, "undefined")));
        }

        if development {
            parameters.push(Argument::from(create_bool_expression(
                alloc,
                is_static_children,
            )));

            let filename = if let Some(value) = filepath {
                create_str_expression(alloc, value)
            } else {
                create_str_expression(alloc, "<source.js>")
            };

            let mut meta_fields = OxcVec::with_capacity_in(3, alloc);
            meta_fields.push(ObjectPropertyKind::ObjectProperty(OxcBox::new_in(
                ObjectProperty {
                    span: SPAN,
                    kind: PropertyKind::Init,
                    key: PropertyKey::StaticIdentifier(OxcBox::new_in(
                        create_ident_name(alloc, "fileName"),
                        alloc,
                    )),
                    value: filename,
                    method: false,
                    shorthand: false,
                    computed: false,
                    node_id: Cell::new(NodeId::DUMMY),
                },
                alloc,
            )));

            if let Some(position) = span_to_position(span, location) {
                meta_fields.push(ObjectPropertyKind::ObjectProperty(OxcBox::new_in(
                    ObjectProperty {
                        span: SPAN,
                        kind: PropertyKind::Init,
                        key: PropertyKey::StaticIdentifier(OxcBox::new_in(
                            create_ident_name(alloc, "lineNumber"),
                            alloc,
                        )),
                        value: create_num_expression(alloc, position.start.line as f64),
                        method: false,
                        shorthand: false,
                        computed: false,
                        node_id: Cell::new(NodeId::DUMMY),
                    },
                    alloc,
                )));

                meta_fields.push(ObjectPropertyKind::ObjectProperty(OxcBox::new_in(
                    ObjectProperty {
                        span: SPAN,
                        kind: PropertyKind::Init,
                        key: PropertyKey::StaticIdentifier(OxcBox::new_in(
                            create_ident_name(alloc, "columnNumber"),
                            alloc,
                        )),
                        value: create_num_expression(alloc, position.start.column as f64),
                        method: false,
                        shorthand: false,
                        computed: false,
                        node_id: Cell::new(NodeId::DUMMY),
                    },
                    alloc,
                )));
            }

            parameters.push(Argument::from(create_object_expression(alloc, meta_fields)));
            parameters.push(Argument::from(Expression::ThisExpression(OxcBox::new_in(
                ThisExpression {
                    span: SPAN,
                    node_id: Cell::new(NodeId::DUMMY),
                },
                alloc,
            ))));
        }

        let callee_name = if development {
            *import_jsx_dev = true;
            "_jsxDEV"
        } else if is_static_children {
            *import_jsxs = true;
            "_jsxs"
        } else {
            *import_jsx = true;
            "_jsx"
        };

        (create_ident_expression(alloc, callee_name), parameters)
    } else {
        // Classic runtime
        let (props, _key) = jsx_attributes_to_props(
            alloc,
            attributes,
            None,
            location,
            automatic,
            development,
            filepath,
            create_element_name,
            fragment_name,
            import_fragment,
            import_jsx,
            import_jsxs,
            import_jsx_dev,
        )?;

        let mut parameters = OxcVec::with_capacity_in(4, alloc);
        parameters.push(Argument::from(name));

        if let Some(props) = props {
            parameters.push(Argument::from(props));
        } else if !children.is_empty() {
            parameters.push(Argument::from(create_null_expression(alloc)));
        }

        children.reverse();
        while let Some(child) = children.pop() {
            parameters.push(Argument::from(child));
        }

        (
            create_member_expression_from_str(alloc, create_element_name),
            parameters,
        )
    };

    Ok(Expression::CallExpression(OxcBox::new_in(
        CallExpression {
            span,
            callee,
            type_arguments: None,
            arguments: parameters,
            optional: false,
            pure: false,
            node_id: Cell::new(NodeId::DUMMY),
        },
        alloc,
    )))
}

/// Convert JSX children to expressions.
#[allow(clippy::too_many_arguments)]
fn jsx_children_to_expressions<'a>(
    children: &mut OxcVec<'a, JSXChild<'a>>,
    alloc: &'a Allocator,
    automatic: bool,
    development: bool,
    filepath: Option<&String>,
    location: Option<&Location>,
    create_element_name: &str,
    fragment_name: &str,
    import_fragment: &mut bool,
    import_jsx: &mut bool,
    import_jsxs: &mut bool,
    import_jsx_dev: &mut bool,
) -> Result<Vec<Expression<'a>>, Message> {
    let mut result = vec![];
    let children_vec: Vec<_> = children.drain(..).collect();

    for child in children_vec {
        match child {
            JSXChild::Spread(spread) => {
                let lo = spread.span.start;
                return Err(Message {
                    reason:
                        "Unexpected spread child, which is not supported in Babel, SWC, or React"
                            .into(),
                    place: u32_to_point(lo, location).map(|p| Box::new(message::Place::Point(p))),
                    source: Box::new("mdxjs-rs".into()),
                    rule_id: Box::new("spread".into()),
                });
            }
            JSXChild::ExpressionContainer(container) => {
                let container = container.unbox();
                match container.expression {
                    JSXExpression::EmptyExpression(_) => {}
                    _ => {
                        if let Some(mut expr) = jsx_expression_to_expression(container.expression) {
                            process_expression_jsx(
                                &mut expr,
                                alloc,
                                automatic,
                                development,
                                filepath,
                                location,
                                create_element_name,
                                fragment_name,
                                import_fragment,
                                import_jsx,
                                import_jsxs,
                                import_jsx_dev,
                            )?;
                            result.push(expr);
                        }
                    }
                }
            }
            JSXChild::Text(text) => {
                let text = text.unbox();
                let value = jsx_text_to_value(text.value.as_str());
                if !value.is_empty() {
                    result.push(create_str_expression(alloc, &value));
                }
            }
            JSXChild::Element(mut elem) => {
                let call = jsx_element_to_call(
                    &mut elem,
                    alloc,
                    automatic,
                    development,
                    filepath,
                    location,
                    create_element_name,
                    fragment_name,
                    import_fragment,
                    import_jsx,
                    import_jsxs,
                    import_jsx_dev,
                )?;
                result.push(call);
            }
            JSXChild::Fragment(mut frag) => {
                let call = jsx_fragment_to_call(
                    &mut frag,
                    alloc,
                    automatic,
                    development,
                    filepath,
                    location,
                    create_element_name,
                    fragment_name,
                    import_fragment,
                    import_jsx,
                    import_jsxs,
                    import_jsx_dev,
                )?;
                result.push(call);
            }
        }
    }

    Ok(result)
}

/// Convert a `JSXExpression` to an `Expression`.
fn jsx_expression_to_expression(jsx_expr: JSXExpression<'_>) -> Option<Expression<'_>> {
    match jsx_expr {
        JSXExpression::EmptyExpression(_) => None,
        // All other variants are Expression variants (inherited)
        _ => {
            // JSXExpression inherits from Expression, so we can extract it
            // The non-empty variants are Expression variants
            Some(jsx_expr.into_expression())
        }
    }
}

/// Convert JSX attributes to props expression and optional key.
#[allow(clippy::too_many_arguments)]
fn jsx_attributes_to_props<'a>(
    alloc: &'a Allocator,
    attributes: Option<Vec<JSXAttributeItem<'a>>>,
    children: Option<&mut Vec<Expression<'a>>>,
    location: Option<&Location>,
    automatic: bool,
    development: bool,
    filepath: Option<&String>,
    create_element_name: &str,
    fragment_name: &str,
    import_fragment: &mut bool,
    import_jsx: &mut bool,
    import_jsxs: &mut bool,
    import_jsx_dev: &mut bool,
) -> Result<(Option<Expression<'a>>, Option<Expression<'a>>), Message> {
    let mut objects: Vec<Expression<'a>> = vec![];
    let mut fields: Vec<ObjectPropertyKind<'a>> = vec![];
    let mut spread = false;
    let mut key = None;

    if let Some(attributes) = attributes {
        for attribute in attributes {
            match attribute {
                JSXAttributeItem::SpreadAttribute(spread_attr) => {
                    let mut spread_attr = spread_attr.unbox();
                    if !fields.is_empty() {
                        let props_vec = OxcVec::from_iter_in(fields.drain(..), alloc);
                        objects.push(create_object_expression(alloc, props_vec));
                    }
                    // Lower any JSX inside the spread argument (e.g. `{...{x: <p/>}}`).
                    process_expression_jsx(
                        &mut spread_attr.argument,
                        alloc,
                        automatic,
                        development,
                        filepath,
                        location,
                        create_element_name,
                        fragment_name,
                        import_fragment,
                        import_jsx,
                        import_jsxs,
                        import_jsx_dev,
                    )?;
                    objects.push(spread_attr.argument);
                    spread = true;
                }
                JSXAttributeItem::Attribute(jsx_attr) => {
                    let jsx_attr = jsx_attr.unbox();
                    let is_key = if let JSXAttributeName::Identifier(ref ident) = jsx_attr.name {
                        ident.name.as_str() == "key"
                    } else {
                        false
                    };

                    let mut value = jsx_attr_value_to_expression(alloc, jsx_attr.value);
                    // Lower any JSX nested in the attribute value (e.g.
                    // `d={<p>x</p>}`, `d={<>x</>}`, `d={cond ? <a/> : <b/>}`).
                    // Children are recursed into separately (in
                    // `jsx_element_to_call`); attribute values were not, so
                    // their JSX leaked through un-lowered as raw `<...>`.
                    process_expression_jsx(
                        &mut value,
                        alloc,
                        automatic,
                        development,
                        filepath,
                        location,
                        create_element_name,
                        fragment_name,
                        import_fragment,
                        import_jsx,
                        import_jsxs,
                        import_jsx_dev,
                    )?;

                    if is_key && children.is_some() {
                        // automatic runtime: extract key
                        if spread {
                            let lo = jsx_attr.span.start;
                            return Err(Message {
                                reason: "Expected `key` to come before any spread expressions"
                                    .into(),
                                place: u32_to_point(lo, location)
                                    .map(|p| Box::new(message::Place::Point(p))),
                                source: Box::new("mdxjs-rs".into()),
                                rule_id: Box::new("key".into()),
                            });
                        }
                        key = Some(value);
                    } else {
                        let prop_key = jsx_attribute_name_to_prop_name(alloc, &jsx_attr.name);
                        fields.push(ObjectPropertyKind::ObjectProperty(OxcBox::new_in(
                            ObjectProperty {
                                span: SPAN,
                                kind: PropertyKind::Init,
                                key: prop_key,
                                value,
                                method: false,
                                shorthand: false,
                                computed: false,
                                node_id: Cell::new(NodeId::DUMMY),
                            },
                            alloc,
                        )));
                    }
                }
            }
        }
    }

    // In the automatic runtime, add children as a prop.
    if let Some(children) = children
        && !children.is_empty()
    {
        let value = if children.len() == 1 {
            children.pop().unwrap()
        } else {
            let mut elements = OxcVec::with_capacity_in(children.len(), alloc);
            for child in children.drain(..) {
                elements.push(ArrayExpressionElement::from(child));
            }
            Expression::ArrayExpression(OxcBox::new_in(
                ArrayExpression {
                    span: SPAN,
                    elements,
                    node_id: Cell::new(NodeId::DUMMY),
                },
                alloc,
            ))
        };

        fields.push(ObjectPropertyKind::ObjectProperty(OxcBox::new_in(
            ObjectProperty {
                span: SPAN,
                kind: PropertyKind::Init,
                key: PropertyKey::StaticIdentifier(OxcBox::new_in(
                    create_ident_name(alloc, "children"),
                    alloc,
                )),
                value,
                method: false,
                shorthand: false,
                computed: false,
                node_id: Cell::new(NodeId::DUMMY),
            },
            alloc,
        )));
    }

    // Add remaining fields.
    if !fields.is_empty() {
        let props_vec = OxcVec::from_iter_in(fields, alloc);
        objects.push(create_object_expression(alloc, props_vec));
    }

    let props = if objects.is_empty() {
        None
    } else if objects.len() == 1 {
        Some(objects.pop().unwrap())
    } else {
        // Object.assign({}, ...objects) if first is not an object
        let mut args = OxcVec::with_capacity_in(objects.len() + 1, alloc);

        objects.reverse();
        if !matches!(objects.last(), Some(Expression::ObjectExpression(_))) {
            objects.push(create_object_expression(alloc, OxcVec::new_in(alloc)));
        }

        while let Some(object) = objects.pop() {
            args.push(Argument::from(object));
        }

        let callee = create_member_expression_from_str(alloc, "Object.assign");
        Some(create_call_expression(alloc, callee, args))
    };

    Ok((props, key))
}

/// Convert a JSX attribute value to an expression.
fn jsx_attr_value_to_expression<'a>(
    alloc: &'a Allocator,
    value: Option<JSXAttributeValue<'a>>,
) -> Expression<'a> {
    match value {
        None => create_bool_expression(alloc, true),
        Some(JSXAttributeValue::StringLiteral(mut s)) => {
            s.raw = None;
            Expression::StringLiteral(s)
        }
        Some(JSXAttributeValue::ExpressionContainer(container)) => {
            let container = container.unbox();
            match container.expression {
                JSXExpression::EmptyExpression(_) => {
                    unreachable!("Cannot use empty JSX expressions in attribute values");
                }
                _ => jsx_expression_to_expression(container.expression).unwrap(),
            }
        }
        Some(JSXAttributeValue::Element(elem)) => Expression::JSXElement(elem),
        Some(JSXAttributeValue::Fragment(frag)) => Expression::JSXFragment(frag),
    }
}

/// Info gathered from comments.
#[derive(Debug, Default, Clone)]
struct Directives {
    runtime: Option<JsxRuntime>,
    import_source: Option<String>,
    pragma: Option<String>,
    pragma_frag: Option<String>,
}

/// Find directives in comments.
fn find_directives(
    comments: &[MdxComment],
    location: Option<&Location>,
) -> Result<Directives, Message> {
    let mut directives = Directives::default();

    for comment in comments {
        if comment.kind != MdxCommentKind::Block {
            continue;
        }

        let lines = comment.text.lines();

        for line in lines {
            let bytes = line.as_bytes();
            let mut index = 0;
            while index < bytes.len() && matches!(bytes[index], b' ' | b'\t') {
                index += 1;
            }
            if index < bytes.len() && bytes[index] == b'*' {
                index += 1;
                while index < bytes.len() && matches!(bytes[index], b' ' | b'\t') {
                    index += 1;
                }
            }
            if !(index + 4 < bytes.len()
                && bytes[index] == b'@'
                && bytes[index + 1] == b'j'
                && bytes[index + 2] == b's'
                && bytes[index + 3] == b'x')
            {
                continue;
            }

            loop {
                let mut key_range = (index, index);
                while index < bytes.len() && !matches!(bytes[index], b' ' | b'\t') {
                    index += 1;
                }
                key_range.1 = index;
                while index < bytes.len() && matches!(bytes[index], b' ' | b'\t') {
                    index += 1;
                }
                let mut value_range = (index, index);
                while index < bytes.len() && !matches!(bytes[index], b' ' | b'\t') {
                    index += 1;
                }
                value_range.1 = index;

                let key = String::from_utf8_lossy(&bytes[key_range.0..key_range.1]);
                let value = String::from_utf8_lossy(&bytes[value_range.0..value_range.1]);

                match key.as_ref() {
                    "@jsxRuntime" => match value.as_ref() {
                        "automatic" => directives.runtime = Some(JsxRuntime::Automatic),
                        "classic" => directives.runtime = Some(JsxRuntime::Classic),
                        "" => {}
                        val => {
                            return Err(Message {
                                reason: format!(
                                    "Runtime must be either `automatic` or `classic`, not {val}",
                                ),
                                place: u32_to_point(comment.span.start, location)
                                    .map(|p| Box::new(message::Place::Point(p))),
                                source: Box::new("mdxjs-rs".into()),
                                rule_id: Box::new("runtime".into()),
                            });
                        }
                    },
                    "@jsxImportSource" => match value.as_ref() {
                        "" => {}
                        val => {
                            directives.runtime = Some(JsxRuntime::Automatic);
                            directives.import_source = Some(val.into());
                        }
                    },
                    "@jsxFrag" => match value.as_ref() {
                        "" => {}
                        val => directives.pragma_frag = Some(val.into()),
                    },
                    "@jsx" => match value.as_ref() {
                        "" => {}
                        val => directives.pragma = Some(val.into()),
                    },
                    "" => break,
                    _ => {}
                }

                while index < bytes.len() && matches!(bytes[index], b' ' | b'\t') {
                    index += 1;
                }
            }
        }
    }

    Ok(directives)
}

/// Turn JSX text into a string.
fn jsx_text_to_value(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let value = value.replace('\t', " ");
    let bytes = value.as_bytes();
    let mut index = 0;
    let mut start = 0;

    while index < bytes.len() {
        if !matches!(bytes[index], b'\r' | b'\n') {
            index += 1;
            continue;
        }

        let mut before = index;
        while before > start && bytes[before - 1] == b' ' {
            before -= 1;
        }

        if start != before {
            if !result.is_empty() {
                result.push(' ');
            }
            result.push_str(str::from_utf8(&bytes[start..before]).unwrap());
        }

        index += 1;
        while index < bytes.len() && bytes[index] == b' ' {
            index += 1;
        }
        start = index;
    }

    if start != bytes.len() {
        // An all-spaces run with no newline is significant in JSX, so keep it rather than drop it.
        if !result.is_empty() {
            result.push(' ');
        }
        result.push_str(str::from_utf8(&bytes[start..]).unwrap());
    }

    // JSX text content carries HTML entities — `&gt;`, `&amp;`, `&#123;`, … —
    // that the runtime expects to see decoded ("foo > bar", not "foo &gt; bar").
    // Apply after whitespace normalisation so a literal `&#32;` doesn't get
    // folded into surrounding whitespace.
    match decode_html_entities(&result) {
        std::borrow::Cow::Borrowed(_) => result,
        std::borrow::Cow::Owned(decoded) => decoded,
    }
}
