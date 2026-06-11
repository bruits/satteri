use satteri_pulldown_cmark::{parse, Options};

fn render(input: &str, opts: Options) -> String {
    let (arena, _) = parse(input, opts);
    satteri_ast::mdast_to_html(&arena)
}

fn opts_quotes_only() -> Options {
    Options::ENABLE_SMART_QUOTES
}

fn opts_dashes_only() -> Options {
    Options::ENABLE_SMART_DASHES
}

fn opts_ellipses_only() -> Options {
    Options::ENABLE_SMART_ELLIPSES
}

fn opts_all_individual() -> Options {
    Options::ENABLE_SMART_QUOTES | Options::ENABLE_SMART_DASHES | Options::ENABLE_SMART_ELLIPSES
}

const INPUT: &str = r#""Hello," she said -- it was... unexpected."#;

#[test]
fn quotes_only_converts_quotes_but_not_dashes_or_ellipses() {
    let html = render(INPUT, opts_quotes_only());
    assert!(html.contains('\u{201c}'), "should have left double quote");
    assert!(html.contains('\u{201d}'), "should have right double quote");
    assert!(html.contains("--"), "dashes should stay as ASCII");
    assert!(html.contains("..."), "dots should stay as ASCII");
}

#[test]
fn dashes_only_converts_dashes_but_not_quotes_or_ellipses() {
    let html = render(INPUT, opts_dashes_only());
    assert!(html.contains('\u{2013}'), "should have en-dash");
    assert!(html.contains('"'), "quotes should stay as ASCII");
    assert!(html.contains("..."), "dots should stay as ASCII");
}

#[test]
fn ellipses_only_converts_dots_but_not_quotes_or_dashes() {
    let html = render(INPUT, opts_ellipses_only());
    assert!(html.contains('\u{2026}'), "should have ellipsis");
    assert!(html.contains('"'), "quotes should stay as ASCII");
    assert!(html.contains("--"), "dashes should stay as ASCII");
}

#[test]
fn all_individual_flags_matches_combined_flag() {
    let combined = render(INPUT, Options::ENABLE_SMART_PUNCTUATION);
    let individual = render(INPUT, opts_all_individual());
    assert_eq!(combined, individual);
}

#[test]
fn no_flags_leaves_everything_as_ascii() {
    let html = render(INPUT, Options::empty());
    assert!(html.contains('"'));
    assert!(html.contains("--"));
    assert!(html.contains("..."));
}

#[test]
fn dashes_em_dash_triple() {
    let html = render("a---b", opts_dashes_only());
    assert!(html.contains('\u{2014}'), "--- should become em-dash");
}

#[test]
fn dashes_en_dash_double() {
    let html = render("a--b", opts_dashes_only());
    assert!(html.contains('\u{2013}'), "-- should become en-dash");
}

#[test]
fn single_quotes_with_quotes_flag() {
    let html = render("'hello'", opts_quotes_only());
    assert!(
        html.contains('\u{2018}') || html.contains('\u{2019}'),
        "single quotes should be curly"
    );
    assert!(!html.contains("'hello'"), "straight quotes should be gone");
}

#[test]
fn combined_flag_still_works() {
    let html = render(INPUT, Options::ENABLE_SMART_PUNCTUATION);
    assert!(!html.contains('"'), "no straight double quotes");
    assert!(!html.contains("--"), "no ASCII dashes");
    assert!(!html.contains("..."), "no ASCII dots");
}

#[test]
fn double_quote_opens_after_ascii_letter() {
    let html = render(r#"x"About Me""#, opts_quotes_only());
    assert_eq!(html, "<p>x“About Me”</p>\n");
}

#[test]
fn double_quote_opens_after_non_ascii_letter() {
    let html = render(r#"에"About Me""#, opts_quotes_only());
    assert_eq!(html, "<p>에“About Me”</p>\n");
}

#[test]
fn double_quote_closes_before_ascii_letter() {
    let html = render(r#""About Me"x"#, opts_quotes_only());
    assert_eq!(html, "<p>“About Me”x</p>\n");
}

#[test]
fn double_quote_closes_before_non_ascii_letter() {
    let html = render(r#""About Me"로"#, opts_quotes_only());
    assert_eq!(html, "<p>“About Me”로</p>\n");
}

#[test]
fn double_quote_opens_and_closes_next_to_ascii_letters() {
    let html = render(r#"x"About Me"y"#, opts_quotes_only());
    assert_eq!(html, "<p>x“About Me”y</p>\n");
}

#[test]
fn double_quote_opens_and_closes_next_to_non_ascii_letters() {
    let html = render(r#"에"About Me"로"#, opts_quotes_only());
    assert_eq!(html, "<p>에“About Me”로</p>\n");
}
