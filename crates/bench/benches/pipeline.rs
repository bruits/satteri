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

// Parse benchmarks

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

// HAST benchmarks

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

// Rebuild benchmarks: isolate the arena-reconstruction cost (the dominant
// structural-mutation overhead per the JS CPU profile). `rebuild_lenient`
// re-emits the entire tree on every command batch, so 0 / 1 / N patches should
// all cost roughly the same — that gap is the incremental-rebuild opportunity.

fn rebuild_mdast_arena() -> satteri_arena::Arena<satteri_arena::Mdast> {
    satteri_pulldown_cmark::parse(MARKDOWN, satteri_pulldown_cmark::DEFAULT_OPTIONS).0
}

fn rebuild_paragraph_ids(arena: &satteri_arena::Arena<satteri_arena::Mdast>) -> Vec<u32> {
    use satteri_ast::mdast::MdastNodeType;
    (0..arena.len() as u32)
        .filter(|&id| arena.get_node(id).node_type == MdastNodeType::Paragraph as u8)
        .collect()
}

/// A minimal structured replacement: a single `text` node at sub-arena node 0.
fn rebuild_text_tree() -> satteri_arena::Arena<satteri_arena::Mdast> {
    use satteri_arena::{ArenaBuilder, Mdast};
    use satteri_ast::mdast::MdastNodeType;
    let mut b = ArenaBuilder::<Mdast>::new(String::new());
    b.open_node(MdastNodeType::Text as u8);
    let sref = b.alloc_string("x");
    b.set_data_current(&satteri_arena::encode_string_ref_data(sref));
    b.close_node();
    b.finish()
}

/// Full rebuild with zero patches — the pure cost of re-emitting every node.
#[divan::bench]
fn rebuild_noop(bencher: divan::Bencher) {
    let arena = rebuild_mdast_arena();
    bencher.bench(|| satteri_ast::rebuild::rebuild_lenient(&arena, &[]).unwrap());
}

/// Rebuild with a single replace (sparse) — exercises whether one mutation still
/// pays the full O(tree) cost.
#[divan::bench]
fn rebuild_replace_one(bencher: divan::Bencher) {
    use satteri_ast::rebuild::{rebuild_lenient, Patch};
    let arena = rebuild_mdast_arena();
    let id = rebuild_paragraph_ids(&arena)[0];
    let patches = vec![Patch::Replace {
        node_id: id,
        new_tree: rebuild_text_tree(),
        keep_children: false,
    }];
    bencher.bench(|| rebuild_lenient(&arena, &patches).unwrap());
}

/// Rebuild replacing every paragraph (dense) — the `build-subtree` workload.
#[divan::bench]
fn rebuild_replace_all_paragraphs(bencher: divan::Bencher) {
    use satteri_ast::rebuild::{rebuild_lenient, Patch};
    let arena = rebuild_mdast_arena();
    let patches: Vec<_> = rebuild_paragraph_ids(&arena)
        .into_iter()
        .map(|id| Patch::Replace {
            node_id: id,
            new_tree: rebuild_text_tree(),
            keep_children: false,
        })
        .collect();
    bencher.bench(|| rebuild_lenient(&arena, &patches).unwrap());
}

/// Build the binary command buffer JS sends for the `build-subtree` workload:
/// one CMD_REPLACE + SERDE_JSON payload per paragraph.
fn build_replace_command_buf(ids: &[u32]) -> Vec<u8> {
    const JSON: &[u8] = br#"{"type":"blockquote","children":[{"type":"heading","depth":3,"children":[{"type":"text","value":"Note"}]},{"type":"paragraph","children":[{"type":"text","value":"Rebuilt paragraph body."}]}]}"#;
    let mut buf = Vec::new();
    for &id in ids {
        buf.push(0x0b); // CMD_REPLACE
        buf.extend_from_slice(&id.to_le_bytes());
        buf.push(0x12); // PAYLOAD_SERDE_JSON
        buf.extend_from_slice(&(JSON.len() as u32).to_le_bytes());
        buf.extend_from_slice(JSON);
    }
    buf
}

/// Full apply path (parse command buffer → build sub-arenas → rebuild), i.e. the
/// whole `applyCommandsToMdastHandle` the JS `apply` frame calls. Compare to
/// `rebuild_replace_all_paragraphs` to split deserialization vs rebuild.
#[divan::bench]
fn apply_replace_all_paragraphs(bencher: divan::Bencher) {
    use satteri_plugin_api::js_commands::apply_mdast_commands_lenient;
    let arena = rebuild_mdast_arena();
    let buf = build_replace_command_buf(&rebuild_paragraph_ids(&arena));
    let dummy = |_: &str| -> satteri_arena::Arena<satteri_arena::Mdast> { unreachable!() };
    bencher
        .with_inputs(|| arena.clone())
        .bench_values(|a| apply_mdast_commands_lenient(a, &buf, &dummy).unwrap());
}

