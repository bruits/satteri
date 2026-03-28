//! Rewrite JSX tags to accept them from props and an optional provider.
//!
//! Port of <https://github.com/mdx-js/mdx/blob/main/packages/mdx/lib/plugin/recma-jsx-rewrite.js>,
//! by the same author.

use crate::hast_util_to_oxc::MdxProgram;
use crate::oxc_utils::{
    create_binding_ident, create_bool_expression, create_call_expression, create_ident_expression,
    create_ident_name, create_member, create_object_expression, create_str_expression,
    create_string_literal, is_literal_name, jsx_member_to_parts, span_to_position,
};
use tryckeri_mdast::mdx_types::Location;

use oxc_allocator::{Allocator, Box as OxcBox, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, ArrayExpressionElement, BinaryExpression, BinaryOperator, BindingPattern,
    BindingProperty, ConditionalExpression, Declaration, Expression, ExpressionStatement,
    FormalParameter, FormalParameterKind, FormalParameters, Function, FunctionBody, FunctionType,
    IdentifierReference, IfStatement, ImportDeclaration, ImportDeclarationSpecifier,
    ImportOrExportKind, ImportSpecifier, JSXAttributeItem, JSXAttributeValue, JSXChild, JSXElement,
    JSXElementName, JSXIdentifier, JSXMemberExpression, JSXMemberExpressionObject,
    LogicalExpression, LogicalOperator, ModuleDeclaration, ModuleExportName, NewExpression,
    ObjectPattern, ObjectProperty, ObjectPropertyKind, PropertyKey, PropertyKind, Statement,
    ThrowStatement, UnaryExpression, UnaryOperator, VariableDeclaration, VariableDeclarationKind,
    VariableDeclarator,
};
use oxc_span::{Atom, SPAN, Span};
use oxc_syntax::node::NodeId;
use rustc_hash::{FxHashMap, FxHashSet};
use std::cell::Cell;

/// Configuration.
#[derive(Debug, Default, Clone)]
pub struct Options {
    pub provider_import_source: Option<String>,
    pub development: bool,
}

/// Collected info about a single function scope.
#[derive(Debug, Default)]
struct ScopeInfo {
    /// Tags referenced that are literal (lowercase, like `h1`, `p`).
    /// Maps tag name -> whether ALL occurrences are explicit JSX.
    /// If `true`, all uses are explicit and the tag won't be rewritten to `_components.tag`.
    /// If `false`, at least one non-explicit use exists, so it should be rewritten.
    /// All tags go into the `_components` defaults regardless.
    literal_tags: FxHashMap<String, bool>,
    /// Component references (uppercase, like `Foo`).
    /// The first element of the pair is the root identifier name.
    components: Vec<(String, Span)>,
    /// Member expression objects (like `Foo` in `Foo.Bar`).
    objects: Vec<String>,
    /// Names defined (declared) in this scope.
    defined: FxHashSet<String>,
}

