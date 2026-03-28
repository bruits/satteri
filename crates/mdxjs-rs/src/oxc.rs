//! Bridge between `markdown-rs` and OXC.

use oxc_allocator::Allocator;
use oxc_ast::ast::{Expression, ObjectPropertyKind, Program, Statement};
use oxc_codegen::Codegen;
use oxc_parser::{ParseOptions, Parser};
use oxc_span::{GetSpan, SPAN, SourceType, Span};
use tryckeri_mdast::mdx_types::{self as message, Location, MdxExpressionKind, Stop};

/// Parse ESM in MDX with OXC.
pub fn parse_esm_to_tree<'a>(
    value: &str,
    stops: &[Stop],
    location: Option<&Location>,
    allocator: &'a Allocator,
) -> Result<Program<'a>, message::Message> {
    let result = parse_esm_core(value, allocator);

    match result {
        Err((span, reason)) => Err(oxc_error_to_message(span, &reason, stops, location)),
        Ok(program) => Ok(program),
    }
}

/// Core to parse ESM.
fn parse_esm_core<'a>(
    value: &str,
    allocator: &'a Allocator,
) -> Result<Program<'a>, (Span, String)> {
    let source_type = SourceType::mjs().with_jsx(true);
    let source = allocator.alloc_str(value);
    let ret = Parser::new(allocator, source, source_type)
        .with_options(ParseOptions::default())
        .parse();

    if !ret.errors.is_empty() {
        let error = &ret.errors[0];
        let span = error
            .labels
            .as_ref()
            .and_then(|labels| {
                labels
                    .first()
                    .map(|l| Span::new(l.offset() as u32, (l.offset() + l.len()) as u32))
            })
            .unwrap_or(SPAN);
        return Err((span, format!("Could not parse esm with oxc: {error}")));
    }

    let program = ret.program;

    // Verify all items are module declarations.
    for node in &program.body {
        if !matches!(
            node,
            Statement::ImportDeclaration(_)
                | Statement::ExportNamedDeclaration(_)
                | Statement::ExportDefaultDeclaration(_)
                | Statement::ExportAllDeclaration(_)
                | Statement::TSExportAssignment(_)
                | Statement::TSNamespaceExportDeclaration(_)
        ) {
            let span = node.span();
            return Err((
                span,
                "Unexpected statement in code: only import/exports are supported".into(),
            ));
        }
    }

    Ok(program)
}

fn parse_expression_core<'a>(
    value: &str,
    kind: &MdxExpressionKind,
    allocator: &'a Allocator,
) -> Result<Option<Expression<'a>>, (Span, String)> {
    // Empty expressions are OK.
    if matches!(kind, MdxExpressionKind::Expression) && whitespace_and_comments(0, value).is_ok() {
        return Ok(None);
    }

    let source_type = SourceType::mjs().with_jsx(true);

    if !matches!(kind, MdxExpressionKind::AttributeExpression) {
        // Use OXC's expression parser so that `{...}` is parsed as an object
        // literal (not a block statement) and string literals like `"hello"`
        // are not misclassified as directives.
        let source = allocator.alloc_str(value);
        let expr = Parser::new(allocator, source, source_type)
            .with_options(ParseOptions::default())
            .parse_expression()
            .map_err(|errors| {
                let error = &errors[0];
                let span = error
                    .labels
                    .as_ref()
                    .and_then(|labels| {
                        labels
                            .first()
                            .map(|l| Span::new(l.offset() as u32, (l.offset() + l.len()) as u32))
                    })
                    .unwrap_or(SPAN);
                (
                    span,
                    format!("Could not parse expression with oxc: {error}"),
                )
            })?;

        // Check the expression ends at the right place.
        let expression_end = expr.span().end as usize;
        if let Err((span, reason)) = whitespace_and_comments(expression_end, value) {
            return Err((span, reason));
        }

        return Ok(Some(expr));
    }

    // For attribute expressions, a spread is needed, for which we have to
    // prefix and suffix the input.
    let prefix = "({";
    let suffix = "})";

    let full_value = format!("{prefix}{value}{suffix}");
    let source = allocator.alloc_str(&full_value);
    let ret = Parser::new(allocator, source, source_type)
        .with_options(ParseOptions::default())
        .parse();

    if !ret.errors.is_empty() {
        let error = &ret.errors[0];
        let span = error
            .labels
            .as_ref()
            .and_then(|labels| {
                labels.first().map(|l| {
                    let start = (l.offset() as u32).saturating_sub(prefix.len() as u32);
                    let end = ((l.offset() + l.len()) as u32).saturating_sub(prefix.len() as u32);
                    Span::new(start, end)
                })
            })
            .unwrap_or(SPAN);
        return Err((
            span,
            format!("Could not parse expression with oxc: {error}"),
        ));
    }

    let program = ret.program;

    let expr = if let Some(first) = program.body.into_iter().next() {
        match first {
            Statement::ExpressionStatement(stmt) => {
                let stmt = stmt.unbox();
                stmt.expression
            }
            _ => {
                return Err((
                    first.span(),
                    "Could not parse expression with oxc: Expected an expression".into(),
                ));
            }
        }
    } else {
        return Err((
            SPAN,
            "Could not parse expression with oxc: Unexpected empty expression".into(),
        ));
    };

    // Check the expression ends at the right place.
    let expression_end = expr.span().end as usize;
    let adj_end = expression_end.saturating_sub(prefix.len());
    if let Err((span, reason)) = whitespace_and_comments(adj_end, value) {
        return Err((span, reason));
    }

    // AttributeExpression handling:
    if matches!(kind, MdxExpressionKind::AttributeExpression) {
        let expr_span = expr.span();

        if let Expression::ParenthesizedExpression(paren) = expr {
            let paren = paren.unbox();
            if let Expression::ObjectExpression(mut obj) = paren.expression {
                let obj = &mut *obj;
                if obj.properties.len() > 1 {
                    return Err((obj.span, "Unexpected extra content in spread (such as `{...x,y}`): only a single spread is supported (such as `{...x}`)".into()));
                }

                if let Some(ObjectPropertyKind::SpreadProperty(spread)) = obj.properties.pop() {
                    let spread = spread.unbox();
                    return Ok(Some(spread.argument));
                }
            }
        }

        return Err((
            expr_span,
            "Unexpected prop in spread (such as `{x}`): only a spread is supported (such as `{...x}`)".into(),
        ));
    }

    Ok(Some(expr))
}

