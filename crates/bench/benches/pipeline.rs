/// End-to-end Rust pipeline benchmarks using divan.
///
/// Covers the full stack: parse → HAST → HTML and MDX → JS.
/// Run with: `cargo bench -p satteri-bench`
const MARKDOWN: &str = include_str!("../fixtures/markdown.md");

/// A short MDX snippet representative of real-world usage.
const MDX: &str = r#"import {Chart} from './chart.js'

# Hello, world

Some *emphasis* and **strong** content.

<Chart values={[1, 2, 3]} />

> A blockquote with a [link](https://example.com).

- item one
- item two
- item three
"#;

/// Same as MDX but with an `export const components` override declaration.
const MDX_WITH_OVERRIDES: &str = r#"import {Chart} from './chart.js'
import {CustomHeading} from './heading.js'

export const components = { h1: CustomHeading }

# Hello, world

Some *emphasis* and **strong** content.

<Chart values={[1, 2, 3]} />

> A blockquote with a [link](https://example.com).

- item one
- item two
- item three
"#;

fn main() {
    divan::main();
}

/// Parse Markdown source into an Arena.
#[divan::bench]
fn parse(bencher: divan::Bencher) {
    let opts = satteri_pulldown_cmark::DEFAULT_OPTIONS;
    bencher.bench(|| satteri_pulldown_cmark::parse(MARKDOWN, opts));
}

/// Parse Markdown source and serialise to a flat binary buffer.
#[divan::bench]
fn parse_to_buffer(bencher: divan::Bencher) {
    let opts = satteri_pulldown_cmark::DEFAULT_OPTIONS;
    bencher.bench(|| {
        let (arena, _) = satteri_pulldown_cmark::parse(MARKDOWN, opts);
        arena.to_raw_buffer()
    });
}

// pulldown-cmark comparison

/// pulldown-cmark: parse to events (GFM + Math extensions).
#[divan::bench]
fn pulldown_parse_events(bencher: divan::Bencher) {
    use satteri_pulldown_cmark::{Options, Parser};

    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_MATH;

    bencher.bench(|| {
        let parser = Parser::new_ext(MARKDOWN, opts);
        for event in parser {
            std::hint::black_box(&event);
        }
    });
}

/// pulldown-cmark: parse to events with MDX enabled.
#[divan::bench]
fn pulldown_parse_events_mdx(bencher: divan::Bencher) {
    use satteri_pulldown_cmark::{Options, Parser};

    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_MATH
        | Options::ENABLE_MDX;

    bencher.bench(|| {
        let parser = Parser::new_ext(MARKDOWN, opts);
        for event in parser {
            std::hint::black_box(&event);
        }
    });
}

/// pulldown-cmark MDX: parse the MDX snippet.
#[divan::bench]
fn pulldown_mdx_parse(bencher: divan::Bencher) {
    use satteri_pulldown_cmark::{Options, Parser};

    let opts = Options::ENABLE_TABLES | Options::ENABLE_MATH | Options::ENABLE_MDX;

    bencher.bench(|| {
        let parser = Parser::new_ext(MDX, opts);
        for event in parser {
            std::hint::black_box(&event);
        }
    });
}

/// Full pipeline: Markdown source → Arena → HTML string.
#[divan::bench]
fn full_pipeline_to_html(bencher: divan::Bencher) {
    let opts = satteri_pulldown_cmark::DEFAULT_OPTIONS;
    bencher.bench(|| {
        let (arena, _) = satteri_pulldown_cmark::parse(MARKDOWN, opts);
        satteri_ast::mdast_to_html(&arena)
    });
}

/// Given a pre-parsed MDAST arena, convert to HAST arena (no buffer round-trip).
#[divan::bench]
fn mdast_arena_to_hast_arena(bencher: divan::Bencher) {
    let (arena, _) =
        satteri_pulldown_cmark::parse(MARKDOWN, satteri_pulldown_cmark::DEFAULT_OPTIONS);
    bencher.bench(|| satteri_ast::hast::mdast_arena_to_hast_arena(&arena));
}

/// Given a pre-built HAST arena, render to HTML (no buffer).
#[divan::bench]
fn hast_arena_to_html(bencher: divan::Bencher) {
    let (arena, _) =
        satteri_pulldown_cmark::parse(MARKDOWN, satteri_pulldown_cmark::DEFAULT_OPTIONS);
    let hast = satteri_ast::hast::mdast_arena_to_hast_arena(&arena);
    bencher.bench(|| satteri_ast::hast::hast_arena_to_html(&hast));
}

// MDX benchmarks: full pipeline and step-by-step breakdown

/// Full pipeline: MDX source → JavaScript (parse + mdast→hast + hast→OXC + serialize).
#[divan::bench]
fn mdx_compile(bencher: divan::Bencher) {
    bencher.bench(|| {
        satteri_mdxjs::compile(
            MDX,
            &satteri_mdxjs::Options::default(),
            satteri_pulldown_cmark::MDX_OPTIONS,
        )
        .unwrap()
    });
}

/// MDX compile with optimize_static enabled (no component overrides in source).
#[divan::bench]
fn mdx_compile_optimize_static(bencher: divan::Bencher) {
    let opts = satteri_mdxjs::Options {
        optimize_static: Some(satteri_mdxjs::OptimizeStaticConfig::default()),
        ..Default::default()
    };
    bencher
        .bench(|| satteri_mdxjs::compile(MDX, &opts, satteri_pulldown_cmark::MDX_OPTIONS).unwrap());
}

/// MDX compile with optimize_static + source has `export const components`.
#[divan::bench]
fn mdx_compile_optimize_static_with_overrides(bencher: divan::Bencher) {
    let opts = satteri_mdxjs::Options {
        optimize_static: Some(satteri_mdxjs::OptimizeStaticConfig::default()),
        ..Default::default()
    };
    bencher.bench(|| {
        satteri_mdxjs::compile(
            MDX_WITH_OVERRIDES,
            &opts,
            satteri_pulldown_cmark::MDX_OPTIONS,
        )
        .unwrap()
    });
}

/// Step 1 of MDX compile: parse MDX source into an Arena.
#[divan::bench]
fn mdx_step1_parse(bencher: divan::Bencher) {
    let opts = satteri_pulldown_cmark::MDX_OPTIONS;
    bencher.bench(|| satteri_pulldown_cmark::parse(MDX, opts));
}

/// Step 2 of MDX compile: MDAST arena → HAST arena.
#[divan::bench]
fn mdx_step2_mdast_to_hast(bencher: divan::Bencher) {
    let (arena, _) = satteri_pulldown_cmark::parse(MDX, satteri_pulldown_cmark::MDX_OPTIONS);

    bencher.bench(|| satteri_ast::hast::mdast_arena_to_hast_arena(&arena));
}

/// Step 3 of MDX compile: HAST arena → OXC ES AST → JavaScript.
#[divan::bench]
fn mdx_step3_hast_to_js(bencher: divan::Bencher) {
    let (arena, _) = satteri_pulldown_cmark::parse(MDX, satteri_pulldown_cmark::MDX_OPTIONS);
    let hast_arena = satteri_ast::hast::mdast_arena_to_hast_arena(&arena);
    let opts = satteri_mdxjs::Options::default();

    bencher.bench(|| satteri_mdxjs::compile_hast_arena(&hast_arena, &opts).unwrap());
}