/// Rewrite JSX in an MDX file so that components can be passed in and provided.
///
/// This does several things:
/// 1. For `_createMdxContent`, adds `const _components = Object.assign({...defaults}, props.components)`
/// 2. Rewrites literal JSX tags (`<h1>`) to `<_components.h1>` (unless explicit)
/// 3. Adds component destructuring from `_components` or `props.components`
/// 4. Adds `_missingMdxReference` checks for components
/// 5. For `MDXContent`, rewrites `props.components || {}` to provider-based version if configured
/// 6. Adds provider import and `_missingMdxReference` helper if needed
pub fn mdx_plugin_recma_jsx_rewrite<'a>(
    program: &mut MdxProgram<'a>,
    options: &Options,
    location: Option<&Location>,
    explicit_jsxs: &FxHashSet<Span>,
    allocator: &'a Allocator,
) {
    let has_provider = options.provider_import_source.is_some();

    // We need to track whether we need a `_missingMdxReference` helper.
    let mut need_missing_reference = false;
    let mut need_missing_reference_with_place = false;

    // Collect all names bound by top-level import declarations. These are
    // already in scope for every function in the module and must not be
    // treated as dynamic components that need to be destructured from
    // `_components` / `props.components`.
    let mut top_level_imports: FxHashSet<String> = FxHashSet::default();
    for stmt in &program.program.body {
        if let Statement::ImportDeclaration(import) = stmt
            && let Some(specifiers) = &import.specifiers
        {
            for spec in specifiers {
                let local_name = match spec {
                    ImportDeclarationSpecifier::ImportSpecifier(s) => s.local.name.to_string(),
                    ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => {
                        s.local.name.to_string()
                    }
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) => {
                        s.local.name.to_string()
                    }
                };
                top_level_imports.insert(local_name);
            }
        }
    }

    // Process the program body. We walk through each top-level statement.
    // We care about function declarations: `_createMdxContent` and `MDXContent`.
    let body_len = program.program.body.len();

    for i in 0..body_len {
        let is_create_mdx_content;
        let is_mdx_content;

        // Identify the function
        {
            let stmt = &program.program.body[i];
            let fn_name = get_function_name(stmt);
            is_create_mdx_content = fn_name.as_deref() == Some("_createMdxContent");
            is_mdx_content = fn_name.as_deref() == Some("MDXContent");
        }

        if !is_create_mdx_content && !is_mdx_content {
            continue;
        }

        // Collect JSX references and defined names in this function body.
        let scope = {
            let stmt = &program.program.body[i];
            let body = get_function_body(stmt);
            if let Some(body) = body {
                collect_scope_info(body, explicit_jsxs)
            } else {
                continue;
            }
        };

        if is_mdx_content {
            // For MDXContent: add `const { wrapper: MDXLayout } = ...` at the top of the body.
            let components_init = if has_provider {
                create_components_merge_expr(allocator)
            } else {
                create_props_components_or_empty(allocator)
            };
            let layout_decl = create_wrapper_destructure(allocator, components_init);

            let stmt = &mut program.program.body[i];
            let body = get_function_body_mut(stmt);
            if let Some(body) = body {
                let existing: Vec<Statement<'a>> = body.statements.drain(..).collect();
                body.statements.push(layout_decl);
                for s in existing {
                    body.statements.push(s);
                }
            }
            continue;
        }

        // For _createMdxContent: do the real rewrite work.
        // Determine which tags need to be in _components.
        let mut defaults: Vec<(String, String)> = Vec::new();
        let mut dynamic_components: Vec<(String, Span)> = Vec::new();
        let mut dynamic_objects: Vec<String> = Vec::new();

        for tag in scope.literal_tags.keys() {
            if !scope.defined.contains(tag.as_str()) {
                defaults.push((tag.clone(), tag.clone()));
            }
        }
        defaults.sort_by(|a, b| a.0.cmp(&b.0));

        for (name, span) in &scope.components {
            if !scope.defined.contains(name.as_str()) && !top_level_imports.contains(name.as_str())
            {
                dynamic_components.push((name.clone(), *span));
            }
        }

        for name in &scope.objects {
            if !scope.defined.contains(name.as_str())
                && !top_level_imports.contains(name.as_str())
                && !dynamic_components.iter().any(|(n, _)| n == name)
            {
                dynamic_objects.push(name.clone());
            }
        }

        let has_defaults = !defaults.is_empty();

        // Now rewrite the function body.
        // We need to:
        // 1. Rewrite JSX element names for literal tags (non-explicit)
        // 2. Insert `const _components = ...` at the top (only if there are defaults)
        // 3. Insert `const { Foo, Bar } = _components` (or from props) for dynamic components
        // 4. Insert `if (!Foo) _missingMdxReference(...)` checks
        {
            let stmt = &mut program.program.body[i];
            let body = get_function_body_mut(stmt);
            if let Some(body) = body {
                // Step 1: Rewrite literal JSX tags to _components.tag
                if has_defaults {
                    rewrite_jsx_tags_in_body(body, &scope, explicit_jsxs, allocator);
                }
            }
        }

        // Step 2-4: Build statements to prepend.
        let mut prepend: Vec<Statement<'a>> = Vec::new();

        if has_defaults {
            // `const _components = Object.assign({tag: "tag", ...}, props.components)`
            // or with provider: `Object.assign({tag: "tag"...}, _provideComponents(), props.components)`
            prepend.push(create_components_decl(allocator, &defaults, has_provider));
        }

        if !dynamic_components.is_empty() || !dynamic_objects.is_empty() {
            let mut all_names: Vec<String> =
                dynamic_components.iter().map(|(n, _)| n.clone()).collect();
            for obj_name in &dynamic_objects {
                if !all_names.contains(obj_name) {
                    all_names.push(obj_name.clone());
                }
            }
            all_names.sort();

            if !all_names.is_empty() {
                if has_defaults {
                    // Destructure from _components (already declared above)
                    prepend.push(create_destructure_from_components(allocator, &all_names));
                } else {
                    // No _components — destructure directly from props.components || {} or provider merge
                    prepend.push(create_destructure_from_props(
                        allocator,
                        &all_names,
                        has_provider,
                    ));
                }
            }
        }

        // Missing reference checks for components
        for (name, span) in &dynamic_components {
            need_missing_reference = true;
            let position_str = if options.development {
                need_missing_reference_with_place = true;
                span_to_position(*span, location).map(|p| {
                    format!(
                        "{}:{}-{}:{}",
                        p.start.line, p.start.column, p.end.line, p.end.column
                    )
                })
            } else {
                None
            };
            prepend.push(create_missing_ref_check(
                allocator,
                name,
                true,
                position_str.as_deref(),
            ));
        }

        // Missing reference checks for objects
        for name in &dynamic_objects {
            need_missing_reference = true;
            prepend.push(create_missing_ref_check(allocator, name, false, None));
        }

        // Insert prepend statements at the beginning of the function body
        if !prepend.is_empty() {
            let stmt = &mut program.program.body[i];
            let body = get_function_body_mut(stmt);
            if let Some(body) = body {
                let existing: Vec<Statement<'a>> = body.statements.drain(..).collect();
                for s in prepend {
                    body.statements.push(s);
                }
                for s in existing {
                    body.statements.push(s);
                }
            }
        }
    }

    // Add provider import at the beginning of the program if needed.
    if let Some(ref source) = options.provider_import_source {
        let import_stmt = create_provider_import(allocator, source);
        // Insert after any existing imports.
        let mut insert_pos = 0;
        for (idx, stmt) in program.program.body.iter().enumerate() {
            if matches!(stmt, Statement::ImportDeclaration(_)) {
                insert_pos = idx + 1;
            }
        }
        program.program.body.insert(insert_pos, import_stmt);
    }

    // Add `_missingMdxReference` helper at the end if needed.
    if need_missing_reference {
        let helper = create_missing_ref_helper(
            allocator,
            options.development && need_missing_reference_with_place,
            program.path.as_deref(),
        );
        program.program.body.push(helper);
    }
}

/// Get the name of a function declaration from a statement.
fn get_function_name(stmt: &Statement) -> Option<String> {
    if let Statement::FunctionDeclaration(func) = stmt {
        func.id.as_ref().map(|id| id.name.to_string())
    } else {
        None
    }
}

/// Get an immutable reference to the function body.
fn get_function_body<'a, 'b>(stmt: &'b Statement<'a>) -> Option<&'b FunctionBody<'a>> {
    if let Statement::FunctionDeclaration(func) = stmt {
        func.body.as_deref()
    } else {
        None
    }
}

/// Get a mutable reference to the function body.
fn get_function_body_mut<'a, 'b>(stmt: &'b mut Statement<'a>) -> Option<&'b mut FunctionBody<'a>> {
    if let Statement::FunctionDeclaration(func) = stmt {
        func.body.as_deref_mut()
    } else {
        None
    }
}

