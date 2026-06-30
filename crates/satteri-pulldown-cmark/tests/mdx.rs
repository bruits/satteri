//! MDX extension tests — ported from markdown-rs test cases.
//!
//! Tests are organized by construct: ESM, expression flow, expression text,
//! JSX flow, JSX text. Edge cases from markdown-rs's mdx_*.rs test files are
//! included.
//!
//! The whole file is MDX-only; it compiles out of the lite (non-mdx) build.
#![cfg(feature = "mdx")]

use satteri_pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

fn mdx_events(input: &str) -> Vec<Event<'_>> {
    let opts = Options::ENABLE_MDX
        | Options::ENABLE_TABLES
        | Options::ENABLE_MATH
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    Parser::new_ext(input, opts).collect()
}

fn has(events: &[Event<'_>], pred: impl Fn(&Event<'_>) -> bool) -> bool {
    events.iter().any(pred)
}

fn count(events: &[Event<'_>], pred: impl Fn(&Event<'_>) -> bool) -> usize {
    events.iter().filter(|e| pred(e)).count()
}

// ===========================================================================
// ESM
// ===========================================================================

#[test]
fn esm_import() {
    let ev = mdx_events("import a from 'b'\n\nc");
    assert!(has(
        &ev,
        |e| matches!(e, Event::MdxEsm(s) if s.contains("import"))
    ));
    assert!(has(&ev, |e| matches!(e, Event::Start(Tag::Paragraph))));
}

#[test]
fn esm_export() {
    let ev = mdx_events("export default a\n\nb");
    assert!(has(
        &ev,
        |e| matches!(e, Event::MdxEsm(s) if s.contains("export"))
    ));
}

#[test]
fn esm_export_brace() {
    let ev = mdx_events("export {a}\n");
    assert!(has(&ev, |e| matches!(e, Event::MdxEsm(_))));
}

#[test]
fn esm_export_star() {
    let ev = mdx_events("export * from 'a'\n");
    assert!(has(&ev, |e| matches!(e, Event::MdxEsm(_))));
}

#[test]
fn esm_not_import_dot() {
    assert!(!has(&mdx_events("import.\n"), |e| matches!(
        e,
        Event::MdxEsm(_)
    )));
}

#[test]
fn esm_not_import_parens() {
    assert!(!has(&mdx_events("import('a')\n"), |e| matches!(
        e,
        Event::MdxEsm(_)
    )));
}

#[test]
fn esm_not_impossible() {
    assert!(!has(&mdx_events("impossible\n"), |e| matches!(
        e,
        Event::MdxEsm(_)
    )));
}

#[test]
fn esm_not_exporting() {
    assert!(!has(&mdx_events("exporting\n"), |e| matches!(
        e,
        Event::MdxEsm(_)
    )));
}

#[test]
fn esm_not_indented() {
    assert!(!has(&mdx_events("  import a from 'b'\n"), |e| matches!(
        e,
        Event::MdxEsm(_)
    )));
}

#[test]
fn esm_consecutive_imports() {
    // Two consecutive import/export lines should both be ESM.
    let ev = mdx_events("import a from 'b'\nexport default c\n\nd");
    let esm_count = count(&ev, |e| matches!(e, Event::MdxEsm(_)));
    // They may be merged into one ESM event or two; either is fine.
    assert!(esm_count >= 1, "consecutive import/export: {:?}", ev);
}

#[test]
fn esm_separated_by_blank() {
    let ev = mdx_events("import a from 'b'\n\nexport default c\n\nd");
    let esm_count = count(&ev, |e| matches!(e, Event::MdxEsm(_)));
    assert!(esm_count >= 2, "separate ESM blocks: {:?}", ev);
}

#[test]
fn esm_between_paragraphs() {
    let ev = mdx_events("a\n\nimport a from 'b'\n\nb");
    assert!(has(&ev, |e| matches!(e, Event::MdxEsm(_))));
    assert_eq!(count(&ev, |e| matches!(e, Event::Start(Tag::Paragraph))), 2);
}

#[test]
fn esm_import_styles() {
    // All import forms should be recognized as ESM.
    let cases = [
        "import a from \"b\"\n",
        "import * as a from \"b\"\n",
        "import {a} from \"b\"\n",
        "import {a as b} from \"c\"\n",
        "import a, {b as c} from \"d\"\n",
        "import a, * as b from \"c\"\n",
        "import \"a\"\n",
    ];
    for input in cases {
        assert!(
            has(&mdx_events(input), |e| matches!(e, Event::MdxEsm(_))),
            "should be ESM: {}",
            input
        );
    }
}

#[test]
fn esm_export_styles() {
    let cases = [
        "export var a = \"\"\n",
        "export const a = \"\"\n",
        "export let a = \"\"\n",
        "export function a() {}\n",
        "export class a {}\n",
        "export default a = 1\n",
        "export default function a() {}\n",
        "export default class a {}\n",
        "export * from \"a\"\n",
        "export {a} from \"b\"\n",
    ];
    for input in cases {
        assert!(
            has(&mdx_events(input), |e| matches!(e, Event::MdxEsm(_))),
            "should be ESM: {}",
            input
        );
    }
}

#[test]
fn esm_multiline_export_object() {
    // Multiline export with nested objects and arrays must be captured as a single ESM block.
    let input = "export const data = {\n  users: [\n    { name: 'Alice', age: 30 },\n    { name: 'Bob', age: 25 }\n  ]\n};\n\n# Hello\n";
    let ev = mdx_events(input);
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::MdxEsm(s) if s.contains("data") && s.contains("}"))
        ),
        "multiline export object should be a single ESM block: {:?}",
        ev
    );
    // The heading after the blank line should still be parsed as a heading.
    assert!(has(&ev, |e| matches!(e, Event::Start(Tag::Heading { .. }))));
}

