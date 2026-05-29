//! Verify that ConvertOptions overrides the strings emitted around the
//! GFM footnotes section so callers can localize them.

use satteri_ast::hast::{Backref, ConvertOptions};
use satteri_ast::mdast_to_html_with_options;

const MD: &str = "Text with footnote[^1].\n\n[^1]: First footnote.\n";

const MD_DOUBLE: &str = "See[^a] and[^a] again.\n\n[^a]: Shared note.\n";

fn html(md: &str, opts: &ConvertOptions) -> String {
    let (arena, _) = satteri_pulldown_cmark::parse(md, satteri_pulldown_cmark::DEFAULT_OPTIONS);
    mdast_to_html_with_options(&arena, opts)
}

#[test]
fn default_options_match_existing_english_strings() {
    let out = html(MD, &ConvertOptions::default());
    assert!(
        out.contains(">Footnotes<"),
        "expected default heading: {out}"
    );
    assert!(
        out.contains("aria-label=\"Back to reference 1\""),
        "expected default aria-label: {out}"
    );
    assert!(out.contains("\u{21a9}"), "expected default arrow: {out}");
}

#[test]
fn custom_footnote_label_overrides_h2_text() {
    let opts = ConvertOptions {
        footnote_label: "Notas al pie".to_string(),
        ..ConvertOptions::default()
    };
    let out = html(MD, &opts);
    assert!(
        out.contains(">Notas al pie<"),
        "expected custom heading: {out}"
    );
    assert!(!out.contains(">Footnotes<"));
}

#[test]
fn custom_back_label_substitutes_reference_token() {
    let opts = ConvertOptions {
        footnote_back_label: Backref::Template("Retour à la référence {reference}".to_string()),
        ..ConvertOptions::default()
    };
    let out = html(MD, &opts);
    assert!(
        out.contains("aria-label=\"Retour à la référence 1\""),
        "expected substituted aria-label: {out}"
    );
}

#[test]
fn custom_back_label_reflects_rerun_suffix() {
    let opts = ConvertOptions {
        footnote_back_label: Backref::Template("Retour à {reference}".to_string()),
        ..ConvertOptions::default()
    };
    let out = html(MD_DOUBLE, &opts);
    assert!(
        out.contains("aria-label=\"Retour à 1\""),
        "expected first backref: {out}"
    );
    assert!(
        out.contains("aria-label=\"Retour à 1-2\""),
        "expected second backref with -K suffix: {out}"
    );
}

#[test]
fn custom_back_content_replaces_arrow() {
    let opts = ConvertOptions {
        footnote_back_content: Backref::Template("haut".to_string()),
        ..ConvertOptions::default()
    };
    let out = html(MD, &opts);
    assert!(
        out.contains(">haut<"),
        "expected custom backref content: {out}"
    );
    assert!(!out.contains("\u{21a9}"));
}

#[test]
fn callback_back_label_gets_number_and_rerun_index() {
    let opts = ConvertOptions {
        footnote_back_label: Backref::Callback(Box::new(|n, k| {
            if k > 1 {
                format!("cb n={n} k={k}")
            } else {
                format!("cb n={n}")
            }
        })),
        ..ConvertOptions::default()
    };
    let out = html(MD_DOUBLE, &opts);
    assert!(
        out.contains("aria-label=\"cb n=1\""),
        "expected first callback output: {out}"
    );
    assert!(
        out.contains("aria-label=\"cb n=1 k=2\""),
        "expected second callback output: {out}"
    );
}

#[test]
fn callback_back_content_returns_per_backref_text() {
    let opts = ConvertOptions {
        footnote_back_content: Backref::Callback(Box::new(|_, k| {
            if k == 1 {
                "first".into()
            } else {
                "more".into()
            }
        })),
        ..ConvertOptions::default()
    };
    let out = html(MD_DOUBLE, &opts);
    assert!(
        out.contains(">first<"),
        "expected k=1 callback content: {out}"
    );
    assert!(
        out.contains(">more<"),
        "expected k>1 callback content: {out}"
    );
}