/// Collect information about JSX references in a function body.
fn collect_scope_info(body: &FunctionBody, explicit_jsxs: &FxHashSet<Span>) -> ScopeInfo {
    let mut info = ScopeInfo::default();

    // Collect defined names from declarations in the body
    for stmt in &body.statements {
        collect_defined_names(stmt, &mut info.defined);
    }

    // Collect JSX references
    for stmt in &body.statements {
        collect_jsx_refs_in_stmt(stmt, explicit_jsxs, &mut info);
    }

    info
}

/// Collect names defined in a statement (variable declarations, function declarations, etc.)
fn collect_defined_names(stmt: &Statement, defined: &mut FxHashSet<String>) {
    match stmt {
        Statement::VariableDeclaration(decl) => {
            for declarator in &decl.declarations {
                collect_binding_names(&declarator.id, defined);
            }
        }
        Statement::FunctionDeclaration(func) => {
            if let Some(id) = &func.id {
                defined.insert(id.name.to_string());
            }
        }
        Statement::ClassDeclaration(class) => {
            if let Some(id) = &class.id {
                defined.insert(id.name.to_string());
            }
        }
        Statement::ImportDeclaration(import) => {
            if let Some(specifiers) = &import.specifiers {
                for spec in specifiers {
                    match spec {
                        ImportDeclarationSpecifier::ImportSpecifier(s) => {
                            defined.insert(s.local.name.to_string());
                        }
                        ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => {
                            defined.insert(s.local.name.to_string());
                        }
                        ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) => {
                            defined.insert(s.local.name.to_string());
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// Collect binding names from a binding pattern.
fn collect_binding_names(pattern: &BindingPattern, names: &mut FxHashSet<String>) {
    match pattern {
        BindingPattern::BindingIdentifier(id) => {
            names.insert(id.name.to_string());
        }
        BindingPattern::ObjectPattern(obj) => {
            for prop in &obj.properties {
                collect_binding_names(&prop.value, names);
            }
            if let Some(rest) = &obj.rest {
                collect_binding_names(&rest.argument, names);
            }
        }
        BindingPattern::ArrayPattern(arr) => {
            for elem in arr.elements.iter().flatten() {
                collect_binding_names(elem, names);
            }
            if let Some(rest) = &arr.rest {
                collect_binding_names(&rest.argument, names);
            }
        }
        BindingPattern::AssignmentPattern(assign) => {
            collect_binding_names(&assign.left, names);
        }
    }
}

/// Recursively collect JSX references in a statement.
fn collect_jsx_refs_in_stmt(
    stmt: &Statement,
    explicit_jsxs: &FxHashSet<Span>,
    info: &mut ScopeInfo,
) {
    match stmt {
        Statement::ReturnStatement(ret) => {
            if let Some(expr) = &ret.argument {
                collect_jsx_refs_in_expr(expr, explicit_jsxs, info);
            }
        }
        Statement::ExpressionStatement(expr_stmt) => {
            collect_jsx_refs_in_expr(&expr_stmt.expression, explicit_jsxs, info);
        }
        Statement::VariableDeclaration(decl) => {
            for declarator in &decl.declarations {
                if let Some(init) = &declarator.init {
                    collect_jsx_refs_in_expr(init, explicit_jsxs, info);
                }
            }
        }
        Statement::IfStatement(if_stmt) => {
            collect_jsx_refs_in_expr(&if_stmt.test, explicit_jsxs, info);
            collect_jsx_refs_in_stmt(&if_stmt.consequent, explicit_jsxs, info);
            if let Some(alt) = &if_stmt.alternate {
                collect_jsx_refs_in_stmt(alt, explicit_jsxs, info);
            }
        }
        Statement::BlockStatement(block) => {
            for s in &block.body {
                collect_jsx_refs_in_stmt(s, explicit_jsxs, info);
            }
        }
        Statement::ForStatement(for_stmt) => {
            if let Some(body) = Some(&for_stmt.body) {
                collect_jsx_refs_in_stmt(body, explicit_jsxs, info);
            }
        }
        Statement::WhileStatement(while_stmt) => {
            collect_jsx_refs_in_stmt(&while_stmt.body, explicit_jsxs, info);
        }
        _ => {}
    }
}

/// Recursively collect JSX references in an expression.
fn collect_jsx_refs_in_expr(
    expr: &Expression,
    explicit_jsxs: &FxHashSet<Span>,
    info: &mut ScopeInfo,
) {
    match expr {
        Expression::JSXElement(elem) => {
            collect_jsx_refs_in_element(elem, explicit_jsxs, info);
        }
        Expression::JSXFragment(frag) => {
            for child in &frag.children {
                collect_jsx_refs_in_child(child, explicit_jsxs, info);
            }
        }
        Expression::ConditionalExpression(cond) => {
            collect_jsx_refs_in_expr(&cond.test, explicit_jsxs, info);
            collect_jsx_refs_in_expr(&cond.consequent, explicit_jsxs, info);
            collect_jsx_refs_in_expr(&cond.alternate, explicit_jsxs, info);
        }
        Expression::CallExpression(call) => {
            collect_jsx_refs_in_expr(&call.callee, explicit_jsxs, info);
            for arg in &call.arguments {
                match arg {
                    Argument::SpreadElement(spread) => {
                        collect_jsx_refs_in_expr(&spread.argument, explicit_jsxs, info);
                    }
                    _ => {
                        if let Some(expr) = arg.as_expression() {
                            collect_jsx_refs_in_expr(expr, explicit_jsxs, info);
                        }
                    }
                }
            }
        }
        Expression::ArrayExpression(arr) => {
            for elem in &arr.elements {
                match elem {
                    ArrayExpressionElement::SpreadElement(spread) => {
                        collect_jsx_refs_in_expr(&spread.argument, explicit_jsxs, info);
                    }
                    ArrayExpressionElement::Elision(_) => {}
                    _ => {
                        if let Some(expr) = elem.as_expression() {
                            collect_jsx_refs_in_expr(expr, explicit_jsxs, info);
                        }
                    }
                }
            }
        }
        Expression::ObjectExpression(obj) => {
            for prop in &obj.properties {
                match prop {
                    ObjectPropertyKind::ObjectProperty(p) => {
                        collect_jsx_refs_in_expr(&p.value, explicit_jsxs, info);
                    }
                    ObjectPropertyKind::SpreadProperty(spread) => {
                        collect_jsx_refs_in_expr(&spread.argument, explicit_jsxs, info);
                    }
                }
            }
        }
        Expression::LogicalExpression(logical) => {
            collect_jsx_refs_in_expr(&logical.left, explicit_jsxs, info);
            collect_jsx_refs_in_expr(&logical.right, explicit_jsxs, info);
        }
        Expression::BinaryExpression(binary) => {
            collect_jsx_refs_in_expr(&binary.left, explicit_jsxs, info);
            collect_jsx_refs_in_expr(&binary.right, explicit_jsxs, info);
        }
        Expression::UnaryExpression(unary) => {
            collect_jsx_refs_in_expr(&unary.argument, explicit_jsxs, info);
        }
        Expression::SequenceExpression(seq) => {
            for e in &seq.expressions {
                collect_jsx_refs_in_expr(e, explicit_jsxs, info);
            }
        }
        Expression::ParenthesizedExpression(paren) => {
            collect_jsx_refs_in_expr(&paren.expression, explicit_jsxs, info);
        }
        Expression::ArrowFunctionExpression(arrow) => {
            // Recurse into arrow function body statements.
            for s in &arrow.body.statements {
                collect_jsx_refs_in_stmt(s, explicit_jsxs, info);
            }
        }
        Expression::AssignmentExpression(assign) => {
            collect_jsx_refs_in_expr(&assign.right, explicit_jsxs, info);
        }
        Expression::TemplateLiteral(tmpl) => {
            for expr in &tmpl.expressions {
                collect_jsx_refs_in_expr(expr, explicit_jsxs, info);
            }
        }
        _ => {}
    }
}

/// Collect JSX refs from a JSX element.
fn collect_jsx_refs_in_element(
    elem: &JSXElement,
    explicit_jsxs: &FxHashSet<Span>,
    info: &mut ScopeInfo,
) {
    let is_explicit = explicit_jsxs.contains(&elem.span);

    match &elem.opening_element.name {
        JSXElementName::Identifier(ident) => {
            // Lowercase identifier like `h1`, `p`, `div` — it's a literal tag.
            let name = ident.name.as_str();
            if is_literal_name(name) {
                let entry = info
                    .literal_tags
                    .entry(name.to_string())
                    .or_insert(is_explicit);
                if !is_explicit {
                    *entry = false;
                }
            }
        }
        JSXElementName::IdentifierReference(ident) => {
            let name = ident.name.as_str();
            if is_literal_name(name) {
                // Literal tag referenced through IdentifierReference
                let entry = info
                    .literal_tags
                    .entry(name.to_string())
                    .or_insert(is_explicit);
                if !is_explicit {
                    *entry = false;
                }
            } else {
                // Component (uppercase) like `Foo`
                if !info.components.iter().any(|(n, _)| n == name) {
                    info.components.push((name.to_string(), elem.span));
                }
            }
        }
        JSXElementName::MemberExpression(member_expr) => {
            // Something like `Foo.Bar` — track the root object
            let parts = jsx_member_to_parts(member_expr);
            if let Some(root) = parts.first()
                && *root != "this"
                && !info.objects.contains(&root.to_string())
            {
                info.objects.push(root.to_string());
            }
        }
        JSXElementName::NamespacedName(_) | JSXElementName::ThisExpression(_) => {
            // Namespace names like `svg:rect` and `this` are left as-is
        }
    }

    // Collect from attributes
    for attr in &elem.opening_element.attributes {
        match attr {
            JSXAttributeItem::Attribute(a) => {
                if let Some(value) = &a.value {
                    match value {
                        JSXAttributeValue::ExpressionContainer(container) => {
                            if let Some(e) = container.expression.as_expression() {
                                collect_jsx_refs_in_expr(e, explicit_jsxs, info);
                            }
                        }
                        JSXAttributeValue::Element(elem) => {
                            collect_jsx_refs_in_element(elem, explicit_jsxs, info);
                        }
                        JSXAttributeValue::Fragment(frag) => {
                            for child in &frag.children {
                                collect_jsx_refs_in_child(child, explicit_jsxs, info);
                            }
                        }
                        JSXAttributeValue::StringLiteral(_) => {}
                    }
                }
            }
            JSXAttributeItem::SpreadAttribute(spread) => {
                collect_jsx_refs_in_expr(&spread.argument, explicit_jsxs, info);
            }
        }
    }

    // Collect from children
    for child in &elem.children {
        collect_jsx_refs_in_child(child, explicit_jsxs, info);
    }
}

/// Collect JSX refs from a JSX child.
fn collect_jsx_refs_in_child(
    child: &JSXChild,
    explicit_jsxs: &FxHashSet<Span>,
    info: &mut ScopeInfo,
) {
    match child {
        JSXChild::Element(elem) => {
            collect_jsx_refs_in_element(elem, explicit_jsxs, info);
        }
        JSXChild::Fragment(frag) => {
            for c in &frag.children {
                collect_jsx_refs_in_child(c, explicit_jsxs, info);
            }
        }
        JSXChild::ExpressionContainer(container) => {
            if let Some(e) = container.expression.as_expression() {
                collect_jsx_refs_in_expr(e, explicit_jsxs, info);
            }
        }
        JSXChild::Spread(spread) => {
            collect_jsx_refs_in_expr(&spread.expression, explicit_jsxs, info);
        }
        JSXChild::Text(_) => {}
    }
}

/// Rewrite JSX tags in a function body.
/// Turns `<h1>` into `<_components.h1>` for non-explicit literal tags.
fn rewrite_jsx_tags_in_body<'a>(
    body: &mut FunctionBody<'a>,
    scope: &ScopeInfo,
    explicit_jsxs: &FxHashSet<Span>,
    allocator: &'a Allocator,
) {
    for stmt in &mut body.statements {
        rewrite_jsx_tags_in_stmt(stmt, scope, explicit_jsxs, allocator);
    }
}

/// Rewrite JSX tags in a statement.
fn rewrite_jsx_tags_in_stmt<'a>(
    stmt: &mut Statement<'a>,
    scope: &ScopeInfo,
    explicit_jsxs: &FxHashSet<Span>,
    allocator: &'a Allocator,
) {
    match stmt {
        Statement::ReturnStatement(ret) => {
            if let Some(expr) = &mut ret.argument {
                rewrite_jsx_tags_in_expr(expr, scope, explicit_jsxs, allocator);
            }
        }
        Statement::ExpressionStatement(expr_stmt) => {
            rewrite_jsx_tags_in_expr(&mut expr_stmt.expression, scope, explicit_jsxs, allocator);
        }
        Statement::VariableDeclaration(decl) => {
            for declarator in &mut decl.declarations {
                if let Some(init) = &mut declarator.init {
                    rewrite_jsx_tags_in_expr(init, scope, explicit_jsxs, allocator);
                }
            }
        }
        Statement::IfStatement(if_stmt) => {
            rewrite_jsx_tags_in_stmt(&mut if_stmt.consequent, scope, explicit_jsxs, allocator);
            if let Some(alt) = &mut if_stmt.alternate {
                rewrite_jsx_tags_in_stmt(alt, scope, explicit_jsxs, allocator);
            }
        }
        Statement::BlockStatement(block) => {
            for s in &mut block.body {
                rewrite_jsx_tags_in_stmt(s, scope, explicit_jsxs, allocator);
            }
        }
        _ => {}
    }
}

/// Rewrite JSX tags in an expression.
fn rewrite_jsx_tags_in_expr<'a>(
    expr: &mut Expression<'a>,
    scope: &ScopeInfo,
    explicit_jsxs: &FxHashSet<Span>,
    allocator: &'a Allocator,
) {
    match expr {
        Expression::JSXElement(elem) => {
            rewrite_jsx_element(elem, scope, explicit_jsxs, allocator);
        }
        Expression::JSXFragment(frag) => {
            for child in &mut frag.children {
                rewrite_jsx_child(child, scope, explicit_jsxs, allocator);
            }
        }
        Expression::ConditionalExpression(cond) => {
            rewrite_jsx_tags_in_expr(&mut cond.test, scope, explicit_jsxs, allocator);
            rewrite_jsx_tags_in_expr(&mut cond.consequent, scope, explicit_jsxs, allocator);
            rewrite_jsx_tags_in_expr(&mut cond.alternate, scope, explicit_jsxs, allocator);
        }
        Expression::CallExpression(call) => {
            rewrite_jsx_tags_in_expr(&mut call.callee, scope, explicit_jsxs, allocator);
            for arg in &mut call.arguments {
                match arg {
                    Argument::SpreadElement(spread) => {
                        rewrite_jsx_tags_in_expr(
                            &mut spread.argument,
                            scope,
                            explicit_jsxs,
                            allocator,
                        );
                    }
                    _ => {
                        if let Some(expr) = arg.as_expression_mut() {
                            rewrite_jsx_tags_in_expr(expr, scope, explicit_jsxs, allocator);
                        }
                    }
                }
            }
        }
        Expression::ArrayExpression(arr) => {
            for elem in &mut arr.elements {
                match elem {
                    ArrayExpressionElement::SpreadElement(spread) => {
                        rewrite_jsx_tags_in_expr(
                            &mut spread.argument,
                            scope,
                            explicit_jsxs,
                            allocator,
                        );
                    }
                    ArrayExpressionElement::Elision(_) => {}
                    _ => {
                        if let Some(expr) = elem.as_expression_mut() {
                            rewrite_jsx_tags_in_expr(expr, scope, explicit_jsxs, allocator);
                        }
                    }
                }
            }
        }
        Expression::ObjectExpression(obj) => {
            for prop in &mut obj.properties {
                match prop {
                    ObjectPropertyKind::ObjectProperty(p) => {
                        rewrite_jsx_tags_in_expr(&mut p.value, scope, explicit_jsxs, allocator);
                    }
                    ObjectPropertyKind::SpreadProperty(spread) => {
                        rewrite_jsx_tags_in_expr(
                            &mut spread.argument,
                            scope,
                            explicit_jsxs,
                            allocator,
                        );
                    }
                }
            }
        }
        Expression::LogicalExpression(logical) => {
            rewrite_jsx_tags_in_expr(&mut logical.left, scope, explicit_jsxs, allocator);
            rewrite_jsx_tags_in_expr(&mut logical.right, scope, explicit_jsxs, allocator);
        }
        Expression::ArrowFunctionExpression(arrow) => {
            for s in &mut arrow.body.statements {
                rewrite_jsx_tags_in_stmt(s, scope, explicit_jsxs, allocator);
            }
        }
        Expression::ParenthesizedExpression(paren) => {
            rewrite_jsx_tags_in_expr(&mut paren.expression, scope, explicit_jsxs, allocator);
        }
        _ => {}
    }
}

/// Rewrite a JSX element's tag name if it's a non-explicit literal.
fn rewrite_jsx_element<'a>(
    elem: &mut OxcBox<'a, JSXElement<'a>>,
    scope: &ScopeInfo,
    explicit_jsxs: &FxHashSet<Span>,
    allocator: &'a Allocator,
) {
    let is_explicit = explicit_jsxs.contains(&elem.span);

    if !is_explicit {
        let tag_name = get_jsx_element_tag_name(&elem.opening_element.name);
        if let Some(name) = tag_name
            && is_literal_name(&name)
            && !scope.defined.contains(&name)
            && let Some(is_only_explicit) = scope.literal_tags.get(&name)
            && !is_only_explicit
        {
            // Rewrite to `_components.tagName`
            let new_name = create_jsx_member_name(allocator, "_components", &name);
            if let Some(closing) = &mut elem.closing_element {
                closing.name = create_jsx_member_name(allocator, "_components", &name);
            }
            elem.opening_element.name = new_name;
        }
    }

    // Recurse into attributes
    for attr in &mut elem.opening_element.attributes {
        match attr {
            JSXAttributeItem::Attribute(a) => {
                if let Some(value) = &mut a.value {
                    match value {
                        JSXAttributeValue::ExpressionContainer(container) => {
                            if let Some(e) = container.expression.as_expression_mut() {
                                rewrite_jsx_tags_in_expr(e, scope, explicit_jsxs, allocator);
                            }
                        }
                        JSXAttributeValue::Element(child_elem) => {
                            rewrite_jsx_element(child_elem, scope, explicit_jsxs, allocator);
                        }
                        JSXAttributeValue::Fragment(frag) => {
                            for child in &mut frag.children {
                                rewrite_jsx_child(child, scope, explicit_jsxs, allocator);
                            }
                        }
                        JSXAttributeValue::StringLiteral(_) => {}
                    }
                }
            }
            JSXAttributeItem::SpreadAttribute(spread) => {
                rewrite_jsx_tags_in_expr(&mut spread.argument, scope, explicit_jsxs, allocator);
            }
        }
    }

    // Recurse into children
    for child in &mut elem.children {
        rewrite_jsx_child(child, scope, explicit_jsxs, allocator);
    }
}

/// Rewrite JSX tags in a child.
fn rewrite_jsx_child<'a>(
    child: &mut JSXChild<'a>,
    scope: &ScopeInfo,
    explicit_jsxs: &FxHashSet<Span>,
    allocator: &'a Allocator,
) {
    match child {
        JSXChild::Element(elem) => {
            rewrite_jsx_element(elem, scope, explicit_jsxs, allocator);
        }
        JSXChild::Fragment(frag) => {
            for c in &mut frag.children {
                rewrite_jsx_child(c, scope, explicit_jsxs, allocator);
            }
        }
        JSXChild::ExpressionContainer(container) => {
            if let Some(e) = container.expression.as_expression_mut() {
                rewrite_jsx_tags_in_expr(e, scope, explicit_jsxs, allocator);
            }
        }
        JSXChild::Spread(spread) => {
            rewrite_jsx_tags_in_expr(&mut spread.expression, scope, explicit_jsxs, allocator);
        }
        JSXChild::Text(_) => {}
    }
}

/// Get the tag name string from a JSX element name (only for simple identifiers).
fn get_jsx_element_tag_name(name: &JSXElementName) -> Option<String> {
    match name {
        JSXElementName::Identifier(ident) => Some(ident.name.to_string()),
        JSXElementName::IdentifierReference(ident) => Some(ident.name.to_string()),
        _ => None,
    }
}

/// Create a JSX member expression name like `_components.h1`.
fn create_jsx_member_name<'a>(
    allocator: &'a Allocator,
    object: &str,
    property: &str,
) -> JSXElementName<'a> {
    JSXElementName::MemberExpression(OxcBox::new_in(
        JSXMemberExpression {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            object: JSXMemberExpressionObject::IdentifierReference(OxcBox::new_in(
                IdentifierReference {
                    node_id: Cell::new(NodeId::DUMMY),
                    span: SPAN,
                    name: Atom::from(allocator.alloc_str(object)).into(),
                    reference_id: Cell::default(),
                },
                allocator,
            )),
            property: JSXIdentifier {
                node_id: Cell::new(NodeId::DUMMY),
                span: SPAN,
                name: Atom::from(allocator.alloc_str(property)),
            },
        },
        allocator,
    ))
}

