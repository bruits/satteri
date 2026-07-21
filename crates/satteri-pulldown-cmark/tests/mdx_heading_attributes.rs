//! Heading attributes in MDX (issue #162): a trailing `{...}` on an ATX heading
//! is read as attributes when its body isn't valid JS, else stays an expression.
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
fn setext_keeps_expression_behavior() {
    // Setext content is parsed before the underline, so `{...}` stays an
    // expression: no attributes, and a valid-JS body doesn't error.
    let (arena, errs) = parse("Heading {title}\n===\n", opts());
    assert!(errs.is_empty(), "{errs:?}");
    let rendered = satteri_ast::mdast_to_html(&arena);
    assert!(
        !rendered.contains("id="),
        "setext should not parse attrs: {rendered}"
    );
}
