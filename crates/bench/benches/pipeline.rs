/// End-to-end Rust pipeline benchmarks using divan.
///
/// Covers the full stack: parse → HAST → HTML and MDX → JS.
/// Run with: `cargo bench -p tryckeri-bench`
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

fn main() {
    divan::main();
}

// ---------------------------------------------------------------------------
// Parse benchmarks
// ---------------------------------------------------------------------------

/// Parse Markdown source into an Arena.
#[divan::bench]
fn parse(bencher: divan::Bencher) {
    let opts = parser::ParseOptions::default();
    bencher.bench(|| parser::parse(MARKDOWN, &opts));
}

/// Parse Markdown source and serialise to a flat binary buffer.
#[divan::bench]
fn parse_to_buffer(bencher: divan::Bencher) {
    let opts = parser::ParseOptions::default();
    bencher.bench(|| {
        let (arena, _) = parser::parse(MARKDOWN, &opts);
        arena.to_raw_buffer()
    });
}

// ---------------------------------------------------------------------------
// pulldown-cmark comparison
// ---------------------------------------------------------------------------

/// pulldown-cmark: parse to events (GFM + Math extensions).
#[divan::bench]
fn pulldown_parse_events(bencher: divan::Bencher) {
    use pulldown_cmark::{Options, Parser};

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
    use pulldown_cmark::{Options, Parser};

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

/// pulldown-cmark: parse + render to HTML string.
#[divan::bench]
fn pulldown_to_html(bencher: divan::Bencher) {
    use pulldown_cmark::{html, Options, Parser};

    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_MATH;

    bencher.bench(|| {
        let parser = Parser::new_ext(MARKDOWN, opts);
        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);
        html_output
    });
}

/// pulldown-cmark MDX: parse the MDX snippet.
#[divan::bench]
fn pulldown_mdx_parse(bencher: divan::Bencher) {
    use pulldown_cmark::{Options, Parser};

    let opts = Options::ENABLE_TABLES | Options::ENABLE_MATH | Options::ENABLE_MDX;

    bencher.bench(|| {
        let parser = Parser::new_ext(MDX, opts);
        for event in parser {
            std::hint::black_box(&event);
        }
    });
}

// ---------------------------------------------------------------------------
// HAST benchmarks
// ---------------------------------------------------------------------------

/// Full pipeline: Markdown source → Arena → HTML string.
#[divan::bench]
fn full_pipeline_to_html(bencher: divan::Bencher) {
    let opts = parser::ParseOptions::default();
    bencher.bench(|| {
        let (arena, _) = parser::parse(MARKDOWN, &opts);
        tryckeri_hast::mdast_to_html(&arena)
    });
}

/// Given a pre-serialised MDAST buffer, convert to HAST buffer.
#[divan::bench]
fn mdast_buffer_to_hast_buffer(bencher: divan::Bencher) {
    let (arena, _) = parser::parse(MARKDOWN, &parser::ParseOptions::default());
    let mdast_buf = arena.to_raw_buffer();

    bencher.bench(|| tryckeri_hast::mdast_to_hast_buffer(&mdast_buf).unwrap());
}

/// Given a pre-built HAST binary buffer, emit an HTML string.
#[divan::bench]
fn hast_buffer_to_html(bencher: divan::Bencher) {
    let (arena, _) = parser::parse(MARKDOWN, &parser::ParseOptions::default());
    let mdast_buf = arena.to_raw_buffer();
    let hast_buf = tryckeri_hast::mdast_to_hast_buffer(&mdast_buf).unwrap();

    bencher.bench(|| tryckeri_hast::hast_buffer_to_html(&hast_buf).unwrap());
}

// ---------------------------------------------------------------------------
// MDX benchmarks — full pipeline and step-by-step breakdown
// ---------------------------------------------------------------------------

/// Full pipeline: MDX source → JavaScript (parse + mdast→hast + hast→OXC + serialize).
#[divan::bench]
fn mdx_compile(bencher: divan::Bencher) {
    bencher.bench(|| mdxjs::compile(MDX, &mdxjs::Options::default()).unwrap());
}

/// Compile from a pre-parsed MDAST binary buffer — skips the parse step.
#[divan::bench]
fn mdx_compile_from_buffer(bencher: divan::Bencher) {
    let (arena, _) = parser::parse(MDX, &parser::ParseOptions::mdx());
    let mdast_buf = arena.to_raw_buffer();

    bencher.bench(|| mdxjs::compile_arena_bytes(&mdast_buf, &mdxjs::Options::default()).unwrap());
}

// ---- Step-by-step breakdown ----

/// Step 1 of MDX compile: parse MDX source into an Arena.
#[divan::bench]
fn mdx_step1_parse(bencher: divan::Bencher) {
    let opts = parser::ParseOptions::mdx();
    bencher.bench(|| parser::parse(MDX, &opts));
}

/// Step 2 of MDX compile: Arena → boxed hast::Node tree.
#[divan::bench]
fn mdx_step2_mdast_to_hast(bencher: divan::Bencher) {
    let (arena, _) = parser::parse(MDX, &parser::ParseOptions::mdx());

    bencher.bench(|| mdxjs::mdast_to_hast(&arena, None));
}

/// Step 3 of MDX compile: hast::Node tree → OXC ES AST + recma plugins + serialize.
#[divan::bench]
fn mdx_step3_hast_to_js(bencher: divan::Bencher) {
    use mdxjs::hast_util_to_oxc_program;
    use oxc_allocator::Allocator;
    use rustc_hash::FxHashSet;

    let (arena, _) = parser::parse(MDX, &parser::ParseOptions::mdx());
    let hast = mdxjs::mdast_to_hast(&arena, None);
    let opts = mdxjs::Options::default();

    bencher.bench(|| {
        let alloc = Allocator::default();
        let mut explicit_jsxs = FxHashSet::default();
        let mut program =
            hast_util_to_oxc_program(&hast, &opts, None, &mut explicit_jsxs, &alloc).unwrap();
        mdxjs::mdx_plugin_recma_document(&mut program, &opts, None, &alloc).unwrap();
        mdxjs::mdx_plugin_recma_jsx_rewrite(&mut program, &opts, None, &explicit_jsxs, &alloc)
            .unwrap();
    });
}
