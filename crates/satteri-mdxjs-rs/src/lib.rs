//! Public API of `mdxjs-rs`.
//!
//! *   [`compile()`][]: turn MDX into JavaScript
#![deny(clippy::pedantic)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]

mod configuration;
mod hast_util_to_oxc;
mod mdx_plugin_recma_document;
mod mdx_plugin_recma_jsx_rewrite;
mod oxc;
mod oxc_util_build_jsx;
mod oxc_utils;

use crate::{
    hast_util_to_oxc::{MdxProgram, hast_util_to_oxc},
    mdx_plugin_recma_document::{
        Options as DocumentOptions, mdx_plugin_recma_document as recma_document,
    },
    mdx_plugin_recma_jsx_rewrite::{
        Options as RewriteOptions, mdx_plugin_recma_jsx_rewrite as recma_jsx_rewrite,
    },
    oxc::serialize,
    oxc_util_build_jsx::{Options as BuildOptions, oxc_util_build_jsx},
    oxc_utils::{
        create_binding_ident, create_ident_expression, create_ident_name, create_num_expression,
        create_object_expression, create_string_literal,
    },
};
use oxc_allocator::{Allocator, Box as OxcBox, Vec as OxcVec};
use oxc_ast::ast::{
    BindingPattern, BindingProperty, Declaration, Directive, ExportSpecifier, Expression,
    ImportDeclarationSpecifier, ModuleExportName, ObjectProperty, ObjectPropertyKind, PropertyKey,
    PropertyKind, ReturnStatement, Statement, VariableDeclaration, VariableDeclarationKind,
    VariableDeclarator,
};
use oxc_estree::{CompactJSSerializer, ESTree};
use oxc_parser::{ParseOptions, Parser};
use oxc_span::{Atom, SPAN, SourceType, Span};
use oxc_syntax::node::NodeId;
use rustc_hash::FxHashSet;
use satteri_arena::mdx_types::{self as message, Location};
use std::cell::Cell;

pub use crate::configuration::{
    ElementAttributeNameCase, MdxConstructs, MdxParseOptions, OptimizeStaticConfig, Options,
    OutputFormat, StylePropertyNameCase,
};
pub use crate::mdx_plugin_recma_document::JsxRuntime;

/// Parse a JavaScript expression and return its ESTree-compatible JSON representation.
///
/// Wraps the expression in an `ExpressionStatement` inside a `Program` to match
/// the standard `ESTree` `Program` shape that tools like `estree-util-*` expect.
///
/// Returns `None` if parsing fails (e.g. invalid syntax).
#[must_use]
pub fn parse_expression_to_estree_json(source: &str) -> Option<String> {
    let allocator = Allocator::default();
    let source_type = SourceType::mjs().with_jsx(true);
    let src = allocator.alloc_str(source);
    let expr = Parser::new(&allocator, src, source_type)
        .with_options(ParseOptions::default())
        .parse_expression()
        .ok()?;

    let mut serializer = CompactJSSerializer::new(false);
    expr.serialize(&mut serializer);
    let expr_json = serializer.into_string();

    // Wrap in Program > ExpressionStatement to match ESTree Program shape
    Some(format!(
        r#"{{"type":"Program","body":[{{"type":"ExpressionStatement","expression":{expr_json}}}],"sourceType":"module"}}"#,
    ))
}

/// Parse ESM (import/export statements) and return its ESTree-compatible JSON.
///
/// Returns `None` if parsing fails.
#[must_use]
pub fn parse_esm_to_estree_json(source: &str) -> Option<String> {
    let allocator = Allocator::default();
    let source_type = SourceType::mjs().with_jsx(true);
    let src = allocator.alloc_str(source);
    let ret = Parser::new(&allocator, src, source_type)
        .with_options(ParseOptions::default())
        .parse();

    if !ret.errors.is_empty() {
        return None;
    }

    let mut serializer = CompactJSSerializer::new(false);
    ret.program.serialize(&mut serializer);
    Some(serializer.into_string())
}

