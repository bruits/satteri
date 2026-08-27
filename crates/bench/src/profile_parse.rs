/// Profiling binary: hammers a parse/convert workload in a tight loop so
/// perf/flamegraph gets enough samples to show a meaningful call graph.
///
/// Run via: cargo flamegraph -p satteri-bench --bin profile_parse [-- <workload>]
/// Workloads: `parse` (default, with positions), `parse-no-pos`, `autolinks`,
/// `html`, `mdx`, `mdx-static`, `apply`. The `mdx*` workloads use the `.mdx`
/// fixture, `autolinks` the autolink-heavy one; the rest use the Markdown one.
fn main() {
    let md_src = include_str!("../fixtures/markdown.md");
    let mdx_src = include_str!("../fixtures/document.mdx");
    let autolinks_src = include_str!("../fixtures/autolinks.md");
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
        "autolinks" => (autolinks_src, |src, opts| {
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
        "stage-noop" => (md_src, |src, _opts| {
            std::hint::black_box(src.len());
        }),
        "stage-parse" => (md_src, |src, opts| {
            std::hint::black_box(satteri_pulldown_cmark::parse(src, opts));
        }),
        "stage-convert" => (md_src, |src, opts| {
            let (arena, _) = satteri_pulldown_cmark::parse(src, opts);
            std::hint::black_box(satteri_ast::hast::mdast_arena_to_hast_arena(&arena));
        }),
        "stage-render" => (md_src, |src, opts| {
            let (arena, _) = satteri_pulldown_cmark::parse(src, opts);
            let hast = satteri_ast::hast::mdast_arena_to_hast_arena(&arena);
            std::hint::black_box(satteri_ast::hast::hast_arena_to_html(&hast));
        }),
        "stage-fused" => (md_src, |src, opts| {
            let (arena, _) = satteri_pulldown_cmark::parse(src, opts);
            std::hint::black_box(satteri_ast::mdast_to_html(&arena));
        }),
        "stage-wire-mdast" => (md_src, |src, opts| {
            let (arena, _) = satteri_pulldown_cmark::parse(src, opts);
            std::hint::black_box(arena.to_raw_buffer());
        }),
        "stage-wire-hast" => (md_src, |src, opts| {
            let (arena, _) = satteri_pulldown_cmark::parse(src, opts);
            let hast = satteri_ast::hast::mdast_arena_to_hast_arena(&arena);
            std::hint::black_box(hast.to_raw_buffer());
        }),
        "apply" => (md_src, |src, opts| {
            let (mdast, _) = satteri_pulldown_cmark::parse(src, opts);
            let hast = satteri_ast::hast::mdast_arena_to_hast_arena(&mdast);
            let patches = satteri_bench::link_replace_patches(&hast);
            let mut applied = hast;
            satteri_ast::patch::apply_patches_in_place(&mut applied, &patches).unwrap();
            std::hint::black_box(applied);
        }),
        other => panic!("unknown workload: {other}"),
    };
    let mut opts = if workload.starts_with("mdx") {
        satteri_pulldown_cmark::MDX_OPTIONS
    } else {
        satteri_pulldown_cmark::DEFAULT_OPTIONS
    };
    // Ablation lever for sizing per-construct costs, e.g. SATTERI_STRIP=TABLES,MATH.
    if let Ok(strip) = std::env::var("SATTERI_STRIP") {
        use satteri_pulldown_cmark::Options;
        for name in strip.split(',') {
            let flag = match name.trim() {
                "TABLES" => Options::ENABLE_TABLES,
                "MATH" => Options::ENABLE_MATH,
                "FOOTNOTES" => Options::ENABLE_FOOTNOTES,
                "STRIKETHROUGH" => Options::ENABLE_STRIKETHROUGH,
                "TASKLISTS" => Options::ENABLE_TASKLISTS,
                "YAML" => Options::ENABLE_YAML_STYLE_METADATA_BLOCKS,
                "GFM" => Options::ENABLE_GFM,
                _ => continue,
            };
            opts.remove(flag);
        }
    }

    // Warm up to avoid cold-start noise.
    for _ in 0..100 {
        run(src, opts);
    }

    // Profile window, enough iterations for ~5s of samples.
    for _ in 0..iters {
        run(src, opts);
    }
}