/// Parse expression in MDX with OXC.
pub fn parse_expression_to_tree<'a>(
    value: &str,
    kind: &MdxExpressionKind,
    stops: &[Stop],
    location: Option<&Location>,
    allocator: &'a Allocator,
) -> Result<Option<Expression<'a>>, message::Message> {
    let result = parse_expression_core(value, kind, allocator);

    match result {
        Err((span, reason)) => Err(oxc_error_to_message(span, &reason, stops, location)),
        Ok(expr_opt) => Ok(expr_opt),
    }
}

/// Serialize an OXC program.
pub fn serialize(program: &Program<'_>) -> String {
    let codegen = Codegen::new().with_options(oxc_codegen::CodegenOptions {
        indent_char: oxc_codegen::IndentChar::Space,
        indent_width: 4,
        ..oxc_codegen::CodegenOptions::default()
    });
    let output = codegen.build(program);
    output.code
}

/// Turn an OXC error into a markdown message, resolving the position via stops.
fn oxc_error_to_message(
    span: Span,
    reason: &str,
    stops: &[Stop],
    location: Option<&Location>,
) -> message::Message {
    let point =
        location.and_then(|location| location.relative_to_point(stops, span.start as usize));

    message::Message {
        reason: reason.into(),
        place: point.map(|point| Box::new(message::Place::Point(point))),
        source: Box::new("mdxjs-rs".into()),
        rule_id: Box::new("oxc".into()),
    }
}

/// Move past JavaScript whitespace and comments.
fn whitespace_and_comments(mut index: usize, value: &str) -> Result<(), (Span, String)> {
    let bytes = value.as_bytes();
    let len = bytes.len();
    let mut in_multiline = false;
    let mut in_line = false;

    while index < len {
        if in_multiline {
            if index + 1 < len && bytes[index] == b'*' && bytes[index + 1] == b'/' {
                index += 1;
                in_multiline = false;
            }
        } else if in_line {
            if bytes[index] == b'\r' || bytes[index] == b'\n' {
                in_line = false;
            }
        } else if index + 1 < len && bytes[index] == b'/' && bytes[index + 1] == b'*' {
            index += 1;
            in_multiline = true;
        } else if index + 1 < len && bytes[index] == b'/' && bytes[index + 1] == b'/' {
            index += 1;
            in_line = true;
        } else if bytes[index].is_ascii_whitespace() {
            // Fine!
        } else {
            return Err((
                Span::new(index as u32, value.len() as u32),
                "Could not parse expression with oxc: Unexpected content after expression".into(),
            ));
        }

        index += 1;
    }

    if in_multiline {
        return Err((
            Span::new(index as u32, value.len() as u32),
            "Could not parse expression with oxc: Unexpected unclosed multiline comment, expected closing: `*/`".into(),
        ));
    }

    if in_line {
        return Err((
            Span::new(index as u32, value.len() as u32),
            "Could not parse expression with oxc: Unexpected unclosed line comment, expected line ending: `\\n`".into(),
        ));
    }

    Ok(())
}
