use satteri_pulldown_cmark::{parse, Options};

fn render(input: &str, opts: Options) -> String {
    let (arena, _) = parse(input, opts);
    satteri_ast::mdast_to_html(&arena)
}

#[test]
fn single_dollar_text_math_default_on() {
    let html = render("inline $x = 1$ here", Options::ENABLE_MATH);
    assert!(
        html.contains("<code") || html.contains("class=\"language-math"),
        "expected single-$ to parse as inline math, got: {html}"
    );
}

#[test]
fn single_dollar_text_math_disabled_keeps_dollars_literal() {
    let opts = Options::ENABLE_MATH | Options::DISABLE_SINGLE_DOLLAR_TEXT_MATH;
    let html = render("the deficit grew from $50 to $100 billion", opts);
    assert!(
        html.contains("$50") && html.contains("$100"),
        "expected literal dollars, got: {html}"
    );
    assert!(
        !html.contains("<code"),
        "expected no math element, got: {html}"
    );
}

#[test]
fn double_dollar_display_math_still_works_when_single_disabled() {
    let opts = Options::ENABLE_MATH | Options::DISABLE_SINGLE_DOLLAR_TEXT_MATH;
    let html = render("text $$x^2$$ more", opts);
    assert!(
        html.contains("x^2") && html.contains("<code"),
        "expected $$..$$ to still parse as display math, got: {html}"
    );
}

#[test]
fn block_math_fence_still_works_when_single_disabled() {
    let opts = Options::ENABLE_MATH | Options::DISABLE_SINGLE_DOLLAR_TEXT_MATH;
    let html = render("$$\nx = 1\n$$\n", opts);
    assert!(
        html.contains("x = 1"),
        "expected block math fence to still parse, got: {html}"
    );
}
