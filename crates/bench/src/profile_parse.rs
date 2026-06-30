/// Profiling binary: hammers a parse/convert workload in a tight loop so
/// perf/flamegraph gets enough samples to show a meaningful call graph.
///
/// Run via: cargo flamegraph -p satteri-bench --bin profile_parse [-- <workload>]
/// Workloads: `parse` (default, with positions), `parse-no-pos`, `html`, `mdx`.
/// The `mdx` workload uses the `.mdx` fixture; the rest use the Markdown one.
fn main() {
    let md_src = include_str!("../fixtures/markdown.md");
    let mdx_src = include_str!("../fixtures/document.mdx");
    let workload = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "parse".to_string());
    let iters: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(50_000);

    let (src, run): (&str, fn(&str, satteri_pulldown_cmark::Options)) = match workload.as_str() {
        "parse" => (md_src, |src, opts| {
            std::hint::black_box(satteri_pulldown_cmark::parse(src, opts));
        }),
        "parse-no-pos" => (md_src, |src, opts| {
            std::hint::black_box(satteri_pulldown_cmark::parse_no_positions(src, opts));
        }),
        "html" => (md_src, |src, opts| {
            let (arena, _) = satteri_pulldown_cmark::parse(src, opts);
            std::hint::black_box(satteri_ast::mdast_to_html(&arena));
        }),
        "mdx" => (mdx_src, |src, _opts| {
            let out = satteri_mdxjs::compile(
                src,
                &satteri_mdxjs::Options::default(),
                satteri_pulldown_cmark::MDX_OPTIONS,
            )
            .unwrap();
            std::hint::black_box(out);
        }),
        "mdx-static" => (mdx_src, |src, _opts| {
            let opts = satteri_mdxjs::Options {
                optimize_static: Some(satteri_mdxjs::OptimizeStaticConfig::default()),
                ..Default::default()
            };
            let out =
                satteri_mdxjs::compile(src, &opts, satteri_pulldown_cmark::MDX_OPTIONS).unwrap();
            std::hint::black_box(out);
        }),
        other => panic!("unknown workload: {other}"),
    };
    let opts = if workload.starts_with("mdx") {
        satteri_pulldown_cmark::MDX_OPTIONS
    } else {
        satteri_pulldown_cmark::DEFAULT_OPTIONS
    };

    // Warm up to avoid cold-start noise.
    for _ in 0..100 {
        run(src, opts);
    }

    // Profile window, enough iterations for ~5s of samples.
    for _ in 0..iters {
        run(src, opts);
    }
}
