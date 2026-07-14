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

/// HAST arena plus per-`<a>` keep-children Replace patches (link-transform shape).
fn hast_with_link_replaces() -> (
    satteri_arena::Arena<satteri_arena::Hast>,
    Vec<satteri_ast::patch::Patch<satteri_arena::Hast>>,
) {
    let (mdast, _) =
        satteri_pulldown_cmark::parse(MARKDOWN, satteri_pulldown_cmark::DEFAULT_OPTIONS);
    let hast = satteri_ast::hast::mdast_arena_to_hast_arena(&mdast);
    let patches = satteri_bench::link_replace_patches(&hast);
    assert!(patches.len() > 10, "fixture should contain many links");
    (hast, patches)
}

/// Structural command application: one keep-children Replace per `<a>`.
#[divan::bench]
fn apply_link_replaces(bencher: divan::Bencher) {
    let (hast, patches) = hast_with_link_replaces();
    bencher
        .with_inputs(|| hast.clone())
        .bench_values(|mut arena| {
            satteri_ast::patch::apply_patches_in_place(&mut arena, divan::black_box(&patches))
                .unwrap();
            arena
        });
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

/// Parse Markdown without position tracking. Used by `markdown_to_html_fast`
/// and `mdx_to_js_fast` where downstream output doesn't carry positions.
#[divan::bench]
fn parse_no_positions(bencher: divan::Bencher) {
    let opts = satteri_pulldown_cmark::DEFAULT_OPTIONS;
    bencher.bench(|| satteri_pulldown_cmark::parse_no_positions(MARKDOWN, opts));
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