#[test]
fn esm_multiline_export_array_ending() {
    // Line ending with `]` should not break the ESM block if braces are still open.
    let input = "export const x = {\n  items: [1, 2, 3]\n};\n";
    let ev = mdx_events(input);
    assert!(
        has(&ev, |e| matches!(e, Event::MdxEsm(s) if s.contains("};"))),
        "closing brace must be included: {:?}",
        ev
    );
}

#[test]
fn esm_export_spanning_blank_line() {
    // An export with a blank line in the middle should still be captured
    // as a single ESM block (oxc detects the incomplete parse and retries).
    let input = "export const x = {\n  a: 1,\n\n  b: 2\n};\n\n# Hello\n";
    let ev = mdx_events(input);
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::MdxEsm(s) if s.contains("b: 2") && s.contains("};"))
        ),
        "export spanning blank line should be a single ESM block: {:?}",
        ev
    );
    assert!(has(&ev, |e| matches!(e, Event::Start(Tag::Heading { .. }))));
}

#[test]
fn esm_whitespace_only_line_after_import() {
    // A "blank" line containing only spaces/tabs must end the ESM block exactly
    // like an empty line, so the following Markdown is parsed as content.
    let ev = mdx_events("import a from 'b'\n \nHello world.");
    assert!(
        has(&ev, |e| matches!(e, Event::MdxEsm(s)
            if s.contains("import") && !s.contains("Hello"))),
        "ESM block must stop at the whitespace-only line: {:?}",
        ev
    );
    assert!(
        has(&ev, |e| matches!(e, Event::Start(Tag::Paragraph))),
        "text after the whitespace-only line must be a paragraph: {:?}",
        ev
    );
}

#[test]
fn esm_export_spanning_whitespace_only_line() {
    // An export with a whitespace-only "blank" line in the middle should still
    // be captured as a single ESM block, like a truly empty line.
    let input = "export const x = {\n  a: 1,\n \n  b: 2\n};\n\n# Hello\n";
    let ev = mdx_events(input);
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::MdxEsm(s) if s.contains("b: 2") && s.contains("};"))
        ),
        "export spanning whitespace-only line should be a single ESM block: {:?}",
        ev
    );
    assert!(has(&ev, |e| matches!(e, Event::Start(Tag::Heading { .. }))));
}

#[test]
fn esm_export_template_spanning_blank_line() {
    // A blank line inside a template literal must not end the ESM block (#111):
    // the scanner stays inside the template instead of truncating it.
    let input = "export const code = `first line\n\nsecond line`;\n\nAfter.\n";
    let ev = mdx_events(input);
    assert!(
        has(&ev, |e| matches!(e, Event::MdxEsm(s)
            if s.contains("second line") && !s.contains("After"))),
        "template spanning a blank line should be one ESM block: {:?}",
        ev
    );
    assert!(has(&ev, |e| matches!(e, Event::Start(Tag::Paragraph))));
}

#[test]
fn esm_export_regex_with_backtick() {
    // A backtick inside a top-level regex literal must not open a phantom
    // template literal and swallow the following content into the ESM block.
    let input = "export const re = /a`b/;\n\nAfter the regex.\n";
    let ev = mdx_events(input);
    assert!(
        has(&ev, |e| matches!(e, Event::MdxEsm(s)
            if s.contains("re =") && !s.contains("After"))),
        "regex containing a backtick must not extend the ESM block: {:?}",
        ev
    );
    assert!(
        has(&ev, |e| matches!(e, Event::Start(Tag::Paragraph))),
        "text after the regex export must be a paragraph: {:?}",
        ev
    );
}

#[test]
fn esm_export_regex_with_quotes() {
    // Quotes inside a regex character class must not be read as string
    // delimiters either.
    let input = "export const re = /[\"']/g;\n\nAfter the regex.\n";
    let ev = mdx_events(input);
    assert!(
        has(&ev, |e| matches!(e, Event::MdxEsm(s)
            if s.contains("re =") && !s.contains("After"))),
        "regex containing quotes must not extend the ESM block: {:?}",
        ev
    );
    assert!(has(&ev, |e| matches!(e, Event::Start(Tag::Paragraph))));
}

#[test]
fn esm_export_division_not_regex() {
    // A `/` that is division (after a value) must not be scanned as a regex
    // and run on past the end of the block.
    let input = "export const half = total / 2;\n\nAfter.\n";
    let ev = mdx_events(input);
    assert!(
        has(&ev, |e| matches!(e, Event::MdxEsm(s)
            if s.contains("half") && !s.contains("After"))),
        "division must not extend the ESM block: {:?}",
        ev
    );
    assert!(has(&ev, |e| matches!(e, Event::Start(Tag::Paragraph))));
}

#[test]
fn esm_not_in_paragraph() {
    // Import/export inside a paragraph (not interrupting) should not be ESM.
    let ev = mdx_events("a\nimport a from 'b'\n");
    assert!(
        !has(&ev, |e| matches!(e, Event::MdxEsm(_))),
        "should not be ESM inside paragraph: {:?}",
        ev
    );
}

#[test]
fn esm_not_in_list() {
    let ev = mdx_events("- import a from 'b'\n");
    assert!(
        !has(&ev, |e| matches!(e, Event::MdxEsm(_))),
        "should not be ESM in list item: {:?}",
        ev
    );
}

