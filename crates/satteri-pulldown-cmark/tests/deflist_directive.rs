//! Regression coverage for the definition-list ↔ directive colon interaction.
//!
//! Both extensions use a leading colon: deflist's `: definition` marker and
//! directive's `:::`/`::` fences. The deflist scanner must only claim a *lone*
//! colon, so enabling the deflist extension can never change how a directive
//! parses. See `scan_definition_list_definition_marker_with_indent`.

use satteri_pulldown_cmark::Options;

fn render(input: &str, opts: Options) -> String {
    let (arena, _) = satteri_pulldown_cmark::parse(input, opts);
    satteri_ast::mdast_to_html(&arena)
}

#[test]
fn deflist_renders_description_list() {
    let html = render("Apple\n:   Red.\n", Options::ENABLE_DEFINITION_LIST);
    assert_eq!(html.trim(), "<dl>\n<dt>Apple</dt>\n<dd>Red.</dd>\n</dl>");
}

#[test]
fn directive_parse_is_unaffected_by_deflist() {
    // A `:::` directive fence must parse identically whether or not the
    // definition-list extension is also enabled — the deflist marker must not
    // swallow the run of colons.
    let dir = Options::ENABLE_DIRECTIVE;
    let dl = Options::ENABLE_DEFINITION_LIST;
    let doc = ":::note\nbody\n:::\n";
    assert_eq!(render(doc, dir), render(doc, dir | dl));
}

#[test]
fn deflist_and_directive_coexist_in_one_document() {
    // A deflist entry followed by a directive block: the entry renders as a
    // `<dl>`, and the `:::` fence is not absorbed into a `<dd>`.
    let both = Options::ENABLE_DIRECTIVE | Options::ENABLE_DEFINITION_LIST;
    let html = render("Apple\n:   Red.\n\n:::note\nbody\n:::\n", both);
    assert!(html.contains("<dl>"), "deflist entry lost: {html}");
    assert!(html.contains("<dd>Red.</dd>"), "definition lost: {html}");
    assert!(
        !html.contains("<dd>:"),
        "directive fence leaked into dd: {html}"
    );
}

#[test]
fn deflist_only_leaves_directive_fences_as_paragraphs() {
    // deflist ON, directive OFF: a run of colons (`::`, `:::`) is never a
    // definition marker — the scanner only claims a lone colon. With no
    // directive extension to consume them, they stay literal paragraph text
    // instead of turning into a stray `<dl>`/`<dd>`.
    let dl = Options::ENABLE_DEFINITION_LIST;
    for input in [
        ":::note\nbody\n:::\n",
        "Term\n::: nope\n",
        "Term\n:: nope\n",
    ] {
        let out = render(input, dl);
        assert!(
            !out.contains("<dl>"),
            "colons became a deflist for {input:?}: {out}"
        );
        assert!(
            !out.contains("<dd>"),
            "colons became a dd for {input:?}: {out}"
        );
    }
}

#[test]
fn leaf_directive_parse_is_unaffected_by_deflist() {
    // The `::` leaf-directive fence (not only `:::`) must parse identically
    // whether or not deflist is also enabled.
    let dir = Options::ENABLE_DIRECTIVE;
    let dl = Options::ENABLE_DEFINITION_LIST;
    let doc = "::note[label]{#id}\n";
    assert_eq!(render(doc, dir), render(doc, dir | dl));
}

#[test]
fn both_extensions_off_leaves_colon_shapes_literal() {
    // Sanity floor: with neither extension enabled, deflist-shaped and
    // directive-shaped input are both plain paragraphs.
    let out = render("Apple\n:   Red.\n\n:::note\nbody\n:::\n", Options::empty());
    assert!(!out.contains("<dl>"), "unexpected deflist: {out}");
    assert!(!out.contains("<dd>"), "unexpected dd: {out}");
}
