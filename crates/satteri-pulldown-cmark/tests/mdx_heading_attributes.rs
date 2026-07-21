//! Heading attributes in MDX (issue #162): a trailing `{...}` on an ATX or
//! setext heading is read as attributes when its body isn't valid JS, else
//! stays an expression.
#![cfg(feature = "mdx")]

use satteri_pulldown_cmark::{parse, CowStr, Event, Options, Parser, Tag, MDX_OPTIONS};

fn opts() -> Options {
    MDX_OPTIONS | Options::ENABLE_HEADING_ATTRIBUTES
}

fn html(input: &str) -> String {
    let (arena, errors) = parse(input, opts());
    assert!(
        errors.is_empty(),
        "unexpected errors for {input:?}: {errors:?}"
    );
    satteri_ast::mdast_to_html(&arena)
}

fn errors(input: &str) -> Vec<(usize, String)> {
    parse(input, opts()).1
}

#[test]
fn atx_custom_id() {
    assert_eq!(
        html("# Heading {#custom-id}\n"),
        "<h1 id=\"custom-id\">Heading</h1>\n"
    );
}

#[test]
fn atx_id_class_and_attrs() {
    assert_eq!(
        html("## Note {#intro .lead data-level=2 hidden}\n"),
        "<h2 id=\"intro\" class=\"lead\" data-level=\"2\" hidden=\"\">Note</h2>\n"
    );
}

#[test]
fn atx_expression_stays_expression() {
    let (arena, errs) = parse("# Heading {title}\n", opts());
    assert!(errs.is_empty(), "{errs:?}");
    let rendered = satteri_ast::mdast_to_html(&arena);
    assert!(
        !rendered.contains("id="),
        "should not be an attribute: {rendered}"
    );
}

#[test]
fn atx_expression_then_trailing_id() {
    // Trailing block is the id; the mid-heading `{title}` stays an expression,
    // which the HTML renderer drops.
    assert_eq!(
        html("# Hi {title} {#custom-id}\n"),
        "<h1 id=\"custom-id\">Hi </h1>\n"
    );
}

#[test]
fn atx_invalid_expression_still_errors() {
    assert!(!errors("# Heading {1 +}\n").is_empty());
}

#[test]
fn atx_attributes_reach_events() {
    let ev: Vec<Event<'_>> = Parser::new_ext("# Heading {#custom-id}\n", opts()).collect();
    let id = ev.iter().find_map(|e| match e {
        Event::Start(Tag::Heading { id, .. }) => Some(id.clone()),
        _ => None,
    });
    assert_eq!(id, Some(Some(CowStr::from("custom-id"))));
}

#[test]
fn setext_custom_id() {
    // Setext content is inline-parsed as a paragraph before the underline, so
    // the trailing `{#custom-id}` was validated as an expression and errored;
    // claiming it as attributes must also clear that stale error.
    let (arena, errs) = parse("Heading {#custom-id}\n===\n", opts());
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(
        satteri_ast::mdast_to_html(&arena),
        "<h1 id=\"custom-id\">Heading</h1>\n"
    );
}

#[test]
fn setext_dashes_id_class_and_attrs() {
    let (arena, errs) = parse("Note {#intro .lead data-level=2 hidden}\n---\n", opts());
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(
        satteri_ast::mdast_to_html(&arena),
        "<h2 id=\"intro\" class=\"lead\" data-level=\"2\" hidden=\"\">Note</h2>\n"
    );
}

#[test]
fn setext_expression_stays_expression() {
    // A valid-JS body isn't attribute-shaped, so it stays an expression: no
    // attributes and no error, same as ATX.
    let (arena, errs) = parse("Heading {title}\n===\n", opts());
    assert!(errs.is_empty(), "{errs:?}");
    assert!(
        !satteri_ast::mdast_to_html(&arena).contains("id="),
        "setext should not parse a valid expression as attrs"
    );
}

#[test]
fn setext_invalid_expression_still_errors() {
    // `{1 +}` is not attribute-shaped, so it isn't claimed and its error stands.
    assert!(!errors("Heading {1 +}\n===\n").is_empty());
}

#[test]
fn setext_mid_expression_error_preserved() {
    // The trailing `{#id}` becomes attributes, but a genuine broken expression
    // earlier in the heading keeps its error — only the trailing block's span
    // is cleared.
    let (arena, errs) = parse("Hi {1 +} {#custom-id}\n===\n", opts());
    assert!(!errs.is_empty(), "mid-heading `{{1 +}}` should still error");
    assert!(
        satteri_ast::mdast_to_html(&arena).contains("id=\"custom-id\""),
        "trailing `{{#custom-id}}` should still apply"
    );
}