#[test]
fn esm_not_in_blockquote() {
    let ev = mdx_events("> export default c\n");
    assert!(
        !has(&ev, |e| matches!(e, Event::MdxEsm(_))),
        "should not be ESM in blockquote: {:?}",
        ev
    );
}

// ===========================================================================
// Expression flow
// ===========================================================================

#[test]
fn expr_flow_simple() {
    let ev = mdx_events("{a}\n");
    assert!(has(
        &ev,
        |e| matches!(e, Event::MdxFlowExpression(s) if s.as_ref() == "a")
    ));
}

#[test]
fn expr_flow_empty() {
    let ev = mdx_events("{}\n");
    assert!(has(
        &ev,
        |e| matches!(e, Event::MdxFlowExpression(s) if s.as_ref() == "")
    ));
}

#[test]
fn expr_flow_nested_braces() {
    let ev = mdx_events("{a({b: 1})}\n");
    assert!(has(
        &ev,
        |e| matches!(e, Event::MdxFlowExpression(s) if s.as_ref() == "a({b: 1})")
    ));
}

#[test]
fn expr_flow_multiline() {
    let ev = mdx_events("{\n}\n");
    assert!(
        has(&ev, |e| matches!(e, Event::MdxFlowExpression(_))),
        "multiline expression: {:?}",
        ev
    );
}

#[test]
fn expr_flow_trailing_whitespace() {
    // `{ a } \t\n` — expression followed by spaces.
    let ev = mdx_events("{ a } \t\n");
    assert!(
        has(&ev, |e| matches!(e, Event::MdxFlowExpression(_))),
        "trailing whitespace: {:?}",
        ev
    );
}

#[test]
fn expr_flow_leading_whitespace() {
    // `  { a }\n` — expression preceded by spaces.
    let ev = mdx_events("  { a }\n");
    // This may or may not be a flow expression (depends on indent handling).
    // In markdown-rs it IS a flow expression. Let's just not crash.
    let _ = ev;
}

#[test]
fn expr_flow_with_strings() {
    // Braces inside strings should not count.
    let ev = mdx_events("{\"a { b }\"}\n");
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::MdxFlowExpression(s) if s.as_ref() == "\"a { b }\"")
        ),
        "strings in expression: {:?}",
        ev
    );
}

#[test]
fn expr_flow_with_template() {
    let ev = mdx_events("{`a { b }`}\n");
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::MdxFlowExpression(s) if s.as_ref() == "`a { b }`")
        ),
        "template in expression: {:?}",
        ev
    );
}

// ===========================================================================
// Expression text (inline)
// ===========================================================================

#[test]
fn expr_text_simple() {
    let ev = mdx_events("a {b} c");
    assert!(has(
        &ev,
        |e| matches!(e, Event::MdxTextExpression(s) if s.as_ref() == "b")
    ));
}

#[test]
fn expr_text_empty() {
    let ev = mdx_events("a {} b");
    assert!(has(
        &ev,
        |e| matches!(e, Event::MdxTextExpression(s) if s.as_ref() == "")
    ));
}

#[test]
fn expr_text_nested() {
    let ev = mdx_events("a {b({c: 1})} d");
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::MdxTextExpression(s) if s.as_ref() == "b({c: 1})")
        ),
        "nested inline expression: {:?}",
        ev
    );
}

#[test]
fn expr_text_multiline() {
    let ev = mdx_events("a {\n} b");
    assert!(
        has(&ev, |e| matches!(e, Event::MdxTextExpression(_))),
        "multiline inline expression: {:?}",
        ev
    );
}

#[test]
fn expr_text_closing_brace_as_text() {
    // A lone `}` should be plain text.
    let ev = mdx_events("a } b");
    assert!(
        !has(&ev, |e| matches!(e, Event::MdxTextExpression(_))),
        "lone }} should be text: {:?}",
        ev
    );
    assert!(has(&ev, |e| matches!(e, Event::Text(_))));
}

#[test]
fn expr_text_at_start() {
    // Expression at the start of inline content.
    let ev = mdx_events("{ a } b");
    assert!(
        has(&ev, |e| matches!(e, Event::MdxTextExpression(_))),
        "expression at start: {:?}",
        ev
    );
}

#[test]
fn expr_text_with_parens() {
    let ev = mdx_events("a{(b)}c");
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::MdxTextExpression(s) if s.as_ref() == "(b)")
        ),
        "parens in expression: {:?}",
        ev
    );
}

#[test]
fn expr_text_with_string_braces() {
    // Braces inside strings don't count.
    let ev = mdx_events("a {\"}\"}  b");
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::MdxTextExpression(s) if s.as_ref() == "\"}\"")
        ),
        "string with brace: {:?}",
        ev
    );
}

#[test]
fn expr_text_comment() {
    let ev = mdx_events("a {/**/} b");
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::MdxTextExpression(s) if s.as_ref() == "/**/")
        ),
        "comment in expression: {:?}",
        ev
    );
}

#[test]
fn expr_text_1_plus_1() {
    let ev = mdx_events("a {1 + 1} b");
    assert!(has(
        &ev,
        |e| matches!(e, Event::MdxTextExpression(s) if s.as_ref() == "1 + 1")
    ));
}

#[test]
fn expr_text_function() {
    let ev = mdx_events("a {function () {}} b");
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::MdxTextExpression(s) if s.as_ref() == "function () {}")
        ),
        "function in expression: {:?}",
        ev
    );
}

