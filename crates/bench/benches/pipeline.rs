/// End-to-end Rust pipeline benchmarks using divan.
///
/// Covers the real entry points: parse, Markdown → HTML, and MDX → JS.
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
fn parse_markdown(bencher: divan::Bencher) {
    let opts = satteri_pulldown_cmark::DEFAULT_OPTIONS;
    bencher.bench(|| satteri_pulldown_cmark::parse(MARKDOWN, opts));
}

/// Parse MDX source into an Arena.
#[divan::bench]
fn parse_mdx(bencher: divan::Bencher) {
    let opts = satteri_pulldown_cmark::MDX_OPTIONS;
    bencher.bench(|| satteri_pulldown_cmark::parse(MDX, opts));
}

// pulldown-cmark comparison (parse to events — for digging into parser regressions)

/// pulldown-cmark: parse Markdown to events with the default extension set.
#[divan::bench]
fn pulldown_parse_events(bencher: divan::Bencher) {
    use satteri_pulldown_cmark::Parser;

    let opts = satteri_pulldown_cmark::DEFAULT_OPTIONS;
    bencher.bench(|| {
        let parser = Parser::new_ext(MARKDOWN, opts);
        for event in parser {
            std::hint::black_box(&event);
        }
    });
}

/// pulldown-cmark: same extensions as `pulldown_parse_events`, plus MDX
/// (`MDX_OPTIONS` is exactly `DEFAULT_OPTIONS | ENABLE_MDX`).
#[divan::bench]
fn pulldown_parse_events_mdx(bencher: divan::Bencher) {
    use satteri_pulldown_cmark::Parser;

    let opts = satteri_pulldown_cmark::MDX_OPTIONS;
    bencher.bench(|| {
        let parser = Parser::new_ext(MARKDOWN, opts);
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

// MDX: full source → JavaScript.

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