/// Create `const _components = Object.assign({tag: "tag", ...}, props.components)`
/// or with provider: `Object.assign({...}, _provideComponents(), props.components)`
fn create_components_decl<'a>(
    alloc: &'a Allocator,
    defaults: &[(String, String)],
    has_provider: bool,
) -> Statement<'a> {
    let init = create_components_init_expr(alloc, defaults, has_provider);

    let mut decls = OxcVec::with_capacity_in(1, alloc);
    decls.push(VariableDeclarator {
        node_id: Cell::new(NodeId::DUMMY),
        span: SPAN,
        kind: VariableDeclarationKind::Const,
        id: BindingPattern::BindingIdentifier(OxcBox::new_in(
            create_binding_ident(alloc, "_components"),
            alloc,
        )),
        type_annotation: None,
        init: Some(init),
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

/// Create the initializer for `_components`: `Object.assign({defaults}, ...)`.
fn create_components_init_expr<'a>(
    alloc: &'a Allocator,
    defaults: &[(String, String)],
    has_provider: bool,
) -> Expression<'a> {
    // Build the defaults object: {h1: "h1", p: "p", ...}
    let mut props = OxcVec::with_capacity_in(defaults.len(), alloc);
    for (key, value) in defaults {
        props.push(ObjectPropertyKind::ObjectProperty(OxcBox::new_in(
            ObjectProperty {
                node_id: Cell::new(NodeId::DUMMY),
                span: SPAN,
                kind: PropertyKind::Init,
                key: PropertyKey::StaticIdentifier(OxcBox::new_in(
                    create_ident_name(alloc, key),
                    alloc,
                )),
                value: create_str_expression(alloc, value),
                method: false,
                shorthand: false,
                computed: false,
            },
            alloc,
        )));
    }
    let defaults_obj = create_object_expression(alloc, props);

    // Build Object.assign({...}, [_provideComponents(),] props.components)
    let mut args = OxcVec::with_capacity_in(if has_provider { 3 } else { 2 }, alloc);
    args.push(Argument::from(defaults_obj));

    if has_provider {
        // _provideComponents()
        let provide_call = create_call_expression(
            alloc,
            create_ident_expression(alloc, "_provideComponents"),
            OxcVec::new_in(alloc),
        );
        args.push(Argument::from(provide_call));
    }

    // props.components
    args.push(Argument::from(create_member(
        alloc,
        create_ident_expression(alloc, "props"),
        "components",
    )));

    create_call_expression(
        alloc,
        create_member(alloc, create_ident_expression(alloc, "Object"), "assign"),
        args,
    )
}