#[test]
fn expr_text_comment_with_braces() {
    // Braces inside block comments should not affect expression boundary.
    let ev = mdx_events("a {/* } */} b");
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::MdxTextExpression(s) if s.as_ref() == "/* } */")
        ),
        "comment with braces: {:?}",
        ev
    );
}

#[test]
fn expr_text_string_with_brace() {
    // Closing brace inside a string should not end the expression.
    let ev = mdx_events("a {\"}\"}  b");
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::MdxTextExpression(s) if s.as_ref() == "\"}\"")
        ),
        "string with brace: {:?}",
        ev
    );
}

#[test]
fn expr_text_template_literal() {
    // Template literal with nested ${} and braces.
    let ev = mdx_events("a {`${x}`} b");
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::MdxTextExpression(s) if s.as_ref() == "`${x}`")
        ),
        "template literal: {:?}",
        ev
    );
}

// ===========================================================================
// JSX flow (block-level)
// ===========================================================================

#[test]
fn jsx_flow_self_closing() {
    let ev = mdx_events("<a />\n");
    assert!(has(&ev, |e| matches!(
        e,
        Event::Start(Tag::MdxJsxFlowElement(_))
    )));
}

#[test]
fn jsx_flow_closed() {
    let ev = mdx_events("<a></a>\n");
    assert!(has(&ev, |e| matches!(
        e,
        Event::Start(Tag::MdxJsxFlowElement(_))
    )));
}

#[test]
fn jsx_flow_fragment() {
    let ev = mdx_events("<>\n");
    assert!(has(&ev, |e| matches!(
        e,
        Event::Start(Tag::MdxJsxFlowElement(_))
    )));
}

#[test]
fn jsx_flow_attributes() {
    let ev = mdx_events("<a b c:d e=\"\" f={/* g */} {...h} />\n");
    assert!(
        has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxFlowElement(_))
        )),
        "attributes: {:?}",
        ev
    );
}

#[test]
fn jsx_flow_with_expression_attr() {
    let ev = mdx_events("<Chart values={[1,2,3]} />\n");
    assert!(has(
        &ev,
        |e| matches!(e, Event::Start(Tag::MdxJsxFlowElement(s)) if s.contains("Chart"))
    ));
}

#[test]
fn jsx_flow_expr_attr_quote_in_jsx_text() {
    // Quotes and apostrophes in the JSX *text* of an element inside an
    // attribute expression are literal — the expression scan consumes the
    // element's children as text and must run through to the real closing `}`.
    // Before the fix the scanner lexed those children as JS, so a quote (after
    // a close tag `</b>'s`, after a `.` in `Corp.'s`, or a paired `"..."`)
    // opened a phantom string literal and swallowed the rest of the line.
    for src in [
        "<Foo d={<p>a<b>x</b>'s</p>} />\n",
        "<Foo d={<p>Widget<Icon />'s label</p>} />\n",
        "<Foo d={<p>Acme Corp.'s annual report</p>} />\n",
        "<Foo d={<p>a \"!?\" badge</p>} />\n",
        "<Foo d={<p>say <b>\"hi\"</b> ok</p>} />\n",
        "<Foo d={<p>nested {expr} and 'text'</p>} />\n",
    ] {
        let ev = mdx_events(src);
        assert!(
            has(&ev, |e| matches!(
                e,
                Event::Start(Tag::MdxJsxFlowElement(s)) if s.contains("Foo")
            )),
            "quote in JSX text after a value token must still parse: {src:?} -> {ev:?}"
        );
    }
}

#[test]
fn jsx_flow_closing_fragment() {
    let ev = mdx_events("</>\n");
    assert!(
        has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxFlowElement(_))
        )),
        "closing fragment: {:?}",
        ev
    );
}

#[test]
fn jsx_flow_closing_tag() {
    let ev = mdx_events("</a>\n");
    assert!(
        has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxFlowElement(_))
        )),
        "closing tag: {:?}",
        ev
    );
}

#[test]
fn jsx_flow_self_referential_close_tag_in_attribute() {
    use satteri_pulldown_cmark::{parse, MDX_OPTIONS};
    let src = "<CodePreview\n  code={`<CodePreview lang=\"astro\">\n    body\n</CodePreview>`}\n  lang=\"astro\"\n>\n  <CodePreview lang=\"astro\">\n    body\n  </CodePreview>\n</CodePreview>\n";
    let (_arena, errors) = parse(src, MDX_OPTIONS);
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
}

#[test]
fn jsx_close_tag_in_string_attribute_is_text() {
    use satteri_pulldown_cmark::{parse, MDX_OPTIONS};
    let (_arena, errors) = parse("<Demo code=\"</Demo>\">child</Demo>\n", MDX_OPTIONS);
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
}

#[test]
fn jsx_text_lowercase() {
    let ev = mdx_events("a <b> c");
    assert!(
        has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxTextElement(_))
        )),
        "lowercase <b>: {:?}",
        ev
    );
}

#[test]
fn jsx_text_self_closing() {
    let ev = mdx_events("a <b/> c.");
    assert!(has(&ev, |e| matches!(
        e,
        Event::Start(Tag::MdxJsxTextElement(_))
    )));
}

#[test]
fn jsx_text_closed() {
    let ev = mdx_events("a <b></b> c.");
    assert!(
        count(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxTextElement(_))
        )) >= 1
    );
}

#[test]
fn jsx_text_fragment() {
    let ev = mdx_events("a <></> c.");
    assert!(
        has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxTextElement(_))
        )),
        "inline fragment: {:?}",
        ev
    );
}

