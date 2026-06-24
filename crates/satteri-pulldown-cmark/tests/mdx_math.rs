//! MDX + math interaction at the MDAST level.
#![cfg(feature = "mdx")]

use satteri_arena::{Arena, Mdast};
use satteri_ast::mdast::{decode_math_data, MdastNodeType};
use satteri_pulldown_cmark::arena_build::parse;
use satteri_pulldown_cmark::Options;

fn mdx_math_options() -> Options {
    Options::ENABLE_MDX | Options::ENABLE_MATH
}

fn find_inline_math_value(arena: &Arena<Mdast>) -> Option<String> {
    (0..arena.len() as u32)
        .find(|&id| arena.get_node(id).node_type == MdastNodeType::InlineMath as u8)
        .map(|id| {
            let value = decode_math_data(arena.get_type_data(id)).value;
            arena.get_str(value).to_string()
        })
}

// Braces inside inline math `$...$` are math text, not MDX expressions, the
// same way block `$$...$$` already behaves (#110). Without the math-span guard
// in the inline `{` handler, LaTeX like `\frac{-b}{2a}` is handed to oxc and
// errors with "Invalid characters after number".
#[test]
fn inline_math_with_braces_is_not_parsed_as_expression() {
    for src in ["$\\frac{-b}{2a}$", "$x{2b}y$", "$x{a-}y$", "$x{a b}y$"] {
        let (_arena, errors) = parse(src, mdx_math_options());
        assert!(errors.is_empty(), "{src:?} produced MDX errors: {errors:?}");
    }
}

#[test]
fn inline_math_preserves_brace_content() {
    let (arena, errors) = parse("$\\frac{-b}{2a}$", mdx_math_options());
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_eq!(
        find_inline_math_value(&arena).as_deref(),
        Some("\\frac{-b}{2a}"),
    );
}

// MDX expressions outside math are still validated (the guard must not disable
// expression scanning wholesale when math is on).
#[test]
fn mdx_expression_outside_math_still_validated() {
    let (_arena, errors) = parse("text {1 +} more", mdx_math_options());
    assert!(
        !errors.is_empty(),
        "malformed expression should still error"
    );
}

// A `\$` only prevents *opening* a math span, not closing one, matching the
// resolver. So `$...\$` is a span that closes at the escaped `$`, and a `{`
// between the dollars is math text. Without the guard tracking this, the `{`
// is handed to oxc as an expression and errors on the unterminated brace.
// A `$$` display-math fence on its own line is a block boundary, so an inline
// `$$` earlier in the paragraph must not pair with it and swallow a `{` as math
// text. Here `{1}` is a real text expression (matching mdx-js), not math.
#[test]
fn inline_dollars_do_not_pair_across_display_fence() {
    let (arena, errors) = parse("a$$\nx{1}y\n$$", mdx_math_options());
    assert!(errors.is_empty(), "errors: {errors:?}");
    let has_expr = (0..arena.len() as u32)
        .any(|id| arena.get_node(id).node_type == MdastNodeType::MdxTextExpression as u8);
    assert!(
        has_expr,
        "`{{1}}` after an inline `$$` must be a text expression, not math text"
    );
}

#[test]
fn brace_in_span_closing_at_escaped_dollar_is_math_text() {
    // In each case the only possible closer is the escaped `$`; dropping it
    // (the old guard's bug) leaves the `{` outside any span and oxc errors on
    // the invalid/unterminated expression body.
    for src in ["e$}}_{\\$h", "$x{1 +}\\$", "a $b{1 +}c\\$"] {
        let (_arena, errors) = parse(src, mdx_math_options());
        assert!(errors.is_empty(), "{src:?} produced MDX errors: {errors:?}");
    }
}