/// Create `const { Foo, Bar } = _components`
fn create_destructure_from_components<'a>(alloc: &'a Allocator, names: &[String]) -> Statement<'a> {
    create_destructure_decl(alloc, names, create_ident_expression(alloc, "_components"))
}

/// Create `const { Foo, Bar } = props.components || {}`
fn create_destructure_from_props<'a>(
    alloc: &'a Allocator,
    names: &[String],
    has_provider: bool,
) -> Statement<'a> {
    let init = if has_provider {
        create_components_merge_expr(alloc)
    } else {
        create_props_components_or_empty(alloc)
    };
    create_destructure_decl(alloc, names, init)
}

/// Create a destructuring variable declaration: `const { a, b } = expr`
fn create_destructure_decl<'a>(
    alloc: &'a Allocator,
    names: &[String],
    init: Expression<'a>,
) -> Statement<'a> {
    let mut properties = OxcVec::with_capacity_in(names.len(), alloc);
    for name in names {
        properties.push(BindingProperty {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            key: PropertyKey::StaticIdentifier(OxcBox::new_in(
                create_ident_name(alloc, name),
                alloc,
            )),
            value: BindingPattern::BindingIdentifier(OxcBox::new_in(
                create_binding_ident(alloc, name),
                alloc,
            )),
            shorthand: true,
            computed: false,
        });
    }

    let pattern = BindingPattern::ObjectPattern(OxcBox::new_in(
        ObjectPattern {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            properties,
            rest: None,
        },
        alloc,
    ));

    let mut decls = OxcVec::with_capacity_in(1, alloc);
    decls.push(VariableDeclarator {
        node_id: Cell::new(NodeId::DUMMY),
        span: SPAN,
        kind: VariableDeclarationKind::Const,
        id: pattern,
        type_annotation: None,
        init: Some(init),
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

/// Create `props.components || {}`
fn create_props_components_or_empty(alloc: &Allocator) -> Expression<'_> {
    Expression::LogicalExpression(OxcBox::new_in(
        LogicalExpression {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            left: create_member(alloc, create_ident_expression(alloc, "props"), "components"),
            right: create_object_expression(alloc, OxcVec::new_in(alloc)),
            operator: LogicalOperator::Or,
        },
        alloc,
    ))
}

/// Create `const { wrapper: MDXLayout } = expr`
fn create_wrapper_destructure<'a>(alloc: &'a Allocator, init: Expression<'a>) -> Statement<'a> {
    let mut properties = OxcVec::with_capacity_in(1, alloc);
    properties.push(BindingProperty {
        node_id: Cell::new(NodeId::DUMMY),
        span: SPAN,
        key: PropertyKey::StaticIdentifier(OxcBox::new_in(
            create_ident_name(alloc, "wrapper"),
            alloc,
        )),
        value: BindingPattern::BindingIdentifier(OxcBox::new_in(
            create_binding_ident(alloc, "MDXLayout"),
            alloc,
        )),
        shorthand: false,
        computed: false,
    });

    let pattern = BindingPattern::ObjectPattern(OxcBox::new_in(
        ObjectPattern {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            properties,
            rest: None,
        },
        alloc,
    ));

    let mut decls = OxcVec::with_capacity_in(1, alloc);
    decls.push(VariableDeclarator {
        node_id: Cell::new(NodeId::DUMMY),
        span: SPAN,
        kind: VariableDeclarationKind::Const,
        id: pattern,
        type_annotation: None,
        init: Some(init),
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

/// Create `Object.assign({}, _provideComponents(), props.components)`.
fn create_components_merge_expr(alloc: &Allocator) -> Expression<'_> {
    let mut args = OxcVec::with_capacity_in(3, alloc);
    args.push(Argument::from(create_object_expression(
        alloc,
        OxcVec::new_in(alloc),
    )));
    args.push(Argument::from(create_call_expression(
        alloc,
        create_ident_expression(alloc, "_provideComponents"),
        OxcVec::new_in(alloc),
    )));
    args.push(Argument::from(create_member(
        alloc,
        create_ident_expression(alloc, "props"),
        "components",
    )));

    create_call_expression(
        alloc,
        create_member(alloc, create_ident_expression(alloc, "Object"), "assign"),
        args,
    )
}

/// Create `if (!Foo) _missingMdxReference("Foo", true)` or with place.
fn create_missing_ref_check<'a>(
    alloc: &'a Allocator,
    name: &str,
    is_component: bool,
    place: Option<&str>,
) -> Statement<'a> {
    let test = Expression::UnaryExpression(OxcBox::new_in(
        UnaryExpression {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            operator: UnaryOperator::LogicalNot,
            argument: create_ident_expression(alloc, name),
        },
        alloc,
    ));

    let mut args = OxcVec::with_capacity_in(if place.is_some() { 3 } else { 2 }, alloc);
    args.push(Argument::from(create_str_expression(alloc, name)));
    args.push(Argument::from(create_bool_expression(alloc, is_component)));
    if let Some(place) = place {
        args.push(Argument::from(create_str_expression(alloc, place)));
    }

    let call = create_call_expression(
        alloc,
        create_ident_expression(alloc, "_missingMdxReference"),
        args,
    );

    Statement::IfStatement(OxcBox::new_in(
        IfStatement {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            test,
            consequent: Statement::ExpressionStatement(OxcBox::new_in(
                ExpressionStatement {
                    node_id: Cell::new(NodeId::DUMMY),
                    span: SPAN,
                    expression: call,
                },
                alloc,
            )),
            alternate: None,
        },
        alloc,
    ))
}

/// Create `import { useMDXComponents as _provideComponents } from "source"`
fn create_provider_import<'a>(alloc: &'a Allocator, source: &str) -> Statement<'a> {
    let mut specifiers = OxcVec::with_capacity_in(1, alloc);
    specifiers.push(ImportDeclarationSpecifier::ImportSpecifier(OxcBox::new_in(
        ImportSpecifier {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            imported: ModuleExportName::IdentifierName(create_ident_name(
                alloc,
                "useMDXComponents",
            )),
            local: create_binding_ident(alloc, "_provideComponents"),
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

/// Create the `_missingMdxReference` helper function.
///
/// Without place:
/// ```js
/// function _missingMdxReference(id, component) {
///   throw new Error("Expected " + (component ? "component" : "object") + " `" + id + "` to be defined: you likely forgot to import, pass, or provide it.");
/// }
/// ```
///
/// With place:
/// ```js
/// function _missingMdxReference(id, component, place) {
///   throw new Error("Expected " + (component ? "component" : "object") + " `" + id + "` to be defined: you likely forgot to import, pass, or provide it." + (place ? "\nIt's referenced in your code at `" + place + "` in `" + filepath + "`" : ""));
/// }
/// ```
fn create_missing_ref_helper<'a>(
    alloc: &'a Allocator,
    with_place: bool,
    filepath: Option<&str>,
) -> Statement<'a> {
    let mut params_list = OxcVec::with_capacity_in(if with_place { 3 } else { 2 }, alloc);
    params_list.push(FormalParameter {
        node_id: Cell::new(NodeId::DUMMY),
        span: SPAN,
        decorators: OxcVec::new_in(alloc),
        pattern: BindingPattern::BindingIdentifier(OxcBox::new_in(
            create_binding_ident(alloc, "id"),
            alloc,
        )),
        type_annotation: None,
        initializer: None,
        optional: false,
        accessibility: None,
        readonly: false,
        r#override: false,
    });
    params_list.push(FormalParameter {
        node_id: Cell::new(NodeId::DUMMY),
        span: SPAN,
        decorators: OxcVec::new_in(alloc),
        pattern: BindingPattern::BindingIdentifier(OxcBox::new_in(
            create_binding_ident(alloc, "component"),
            alloc,
        )),
        type_annotation: None,
        initializer: None,
        optional: false,
        accessibility: None,
        readonly: false,
        r#override: false,
    });
    if with_place {
        params_list.push(FormalParameter {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            decorators: OxcVec::new_in(alloc),
            pattern: BindingPattern::BindingIdentifier(OxcBox::new_in(
                create_binding_ident(alloc, "place"),
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

    // Build the error message expression.
    // "Expected " + (component ? "component" : "object") + " `" + id + "` to be defined: ..."
    let ternary = Expression::ConditionalExpression(OxcBox::new_in(
        ConditionalExpression {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            test: create_ident_expression(alloc, "component"),
            consequent: create_str_expression(alloc, "component"),
            alternate: create_str_expression(alloc, "object"),
        },
        alloc,
    ));

    // Build the concatenation:
    // "Expected " + ternary + " `" + id + "` to be defined: you likely forgot to import, pass, or provide it."
    let mut message = create_str_expression(alloc, "Expected ");
    message = create_binary_add(alloc, message, ternary);
    message = create_binary_add(alloc, message, create_str_expression(alloc, " `"));
    message = create_binary_add(alloc, message, create_ident_expression(alloc, "id"));
    message = create_binary_add(
        alloc,
        message,
        create_str_expression(
            alloc,
            "` to be defined: you likely forgot to import, pass, or provide it.",
        ),
    );

    if with_place {
        // + (place ? "\nIt's referenced in your code at `" + place + "` in `filepath`" : "")
        let filepath_str = filepath.unwrap_or("");

        let place_msg = {
            let mut m = create_str_expression(alloc, "\nIt\u{2019}s referenced in your code at `");
            m = create_binary_add(alloc, m, create_ident_expression(alloc, "place"));
            m = create_binary_add(
                alloc,
                m,
                create_str_expression(alloc, &format!("` in `{filepath_str}`")),
            );
            m
        };

        let place_ternary = Expression::ConditionalExpression(OxcBox::new_in(
            ConditionalExpression {
                node_id: Cell::new(NodeId::DUMMY),
                span: SPAN,
                test: create_ident_expression(alloc, "place"),
                consequent: place_msg,
                alternate: create_str_expression(alloc, ""),
            },
            alloc,
        ));

        message = create_binary_add(alloc, message, place_ternary);
    }

    // `new Error(message)`
    let mut error_args = OxcVec::with_capacity_in(1, alloc);
    error_args.push(Argument::from(message));

    let new_error = Expression::NewExpression(OxcBox::new_in(
        NewExpression {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            callee: create_ident_expression(alloc, "Error"),
            arguments: error_args,
            type_arguments: None,
            pure: false,
        },
        alloc,
    ));

    // `throw new Error(...)`
    let throw = Statement::ThrowStatement(OxcBox::new_in(
        ThrowStatement {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            argument: new_error,
        },
        alloc,
    ));

    let mut body_stmts = OxcVec::with_capacity_in(1, alloc);
    body_stmts.push(throw);

    Statement::from(Declaration::FunctionDeclaration(OxcBox::new_in(
        Function {
            node_id: Cell::new(NodeId::DUMMY),
            r#type: FunctionType::FunctionDeclaration,
            span: SPAN,
            id: Some(create_binding_ident(alloc, "_missingMdxReference")),
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
                    items: params_list,
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
                    statements: body_stmts,
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

/// Create a binary `+` expression.
fn create_binary_add<'a>(
    alloc: &'a Allocator,
    left: Expression<'a>,
    right: Expression<'a>,
) -> Expression<'a> {
    Expression::BinaryExpression(OxcBox::new_in(
        BinaryExpression {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            left,
            right,
            operator: BinaryOperator::Addition,
        },
        alloc,
    ))
}