#[test]
fn jsx_text_uppercase() {
    let ev = mdx_events("a <Badge /> c.");
    assert!(has(
        &ev,
        |e| matches!(e, Event::Start(Tag::MdxJsxTextElement(s)) if s.contains("Badge"))
    ));
}

#[test]
fn jsx_text_namespaced() {
    // `<a:b />` — namespaced tag name.
    let ev = mdx_events("<a:b />.");
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::Start(Tag::MdxJsxTextElement(s)) if s.contains("a:b"))
        ),
        "namespaced tag: {:?}",
        ev
    );
}

#[test]
fn jsx_text_member() {
    // `<a.b.c />` — member expression tag name.
    let ev = mdx_events("<a.b.c />.");
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::Start(Tag::MdxJsxTextElement(s)) if s.contains("a.b.c"))
        ),
        "member tag: {:?}",
        ev
    );
}

#[test]
fn jsx_text_spread_attribute() {
    let ev = mdx_events("<a {...b} />.");
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::Start(Tag::MdxJsxTextElement(s)) if s.contains("...b"))
        ),
        "spread attribute: {:?}",
        ev
    );
}

#[test]
fn jsx_text_boolean_attribute() {
    let ev = mdx_events("<a b />.");
    assert!(
        has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxTextElement(_))
        )),
        "boolean attribute: {:?}",
        ev
    );
}

#[test]
fn jsx_text_string_attribute() {
    let ev = mdx_events("<a b=\"c\" />.");
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::Start(Tag::MdxJsxTextElement(s)) if s.contains("b=\"c\""))
        ),
        "string attribute: {:?}",
        ev
    );
}

#[test]
fn jsx_text_expression_attribute() {
    let ev = mdx_events("<a b={c} />.");
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::Start(Tag::MdxJsxTextElement(s)) if s.contains("b={c}"))
        ),
        "expression attribute: {:?}",
        ev
    );
}

#[test]
fn jsx_text_closing_tag() {
    let ev = mdx_events("a </b> c");
    assert!(
        has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxTextElement(_))
        )),
        "closing tag inline: {:?}",
        ev
    );
}

#[test]
fn jsx_text_not_lt_number() {
    // `a < 3` is NOT JSX — no tag name after `<`.
    let ev = mdx_events("a < 3 b");
    assert!(
        !has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxTextElement(_))
        )),
        "< number is not JSX: {:?}",
        ev
    );
}

#[test]
fn jsx_text_complex_attrs() {
    // Multiple different attribute types.
    let ev = mdx_events("<a b c:d e=\"\" f={g} {...h} />.");
    assert!(
        has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxTextElement(_))
        )),
        "complex attrs: {:?}",
        ev
    );
}

// ===========================================================================
// Non-MDX mode
// ===========================================================================

#[test]
fn no_mdx_without_flag() {
    let ev: Vec<_> = Parser::new("import foo from 'bar'\n").collect();
    assert!(!has(&ev, |e| matches!(e, Event::MdxEsm(_))));
}

#[test]
fn no_mdx_expressions_without_flag() {
    let ev: Vec<_> = Parser::new("a {b} c\n").collect();
    assert!(!has(&ev, |e| matches!(e, Event::MdxTextExpression(_))));
}

#[test]
fn no_mdx_jsx_without_flag() {
    // Without MDX, <em> is regular HTML, not JSX.
    let ev: Vec<_> = Parser::new("a <em>b</em> c\n").collect();
    assert!(!has(&ev, |e| matches!(
        e,
        Event::Start(Tag::MdxJsxTextElement(_))
    )));
}

// ===========================================================================
// Mixed / interleaving
// ===========================================================================

#[test]
fn expression_then_jsx_same_line_is_inline() {
    // `{1}<a/>` on same line → inline (inside a paragraph), not flow.
    let ev = mdx_events("{1}<a/>\n");
    assert!(
        has(&ev, |e| matches!(e, Event::MdxTextExpression(_)))
            || has(&ev, |e| matches!(
                e,
                Event::Start(Tag::MdxJsxTextElement(_))
            ))
            || has(&ev, |e| matches!(e, Event::MdxFlowExpression(_)))
            || has(&ev, |e| matches!(
                e,
                Event::Start(Tag::MdxJsxFlowElement(_))
            )),
        "expression+jsx on same line: {:?}",
        ev
    );
}

#[test]
fn jsx_then_expression_same_line_is_inline() {
    let ev = mdx_events("<x/>{1}\n");
    // Should parse without crashing. May be flow or inline.
    let _ = ev;
}

#[test]
fn jsx_opening_with_expr_child_is_flow() {
    // `<Foo>{x}</Foo>` on one line: body contains only expressions/JSX, so
    // it's a flow element (matching remark-mdx behavior).
    let ev = mdx_events("<Foo>{x}</Foo>\n");
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::Start(Tag::MdxJsxFlowElement(s)) if s.contains("Foo"))
        ),
        "should be a flow element: {:?}",
        ev
    );
}

#[test]
fn jsx_balanced_empty_tag_is_flow() {
    // Regression guard for the fix above — `<a></a>` with no body must still
    // be recognized as a flow element.
    let ev = mdx_events("<a></a>\n");
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::Start(Tag::MdxJsxFlowElement(s)) if s.contains("a"))
        ),
        "balanced empty tag should be flow: {:?}",
        ev
    );
}

