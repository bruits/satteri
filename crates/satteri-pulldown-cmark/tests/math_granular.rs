use satteri_pulldown_cmark::{parse, Options};

fn render(input: &str, opts: Options) -> String {
    let (arena, _) = parse(input, opts);
    satteri_ast::mdast_to_html(&arena)
}

fn opts_single_only() -> Options {
    Options::ENABLE_MATH_SINGLE_DOLLAR
}

fn opts_multi_only() -> Options {
    Options::ENABLE_MATH_MULTI_DOLLAR
}

fn opts_all_individual() -> Options {
    Options::ENABLE_MATH_SINGLE_DOLLAR | Options::ENABLE_MATH_MULTI_DOLLAR
}

// Exercises every delimiter shape: single inline, double inline, and a block fence.
const INPUT: &str = "Inline $a$ and double $$b$$\n\n$$\n\\alpha\n$$";

#[test]
fn umbrella_flag_matches_combined_individual_flags() {
    let umbrella = render(INPUT, Options::ENABLE_MATH);
    let individual = render(INPUT, opts_all_individual());
    assert_eq!(umbrella, individual);
}

#[test]
fn single_only_keeps_inline_dollar_but_not_blocks() {
    let html = render("$x=y$", opts_single_only());
    assert!(html.contains("<math>"), "single $ should be inline math");

    let block = render("$$\n\\alpha\n$$", opts_single_only());
    assert!(
        !block.contains("<math display=\"block\">"),
        "block fences need multi-dollar; single-only must not parse them"
    );
}

#[test]
fn multi_only_keeps_double_and_blocks_but_not_single() {
    let single = render("$x=y$", opts_multi_only());
    assert!(!single.contains("<math>"), "lone $ stays literal");
    assert!(single.contains("$x=y$"), "lone $ renders verbatim");

    let double = render("$$\\alpha$$", opts_multi_only());
    assert!(double.contains("<math>"), "$$..$$ inline is math");

    let block = render("$$\n\\alpha\n$$", opts_multi_only());
    assert!(
        block.contains("<math display=\"block\">"),
        "$$ fence is display math"
    );
}

#[test]
fn multi_only_leaves_currency_literal() {
    let html = render("$50 to $100 billion", opts_multi_only());
    assert!(
        !html.contains("math-inline"),
        "paired currency $ is not math"
    );
    assert!(
        html.contains("$50 to $100 billion"),
        "currency renders verbatim"
    );
}

#[test]
fn no_math_flags_leaves_dollars_literal() {
    let html = render("$x=y$ and $$\\alpha$$", Options::empty());
    assert!(!html.contains("math-inline"));
    assert!(!html.contains("math-display"));
    assert!(html.contains("$x=y$"));
}