/// Turn MDX into JavaScript.
///
/// ## Examples
///
/// ```
/// use satteri_mdxjs::compile;
/// # fn main() -> Result<(), satteri_arena::mdx_types::Message> {
///
/// let result = compile("# Hi!", &Default::default(), satteri_pulldown_cmark::MDX_OPTIONS)?;
/// assert!(result.contains("function _createMdxContent"));
/// # Ok(())
/// # }
/// ```
///
/// ## Errors
///
/// This project errors for many different reasons, such as syntax errors in
/// the MDX format or misconfiguration.
pub fn compile(
    value: &str,
    options: &Options,
    parse_options: satteri_pulldown_cmark::Options,
) -> Result<String, message::Message> {
    compile_with_convert_options(
        value,
        options,
        parse_options,
        &satteri_ast::hast::ConvertOptions::default(),
    )
}

/// Compile MDX source with caller-supplied mdast→hast conversion options
/// (footnote i18n strings, etc.).
///
/// # Errors
///
/// Same conditions as [`compile`].
pub fn compile_with_convert_options(
    value: &str,
    options: &Options,
    parse_options: satteri_pulldown_cmark::Options,
    convert_options: &satteri_ast::hast::ConvertOptions,
) -> Result<String, message::Message> {
    let normalised;
    let value = if value.contains('\t') {
        normalised = expand_tabs(value);
        &normalised as &str
    } else {
        value
    };
    let (arena, mdx_errors) = satteri_pulldown_cmark::parse(value, parse_options);
    if let Some((offset, msg)) = mdx_errors.first() {
        return Err(parse_error_to_message(value, *offset, msg));
    }
    let hast_arena =
        satteri_ast::hast::mdast_arena_to_hast_arena_with_options(&arena, convert_options);
    compile_hast_arena(&hast_arena, options)
}

/// Compile a HAST arena directly to JavaScript.
///
/// The arena can be mutated before calling (e.g. `simplify_plain_mdx_nodes`).
///
/// # Errors
///
/// Returns an error if compilation fails (e.g. invalid MDX expressions).
pub fn compile_hast_arena(
    arena: &satteri_arena::Arena<satteri_arena::Hast>,
    options: &Options,
) -> Result<String, message::Message> {
    let source_bytes = arena.source().as_bytes();
    let allocator = Allocator::default();
    let location = Location::new(source_bytes);
    let mut explicit_jsxs = FxHashSet::default();
    let mut program = hast_util_to_oxc(
        arena,
        options.filepath.clone(),
        Some(&location),
        &mut explicit_jsxs,
        &allocator,
        options.optimize_static.as_ref(),
        options.element_attribute_name_case,
        options.style_property_name_case,
    )?;
    mdx_plugin_recma_document(&mut program, options, Some(&location), &allocator)?;
    mdx_plugin_recma_jsx_rewrite(
        &mut program,
        options,
        Some(&location),
        &explicit_jsxs,
        &allocator,
    )?;
    if options.output_format == OutputFormat::FunctionBody {
        transform_program_to_function_body(&mut program, &allocator);
    }
    Ok(serialize(&program.program))
}

fn transform_program_to_function_body<'a>(program: &mut MdxProgram<'a>, allocator: &'a Allocator) {
    let body = std::mem::replace(&mut program.program.body, OxcVec::new_in(allocator));
    let mut new_body: Vec<Statement<'a>> = Vec::with_capacity(body.len());
    // (exported_name, local_name) pairs
    let mut exports: Vec<(String, String)> = Vec::new();

    program.program.directives = {
        let mut directives = OxcVec::with_capacity_in(1, allocator);
        directives.push(Directive {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            expression: create_string_literal(allocator, "use strict"),
            directive: Atom::from(allocator.alloc_str("use strict")),
        });
        directives
    };

    for stmt in body {
        match stmt {
            Statement::ImportDeclaration(import_decl) => {
                let import_decl = import_decl.unbox();
                if let Some(specifiers) = &import_decl.specifiers
                    && !specifiers.is_empty()
                {
                    new_body.push(import_to_arguments_destructure(allocator, specifiers));
                }
            }
            Statement::ExportDefaultDeclaration(_) => {}
            Statement::ExportNamedDeclaration(named_export) => {
                let named_export = named_export.unbox();
                collect_specifier_exports(&named_export.specifiers, &mut exports);
                if let Some(decl) = named_export.declaration {
                    collect_declaration_exports(&decl, &mut exports);
                    new_body.push(Statement::from(decl));
                }
            }
            other => new_body.push(other),
        }
    }

    new_body.push(create_exports_return(allocator, &exports));
    program.program.body = OxcVec::from_iter_in(new_body, allocator);
}