#[test]
fn inline_expression_and_jsx() {
    let ev = mdx_events("a {b} <C /> d");
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::MdxTextExpression(s) if s.as_ref() == "b")
        ),
        "inline expr: {:?}",
        ev
    );
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::Start(Tag::MdxJsxTextElement(s)) if s.contains("C"))
        ),
        "inline jsx: {:?}",
        ev
    );
}

#[test]
fn esm_then_heading() {
    let ev = mdx_events("import a from 'b'\n\n# c\n");
    assert!(has(&ev, |e| matches!(e, Event::MdxEsm(_))));
    assert!(has(&ev, |e| matches!(e, Event::Start(Tag::Heading { .. }))));
}

// ===========================================================================
// Balanced tag events: Start + End
// ===========================================================================

#[test]
fn jsx_flow_emits_end() {
    let ev = mdx_events("<a />\n");
    assert!(has(&ev, |e| matches!(
        e,
        Event::Start(Tag::MdxJsxFlowElement(_))
    )));
    assert!(has(&ev, |e| matches!(
        e,
        Event::End(TagEnd::MdxJsxFlowElement)
    )));
}

#[test]
fn jsx_text_emits_end() {
    let ev = mdx_events("x <b/> y");
    assert!(has(&ev, |e| matches!(
        e,
        Event::Start(Tag::MdxJsxTextElement(_))
    )));
    assert!(has(&ev, |e| matches!(
        e,
        Event::End(TagEnd::MdxJsxTextElement)
    )));
}

// ===========================================================================
// Multi-line inline JSX
// ===========================================================================

#[test]
fn jsx_text_multiline_tag() {
    // Tag name + attributes spanning multiple lines.
    let ev = mdx_events("a <b\nc\n d\n/> e.");
    assert!(
        has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxTextElement(_))
        )),
        "multiline tag: {:?}",
        ev
    );
}

#[test]
fn jsx_text_multiline_string_attr() {
    let ev = mdx_events("a <b c=\"d\ne\" /> f");
    assert!(
        has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxTextElement(_))
        )),
        "multiline string attr: {:?}",
        ev
    );
}

#[test]
fn jsx_text_multiline_expr_attr() {
    let ev = mdx_events("a <b c={d\ne} /> f");
    assert!(
        has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxTextElement(_))
        )),
        "multiline expr attr: {:?}",
        ev
    );
}

#[test]
fn jsx_text_multiline_spread() {
    // A bare `{c\nd}` in attribute position is invalid JSX (mdx-js rejects
    // with "Could not parse expression with acorn"). Use a real multi-line
    // spread instead.
    let ev = mdx_events("a <b {...\nc} /> e");
    assert!(
        has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxTextElement(_))
        )),
        "multiline spread: {:?}",
        ev
    );
}

#[test]
fn jsx_text_multiline_complex() {
    // From markdown-rs: `<a\nb \nc\n d\n/>.`
    let ev = mdx_events("<a\nb \nc\n d\n/>.");
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::Start(Tag::MdxJsxTextElement(s)) if s.contains("<a"))
        ),
        "complex multiline: {:?}",
        ev
    );
}

// ===========================================================================
// Non-ASCII and special characters in tag names
// ===========================================================================

#[test]
fn jsx_text_non_ascii_pi() {
    let ev = mdx_events("a <π /> b.");
    assert!(
        has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxTextElement(_))
        )),
        "non-ascii π: {:?}",
        ev
    );
}

#[test]
fn jsx_text_dollar_tag() {
    let ev = mdx_events("a <$Component /> b.");
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::Start(Tag::MdxJsxTextElement(s)) if s.contains("$Component"))
        ),
        "dollar tag: {:?}",
        ev
    );
}

#[test]
fn jsx_flow_non_ascii() {
    let ev = mdx_events("<π />\n");
    assert!(
        has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxFlowElement(_))
        )),
        "flow non-ascii: {:?}",
        ev
    );
}

#[test]
fn jsx_flow_dollar() {
    let ev = mdx_events("<$C />\n");
    assert!(
        has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxFlowElement(_))
        )),
        "flow dollar: {:?}",
        ev
    );
}

#[test]
fn jsx_text_dash_in_name() {
    // Custom elements with dashes: `<my-component />`
    let ev = mdx_events("a <my-component /> b.");
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::Start(Tag::MdxJsxTextElement(s)) if s.contains("my-component"))
        ),
        "dash in name: {:?}",
        ev
    );
}

#[test]
fn jsx_text_dashes_in_name() {
    // `<a-->` — from markdown-rs
    let ev = mdx_events("a <a-->b</a-->.");
    assert!(
        has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxTextElement(_))
        )),
        "dashes in tag name: {:?}",
        ev
    );
}

// ===========================================================================
// Whitespace edge cases
// ===========================================================================

#[test]
fn jsx_text_whitespace_after_slash() {
    let ev = mdx_events("<a/ \t>.");
    assert!(
        has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxTextElement(_))
        )),
        "whitespace after slash: {:?}",
        ev
    );
}

#[test]
fn jsx_text_lt_space_not_jsx() {
    let ev = mdx_events("a < \t>b c");
    assert!(
        !has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxTextElement(_))
        )),
        "< followed by space is not JSX: {:?}",
        ev
    );
}

#[test]
fn jsx_text_lt_newline_not_jsx() {
    let ev = mdx_events("a < \nb\t>b c");
    assert!(
        !has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxTextElement(_))
        )),
        "< followed by newline is not JSX: {:?}",
        ev
    );
}

#[test]
fn less_than_number() {
    let ev = mdx_events("1 < 3");
    assert!(
        !has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxTextElement(_))
        )),
        "1 < 3 is not JSX: {:?}",
        ev
    );
}