/// The 5-node replacement tree the `build-subtree` plugin emits, built directly
/// via `ArenaBuilder` (no JSON, no JsNode) — isolates sub-arena construction.
fn build_blocksubtree() -> satteri_arena::Arena<satteri_arena::Mdast> {
    use satteri_arena::{encode_string_ref_data, ArenaBuilder, Mdast};
    use satteri_ast::mdast::{codec::encode_heading_data, MdastNodeType as T};
    let mut b = ArenaBuilder::<Mdast>::new(String::new());
    b.open_node(T::Blockquote as u8);
    b.open_node(T::Heading as u8);
    b.set_data_current(&encode_heading_data(3));
    b.open_node(T::Text as u8);
    let s = b.alloc_string("Note");
    b.set_data_current(&encode_string_ref_data(s));
    b.close_node();
    b.close_node();
    b.open_node(T::Paragraph as u8);
    b.open_node(T::Text as u8);
    let s2 = b.alloc_string("Rebuilt paragraph body.");
    b.set_data_current(&encode_string_ref_data(s2));
    b.close_node();
    b.close_node();
    b.close_node();
    b.finish()
}

const REPLACEMENT_JSON: &str = r#"{"type":"blockquote","children":[{"type":"heading","depth":3,"children":[{"type":"text","value":"Note"}]},{"type":"paragraph","children":[{"type":"text","value":"Rebuilt paragraph body."}]}]}"#;

/// Split half 1: JSON → JsNode (serde parse + JsNode allocation), ×N.
#[divan::bench]
fn deserialize_jsnode_only(bencher: divan::Bencher) {
    use satteri_ast::commands::JsNode;
    let n = rebuild_paragraph_ids(&rebuild_mdast_arena()).len();
    bencher.bench(|| {
        for _ in 0..n {
            let node: JsNode = serde_json::from_str(REPLACEMENT_JSON).unwrap();
            divan::black_box(node);
        }
    });
}

/// Split half 2: build the replacement sub-arena via ArenaBuilder, ×N.
#[divan::bench]
fn build_subarena_only(bencher: divan::Bencher) {
    let n = rebuild_paragraph_ids(&rebuild_mdast_arena()).len();
    bencher.bench(|| {
        for _ in 0..n {
            divan::black_box(build_blocksubtree());
        }
    });
}

// Imperative-builder prototype: instead of JSON→JsNode→sub-arena, the plugin's
// builder emits an op-stream (OPEN/CLOSE/SET_VALUE/SET_DEPTH) that Rust replays
// directly into an ArenaBuilder — no JsNode. Measures the Rust-side ceiling.

fn build_replay_ops() -> Vec<u8> {
    use satteri_ast::mdast::MdastNodeType as T;
    let mut b = Vec::new();
    let open = |b: &mut Vec<u8>, t: u8| {
        b.push(0x01);
        b.push(t);
    };
    let close = |b: &mut Vec<u8>| b.push(0x02);
    let value = |b: &mut Vec<u8>, s: &str| {
        b.push(0x10);
        b.extend_from_slice(&(s.len() as u32).to_le_bytes());
        b.extend_from_slice(s.as_bytes());
    };
    let depth = |b: &mut Vec<u8>, d: u8| {
        b.push(0x11);
        b.push(d);
    };
    open(&mut b, T::Blockquote as u8);
    open(&mut b, T::Heading as u8);
    depth(&mut b, 3);
    open(&mut b, T::Text as u8);
    value(&mut b, "Note");
    close(&mut b);
    close(&mut b);
    open(&mut b, T::Paragraph as u8);
    open(&mut b, T::Text as u8);
    value(&mut b, "Rebuilt paragraph body.");
    close(&mut b);
    close(&mut b);
    close(&mut b);
    b
}

fn replay_into_arena(buf: &[u8]) -> satteri_arena::Arena<satteri_arena::Mdast> {
    use satteri_arena::{encode_string_ref_data, ArenaBuilder, Mdast};
    use satteri_ast::mdast::codec::encode_heading_data;
    let mut bld = ArenaBuilder::<Mdast>::new(String::new());
    let mut i = 0;
    while i < buf.len() {
        match buf[i] {
            0x01 => {
                bld.open_node(buf[i + 1]);
                i += 2;
            }
            0x02 => {
                bld.close_node();
                i += 1;
            }
            0x10 => {
                let len = u32::from_le_bytes(buf[i + 1..i + 5].try_into().unwrap()) as usize;
                let s = std::str::from_utf8(&buf[i + 5..i + 5 + len]).unwrap();
                let sref = bld.alloc_string(s);
                bld.set_data_current(&encode_string_ref_data(sref));
                i += 5 + len;
            }
            0x11 => {
                bld.set_data_current(&encode_heading_data(buf[i + 1]));
                i += 2;
            }
            _ => unreachable!(),
        }
    }
    bld.finish()
}

/// Replay the op-stream into a sub-arena, ×N — the imperative-builder Rust path.
/// Compare to `deserialize_jsnode_only` (52µs) + `build_subarena_only` (15µs).
#[divan::bench]
fn replay_ops_to_arena(bencher: divan::Bencher) {
    let n = rebuild_paragraph_ids(&rebuild_mdast_arena()).len();
    let ops = build_replay_ops();
    bencher.bench(|| {
        for _ in 0..n {
            divan::black_box(replay_into_arena(&ops));
        }
    });
}