fn import_to_arguments_destructure<'a>(
    alloc: &'a Allocator,
    specifiers: &[ImportDeclarationSpecifier<'a>],
) -> Statement<'a> {
    use oxc_ast::ast::{ComputedMemberExpression, MemberExpression, ObjectPattern};

    let mut properties = OxcVec::with_capacity_in(specifiers.len(), alloc);
    for spec in specifiers {
        match spec {
            ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => {
                properties.push(BindingProperty {
                    node_id: Cell::new(NodeId::DUMMY),
                    span: SPAN,
                    key: PropertyKey::StaticIdentifier(OxcBox::new_in(
                        create_ident_name(alloc, "default"),
                        alloc,
                    )),
                    value: BindingPattern::BindingIdentifier(OxcBox::new_in(
                        create_binding_ident(alloc, s.local.name.as_str()),
                        alloc,
                    )),
                    shorthand: false,
                    computed: false,
                });
            }
            ImportDeclarationSpecifier::ImportSpecifier(s) => {
                let imported = module_export_name_str(&s.imported);
                let local = s.local.name.as_str();
                properties.push(BindingProperty {
                    node_id: Cell::new(NodeId::DUMMY),
                    span: SPAN,
                    key: PropertyKey::StaticIdentifier(OxcBox::new_in(
                        create_ident_name(alloc, imported),
                        alloc,
                    )),
                    value: BindingPattern::BindingIdentifier(OxcBox::new_in(
                        create_binding_ident(alloc, local),
                        alloc,
                    )),
                    shorthand: imported == local,
                    computed: false,
                });
            }
            ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => {}
        }
    }

    let arguments_0 = Expression::from(MemberExpression::ComputedMemberExpression(OxcBox::new_in(
        ComputedMemberExpression {
            node_id: Cell::new(NodeId::DUMMY),
            object: create_ident_expression(alloc, "arguments"),
            expression: create_num_expression(alloc, 0.0),
            optional: false,
            span: SPAN,
        },
        alloc,
    )));

    let mut decls = OxcVec::with_capacity_in(1, alloc);
    decls.push(VariableDeclarator {
        node_id: Cell::new(NodeId::DUMMY),
        span: SPAN,
        kind: VariableDeclarationKind::Const,
        id: BindingPattern::ObjectPattern(OxcBox::new_in(
            ObjectPattern {
                node_id: Cell::new(NodeId::DUMMY),
                span: SPAN,
                properties,
                rest: None,
            },
            alloc,
        )),
        type_annotation: None,
        init: Some(arguments_0),
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

fn collect_specifier_exports(
    specifiers: &[ExportSpecifier<'_>],
    exports: &mut Vec<(String, String)>,
) {
    for spec in specifiers {
        let exported = module_export_name_str(&spec.exported);
        if exported == "default" {
            continue;
        }
        let local = module_export_name_str_local(&spec.local);
        exports.push((exported.to_string(), local.to_string()));
    }
}

fn collect_declaration_exports(decl: &Declaration<'_>, exports: &mut Vec<(String, String)>) {
    match decl {
        Declaration::VariableDeclaration(var_decl) => {
            for declarator in &var_decl.declarations {
                if let BindingPattern::BindingIdentifier(ident) = &declarator.id {
                    let name = ident.name.to_string();
                    exports.push((name.clone(), name));
                }
            }
        }
        Declaration::FunctionDeclaration(func) => {
            if let Some(id) = &func.id {
                let name = id.name.to_string();
                exports.push((name.clone(), name));
            }
        }
        Declaration::ClassDeclaration(cls) => {
            if let Some(id) = &cls.id {
                let name = id.name.to_string();
                exports.push((name.clone(), name));
            }
        }
        _ => {}
    }
}

fn create_exports_return<'a>(alloc: &'a Allocator, exports: &[(String, String)]) -> Statement<'a> {
    let mut properties = OxcVec::with_capacity_in(exports.len() + 1, alloc);

    for (exported, local) in exports {
        let shorthand = exported == local;
        properties.push(ObjectPropertyKind::ObjectProperty(OxcBox::new_in(
            ObjectProperty {
                node_id: Cell::new(NodeId::DUMMY),
                span: SPAN,
                kind: PropertyKind::Init,
                key: PropertyKey::StaticIdentifier(OxcBox::new_in(
                    create_ident_name(alloc, exported),
                    alloc,
                )),
                value: create_ident_expression(alloc, local),
                shorthand,
                method: false,
                computed: false,
            },
            alloc,
        )));
    }

    properties.push(ObjectPropertyKind::ObjectProperty(OxcBox::new_in(
        ObjectProperty {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            kind: PropertyKind::Init,
            key: PropertyKey::StaticIdentifier(OxcBox::new_in(
                create_ident_name(alloc, "default"),
                alloc,
            )),
            value: create_ident_expression(alloc, "MDXContent"),
            shorthand: false,
            method: false,
            computed: false,
        },
        alloc,
    )));

    Statement::ReturnStatement(OxcBox::new_in(
        ReturnStatement {
            node_id: Cell::new(NodeId::DUMMY),
            span: SPAN,
            argument: Some(create_object_expression(alloc, properties)),
        },
        alloc,
    ))
}

fn module_export_name_str<'a>(name: &'a ModuleExportName<'_>) -> &'a str {
    match name {
        ModuleExportName::IdentifierName(ident) => ident.name.as_str(),
        ModuleExportName::IdentifierReference(ident) => ident.name.as_str(),
        ModuleExportName::StringLiteral(lit) => lit.value.as_str(),
    }
}

fn module_export_name_str_local<'a>(name: &'a ModuleExportName<'_>) -> &'a str {
    match name {
        ModuleExportName::IdentifierReference(ident) => ident.name.as_str(),
        ModuleExportName::IdentifierName(ident) => ident.name.as_str(),
        ModuleExportName::StringLiteral(lit) => lit.value.as_str(),
    }
}

/// Simplify plain MDX JSX elements into regular HAST elements in-place.
///
/// Finds MDX JSX elements (e.g. `<kbd>`, `<abbr>`) that are:
/// - Lowercase name (not a component)
/// - No attributes
/// - Not in the ignore list
///
/// And converts them to `HAST_ELEMENT` nodes so they can be:
/// - Collapsed by `optimizeStatic` into `set:html`
/// - Rendered to HTML by `render_node`
pub fn simplify_plain_mdx_nodes(
    arena: &mut satteri_arena::Arena<satteri_arena::Hast>,
    ignore_elements: &[String],
) {
    use satteri_ast::hast::HastNodeType;
    use satteri_ast::mdast::codec::{decode_mdx_jsx_attr_count, decode_mdx_jsx_element_name};

    let node_count = arena.len();
    for i in 0..node_count {
        let node_id = i as u32;
        let node = arena.get_node(node_id);
        let nt = node.node_type;

        if !matches!(
            HastNodeType::from_u8(nt),
            Some(HastNodeType::MdxJsxElement | HastNodeType::MdxJsxTextElement)
        ) {
            continue;
        }

        let data = arena.get_type_data(node_id);
        if data.len() < 16 {
            continue;
        }

        // Must have a name (not a fragment)
        let name_ref = decode_mdx_jsx_element_name(data);
        if name_ref.is_empty() {
            continue;
        }

        // Must be lowercase (not a component)
        let name = arena.get_str(name_ref);
        if !name.as_bytes().first().is_some_and(u8::is_ascii_lowercase) {
            continue;
        }

        // Must not be in ignore list
        if ignore_elements.iter().any(|s| s == name) {
            continue;
        }

        // Must have no attributes
        let attr_count = decode_mdx_jsx_attr_count(data);
        if attr_count > 0 {
            continue;
        }

        // Rewrite: change node_type to HAST_ELEMENT and rewrite type_data
        // MDX JSX format: [name: StringRef(8B)][attr_count: u32(4B)][_pad: u32(4B)]
        // Element format: [tag_name: StringRef(8B)][prop_count: u32(4B)][_pad: u32(4B)]
        // The layout is the same! name_ref is already at offset 0, and attr_count (0) becomes prop_count (0).
        // We only need to change the node_type byte.
        arena.get_node_mut(node_id).node_type = HastNodeType::Element as u8;
    }
}

/// Wrap the ES AST nodes coming from hast into a whole document.
///
/// ## Errors
///
/// This functions errors for double layouts (default exports).
pub fn mdx_plugin_recma_document<'a>(
    program: &mut MdxProgram<'a>,
    options: &Options,
    location: Option<&Location>,
    allocator: &'a Allocator,
) -> Result<(), message::Message> {
    let document_options = DocumentOptions {
        pragma: options.pragma.clone(),
        pragma_frag: options.pragma_frag.clone(),
        pragma_import_source: options.pragma_import_source.clone(),
        jsx_import_source: options.jsx_import_source.clone(),
        jsx_runtime: options.jsx_runtime,
    };
    recma_document(program, &document_options, location, allocator)
}