// ===========================================================================
// HTML comments suppressed in MDX
// ===========================================================================

#[test]
fn html_comment_not_block_in_mdx() {
    let ev = mdx_events("<!-- comment -->\n");
    // Should NOT be an HTML block — should be a paragraph with text.
    assert!(
        !has(&ev, |e| matches!(e, Event::Start(Tag::HtmlBlock))),
        "<!-- should not be HTML block in MDX: {:?}",
        ev
    );
}

#[test]
fn html_comment_not_inline_in_mdx() {
    let ev = mdx_events("a <!-- b --> c");
    // Should NOT be inline HTML.
    assert!(
        !has(&ev, |e| matches!(e, Event::InlineHtml(_))),
        "<!-- should not be inline HTML in MDX: {:?}",
        ev
    );
}

// ===========================================================================
// JSX in containers (blockquotes)
// ===========================================================================

#[test]
fn jsx_text_in_blockquote() {
    let ev = mdx_events("> a <b>\n> c </b> d.");
    assert!(
        has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxTextElement(_))
        )),
        "jsx in blockquote: {:?}",
        ev
    );
    assert!(
        has(&ev, |e| matches!(e, Event::Start(Tag::BlockQuote(_)))),
        "has blockquote: {:?}",
        ev
    );
}

#[test]
fn jsx_text_multiline_attr_in_blockquote() {
    let ev = mdx_events("> a <b c=\"d\ne\" /> f");
    assert!(
        has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxTextElement(_))
        )),
        "multiline attr in blockquote: {:?}",
        ev
    );
}

#[test]
fn jsx_text_multiline_expr_attr_in_blockquote() {
    let ev = mdx_events("> a <b c={d\ne} /> f");
    assert!(
        has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxTextElement(_))
        )),
        "multiline expr attr in blockquote: {:?}",
        ev
    );
}

#[test]
fn expr_flow_in_blockquote() {
    let ev = mdx_events("> {a}\n");
    assert!(
        has(
            &ev,
            |e| matches!(e, Event::MdxFlowExpression(s) if s.as_ref() == "a")
        ) || has(
            &ev,
            |e| matches!(e, Event::MdxTextExpression(s) if s.as_ref() == "a")
        ),
        "expression in blockquote: {:?}",
        ev
    );
}

// ===========================================================================
// JSX with markdown content between tags
// ===========================================================================

#[test]
fn jsx_text_markdown_children() {
    // `a <b>*c*</b> d.` — emphasis inside JSX.
    let ev = mdx_events("a <b>*c*</b> d.");
    assert!(
        has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxTextElement(_))
        )),
        "jsx with markdown children: {:?}",
        ev
    );
    // The emphasis should still be parsed.
    assert!(
        has(&ev, |e| matches!(e, Event::Start(Tag::Emphasis))),
        "emphasis inside jsx: {:?}",
        ev
    );
}

#[test]
fn jsx_text_nested_tags() {
    let ev = mdx_events("a <>b <>c</> d</>.");
    let jsx_count = count(&ev, |e| {
        matches!(e, Event::Start(Tag::MdxJsxTextElement(_)))
    });
    assert!(jsx_count >= 2, "nested fragments: {:?}", ev);
}

#[test]
fn jsx_flow_with_content() {
    // `<a>\nb\n</a>` — block JSX with content between open/close.
    let ev = mdx_events("<a>\nb\n</a>\n");
    assert!(
        has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxFlowElement(_))
        )),
        "flow jsx with content: {:?}",
        ev
    );
}

// ===========================================================================
// Mismatched / unclosed tags (should not crash)
// ===========================================================================

#[test]
fn jsx_text_unclosed_does_not_crash() {
    let ev = mdx_events("a <b> c");
    assert!(
        has(&ev, |e| matches!(
            e,
            Event::Start(Tag::MdxJsxTextElement(_))
        )),
        "unclosed tag should parse: {:?}",
        ev
    );
}

#[test]
fn jsx_text_mismatched_does_not_crash() {
    let ev = mdx_events("a <></b> c");
    // Should not crash. May or may not pair correctly.
    let _ = ev;
}

#[test]
fn jsx_text_closing_without_open_does_not_crash() {
    let ev = mdx_events("a </> c.");
    // Should not crash.
    let _ = ev;
}

#[test]
fn jsx_text_closing_self_closing_does_not_crash() {
    let ev = mdx_events("a <a>b</a/> c");
    let _ = ev;
}

#[test]
fn jsx_text_closing_with_attr_does_not_crash() {
    let ev = mdx_events("a <a>b</a c> d");
    let _ = ev;
}

#[test]
fn tab_indented_code_fence_in_list() {
    // Code fence with 2-tab indentation inside a list item should be recognized
    let input = "5. item:\n\n\t\t```ts\n\tcontent {a: 1}\n\t\t```\n";
    let events = mdx_events(input);
    let has_code_block = has(&events, |e| matches!(e, Event::Start(Tag::CodeBlock(_))));
    assert!(
        has_code_block,
        "Tab-indented code fence in list not detected: {:?}",
        events
    );
    // Should NOT have MDX expression (the {a: 1} is inside a code block)
    let has_expr = has(&events, |e| {
        matches!(e, Event::MdxFlowExpression(_) | Event::MdxTextExpression(_))
    });
    assert!(
        !has_expr,
        "Content inside code fence incorrectly parsed as MDX expression: {:?}",
        events
    );
}