/// Rewrite JSX in an MDX file so that components can be passed in and provided.
/// Also compiles JSX to function calls unless `options.jsx` is true.
///
/// ## Errors
///
/// This functions errors for incorrect JSX runtime configuration *inside*
/// MDX files and problems with OXC (broken JS syntax).
#[allow(clippy::implicit_hasher)]
pub fn mdx_plugin_recma_jsx_rewrite<'a>(
    program: &mut MdxProgram<'a>,
    options: &Options,
    location: Option<&Location>,
    explicit_jsxs: &FxHashSet<Span>,
    allocator: &'a Allocator,
) -> Result<(), message::Message> {
    let rewrite_options = RewriteOptions {
        development: options.development,
        provider_import_source: options.provider_import_source.clone(),
    };

    recma_jsx_rewrite(
        program,
        &rewrite_options,
        location,
        explicit_jsxs,
        allocator,
    );

    if !options.jsx {
        let build_options = BuildOptions {
            development: options.development,
        };

        oxc_util_build_jsx(program, &build_options, location, allocator)?;
    }

    Ok(())
}

/// Build a positioned [`message::Message`] from a pulldown-cmark MDX parse
/// error (a `(byte_offset, reason)` pair). The Rust `compile` entry points and
/// the NAPI parse functions share this so a parse error surfaces a source
/// line/column instead of a bare byte offset.
#[must_use]
pub fn parse_error_to_message(source: &str, offset: usize, reason: &str) -> message::Message {
    message::Message {
        place: Some(Box::new(message::Place::Point(byte_offset_to_point(
            source, offset,
        )))),
        reason: reason.to_string(),
        rule_id: Box::new("unexpected-character".into()),
        source: Box::new("mdx-jsx".into()),
    }
}

/// Convert a byte offset in source text to a `Point` (line, column, offset).
fn byte_offset_to_point(value: &str, offset: usize) -> message::Point {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in value.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    message::Point::new(line, col, offset)
}

/// Expand tab characters to spaces for indentation purposes.
///
/// `markdown-rs` and `micromark` handle tabs inside list items differently:
/// micromark treats a tab as continuation whitespace for the list item,
/// while `markdown-rs` can interpret it as a code-indented block boundary.
/// Normalising leading tabs to spaces before parsing avoids this discrepancy.
fn expand_tabs(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for line in value.split('\n') {
        let mut col = 0usize;
        let chars = line.chars().peekable();
        let mut in_indent = true;
        for ch in chars {
            if in_indent && ch == '\t' {
                let spaces = 4 - (col % 4);
                for _ in 0..spaces {
                    out.push(' ');
                }
                col += spaces;
            } else {
                if ch != ' ' {
                    in_indent = false;
                }
                out.push(ch);
                col += 1;
            }
        }
        out.push('\n');
    }
    if !value.ends_with('\n') {
        out.pop();
    }
    out
}