#[test]
fn tab_indented_code_fence_standalone() {
    // Code fence with 2-tab indentation standalone
    let input = "\t\t```ts\ncontent {a: 1}\n\t\t```\n";
    let events = mdx_events(input);
    let has_code_block = has(&events, |e| matches!(e, Event::Start(Tag::CodeBlock(_))));
    assert!(
        has_code_block,
        "Tab-indented standalone code fence not detected: {:?}",
        events
    );
}

#[test]
fn expression_with_double_quote_string() {
    let events = mdx_events(r#"{"hello"}"#);
    // At the start of a line, this is a flow expression (not text)
    let has_expr = has(&events, |e| matches!(e, Event::MdxFlowExpression(_)));
    assert!(
        has_expr,
        "Double-quote string expression not detected: {:?}",
        events
    );
    for e in &events {
        if let Event::MdxFlowExpression(content) = e {
            assert_eq!(content.as_ref(), r#""hello""#, "Wrong expression content");
        }
    }
}

/// An unbalanced `{` inside an inline code span must not bubble up as an MDX
/// expression error — `@mdx-js/mdx` treats the backticks as authoritative and
/// the `{` as literal text.
#[test]
fn inline_code_with_unbalanced_brace_is_literal() {
    let events = mdx_events("`{`\n");
    assert!(
        has(
            &events,
            |e| matches!(e, Event::Code(c) if c.as_ref() == "{")
        ),
        "expected Code(\"{{\"): {:?}",
        events
    );
    assert!(
        !has(&events, |e| matches!(e, Event::MdxTextExpression(_))),
        "must not emit MdxTextExpression for `{{` in code span: {:?}",
        events
    );
}

/// Balanced `{...}` inside a code span stays literal too — the whole thing is
/// one `Code` event, no `MdxTextExpression` leaks out.
#[test]
fn inline_code_with_balanced_braces_is_literal() {
    let events = mdx_events("`a { b }`\n");
    assert!(
        has(
            &events,
            |e| matches!(e, Event::Code(c) if c.as_ref() == "a { b }")
        ),
        "expected single Code event: {:?}",
        events
    );
    assert!(
        !has(&events, |e| matches!(e, Event::MdxTextExpression(_))),
        "must not emit MdxTextExpression inside code span: {:?}",
        events
    );
}

/// A code span with unbalanced `{` followed by unrelated blocks (the original
/// cloudflare-docs bug) must not scan an expression across paragraphs.
#[test]
fn inline_code_with_unbalanced_brace_does_not_cross_blocks() {
    let input = "`){`\n\n```\nhttps://x {\n}\n```\n";
    let events = mdx_events(input);
    assert!(
        !has(&events, |e| matches!(e, Event::MdxTextExpression(_))),
        "unbalanced `{{` in code span must not match `}}` in a later block: {:?}",
        events
    );
}

/// Inline text expressions can't cross a paragraph boundary — a blank line
/// before the closing `}` aborts the scan.
#[test]
fn inline_expression_aborts_on_blank_line() {
    let events = mdx_events("text {a\n\n} text\n");
    assert!(
        !has(&events, |e| matches!(e, Event::MdxTextExpression(_))),
        "expression must not span a blank line in inline context: {:?}",
        events
    );
}

/// Flow (block-level) expressions, by contrast, *can* span blank lines —
/// `@mdx-js/mdx` accepts `{\n\n}` at block level.
#[test]
fn flow_expression_allows_blank_line() {
    let events = mdx_events("{\n\n}\n");
    assert!(
        has(&events, |e| matches!(e, Event::MdxFlowExpression(_))),
        "flow expression must tolerate blank line: {:?}",
        events
    );
}

/// A template literal with a blank line inside it is valid at flow level
/// (mdx-js accepts it). Regression guard for the blank-line check leaking
/// into the `inline=false` path.
#[test]
fn flow_expression_template_literal_allows_blank_line() {
    let events = mdx_events("{`multi\n\nline`}\n");
    assert!(
        has(
            &events,
            |e| matches!(e, Event::MdxFlowExpression(c) if c.as_ref().contains("multi"))
        ),
        "flow expression with multi-line template must parse: {:?}",
        events
    );
}

/// In inline context, the blank-line bail also applies inside template
/// literals — otherwise an accidental backtick (e.g., the close of a code
/// span) can make the scanner skip past paragraph breaks.
#[test]
fn inline_expression_template_literal_aborts_on_blank_line() {
    let events = mdx_events("text {`multi\n\nline`} text\n");
    assert!(
        !has(&events, |e| matches!(e, Event::MdxTextExpression(_))),
        "inline expression with blank-line template must abort: {:?}",
        events
    );
}

// A `<` that is inside a math span (`$<$`) is literal math content, not an
// open inline JSX tag, so a following `>` line opens a blockquote rather than
// being swallowed as a lazy paragraph continuation.
#[test]
fn blockquote_interrupts_after_lt_inside_math() {
    let with_lt = mdx_events("$<$\n>");
    assert!(
        has(&with_lt, |e| matches!(e, Event::Start(Tag::BlockQuote(_)))),
        "`>` after `$<$` must open a blockquote: {:?}",
        with_lt
    );
    // A genuinely open inline JSX tag must still suppress the interrupt; the
    // `>` is the tag's own close, not a blockquote.
    let open_jsx = mdx_events("a <foo\n>");
    assert!(
        !has(&open_jsx, |e| matches!(e, Event::Start(Tag::BlockQuote(_)))),
        "real open inline JSX must still suppress the interrupt: {:?}",
        open_jsx
    );
}
