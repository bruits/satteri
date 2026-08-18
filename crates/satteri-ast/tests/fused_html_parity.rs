//! The fused mdast→HTML emitter must be byte-identical to the two-stage pipeline, its own oracle.

use satteri_arena::{Arena, Mdast};
use satteri_ast::hast::{
    Backref, ConvertOptions, hast_arena_to_html, mdast_arena_to_hast_arena_with_options,
};
use satteri_ast::mdast::{ListItemData, MdastNodeType};
use satteri_ast::try_mdast_to_html_fused;
use satteri_pulldown_cmark::Options;

const SPEC_JSON: &str =
    include_str!("../../satteri-pulldown-cmark/third_party/CommonMark/spec.json");

const SUITE_FILES: &[(&str, &str)] = &[
    (
        "spec",
        include_str!("../../satteri-pulldown-cmark/tests/suite/spec.rs"),
    ),
    (
        "table",
        include_str!("../../satteri-pulldown-cmark/tests/suite/table.rs"),
    ),
    (
        "gfm_table",
        include_str!("../../satteri-pulldown-cmark/tests/suite/gfm_table.rs"),
    ),
    (
        "gfm_tasklist",
        include_str!("../../satteri-pulldown-cmark/tests/suite/gfm_tasklist.rs"),
    ),
    (
        "gfm_autolink",
        include_str!("../../satteri-pulldown-cmark/tests/suite/gfm_autolink.rs"),
    ),
    (
        "gfm_strikethrough",
        include_str!("../../satteri-pulldown-cmark/tests/suite/gfm_strikethrough.rs"),
    ),
    (
        "strikethrough",
        include_str!("../../satteri-pulldown-cmark/tests/suite/strikethrough.rs"),
    ),
    (
        "footnotes",
        include_str!("../../satteri-pulldown-cmark/tests/suite/footnotes.rs"),
    ),
    (
        "math",
        include_str!("../../satteri-pulldown-cmark/tests/suite/math.rs"),
    ),
    (
        "math_multi_dollar",
        include_str!("../../satteri-pulldown-cmark/tests/suite/math_multi_dollar.rs"),
    ),
    (
        "metadata_blocks",
        include_str!("../../satteri-pulldown-cmark/tests/suite/metadata_blocks.rs"),
    ),
    (
        "definition_list",
        include_str!("../../satteri-pulldown-cmark/tests/suite/definition_list.rs"),
    ),
    (
        "container_extensions",
        include_str!("../../satteri-pulldown-cmark/tests/suite/container_extensions.rs"),
    ),
    (
        "heading_attrs",
        include_str!("../../satteri-pulldown-cmark/tests/suite/heading_attrs.rs"),
    ),
    (
        "smart_punct",
        include_str!("../../satteri-pulldown-cmark/tests/suite/smart_punct.rs"),
    ),
    (
        "smartypants",
        include_str!("../../satteri-pulldown-cmark/tests/suite/smartypants.rs"),
    ),
    (
        "super_sub",
        include_str!("../../satteri-pulldown-cmark/tests/suite/super_sub.rs"),
    ),
    (
        "wikilinks",
        include_str!("../../satteri-pulldown-cmark/tests/suite/wikilinks.rs"),
    ),
    (
        "regression",
        include_str!("../../satteri-pulldown-cmark/tests/suite/regression.rs"),
    ),
];

const FIXTURES: &[(&str, &str)] = &[
    (
        "markdown.md",
        include_str!("../../bench/fixtures/markdown.md"),
    ),
    (
        "autolinks.md",
        include_str!("../../bench/fixtures/autolinks.md"),
    ),
    (
        "document.mdx",
        include_str!("../../bench/fixtures/document.mdx"),
    ),
];

/// Every extension the parser exposes, so no converter arm is unreachable for want of a feature bit.
fn everything() -> Options {
    Options::all()
}

fn option_matrix() -> Vec<(&'static str, Options)> {
    vec![
        ("empty", Options::empty()),
        ("default", satteri_pulldown_cmark::DEFAULT_OPTIONS),
        ("mdx", satteri_pulldown_cmark::MDX_OPTIONS),
        ("all", everything()),
        ("all-no-mdx", everything() - Options::ENABLE_MDX),
    ]
}

fn two_stage(arena: &Arena<Mdast>, options: &ConvertOptions) -> String {
    let hast = mdast_arena_to_hast_arena_with_options(arena, options);
    hast_arena_to_html(&hast)
}

/// `hName` / `hProperties` / `hChildren` on any node is the documented reason
/// the fused path may decline; anything else declining is a bug.
fn has_h_data(arena: &Arena<Mdast>) -> bool {
    arena.node_data.values().any(|blob| {
        let text = String::from_utf8_lossy(blob);
        text.contains("\"hName\"")
            || text.contains("\"hProperties\"")
            || text.contains("\"hChildren\"")
    })
}

fn assert_parity(label: &str, source: &str, parse_options: Options, convert: &ConvertOptions) {
    let (arena, _) = satteri_pulldown_cmark::parse(source, parse_options);
    let expected = two_stage(&arena, convert);
    match try_mdast_to_html_fused(&arena, convert) {
        Some(actual) => assert_eq!(
            expected, actual,
            "{label}\ninput: {source:?}\ntwo-stage: {expected:?}\nfused:     {actual:?}"
        ),
        None => assert!(
            has_h_data(&arena),
            "{label}: fused path declined without h-shaped node data\ninput: {source:?}"
        ),
    }

    assert_eq!(
        satteri_ast::mdast_to_html_with_options(&arena, convert),
        expected,
        "{label}: entry point diverged\ninput: {source:?}"
    );
}

fn assert_parity_all_options(label: &str, source: &str) {
    let convert = ConvertOptions::default();
    for (name, parse_options) in option_matrix() {
        assert_parity(
            &format!("{label} [{name}]"),
            source,
            parse_options,
            &convert,
        );
    }
}

/// Pull every `let original = r##"..."##;` literal out of a generated suite file.
fn extract_suite_inputs(src: &str) -> Vec<String> {
    const OPEN: &str = "let original = r##\"";
    const CLOSE: &str = "\"##;";
    let mut out = Vec::new();
    let mut rest = src;
    while let Some(start) = rest.find(OPEN) {
        let body = &rest[start + OPEN.len()..];
        let Some(end) = body.find(CLOSE) else { break };
        out.push(body[..end].to_string());
        rest = &body[end + CLOSE.len()..];
    }
    out
}

#[test]
fn commonmark_spec_examples_are_byte_identical() {
    let examples: Vec<serde_json::Value> =
        serde_json::from_str(SPEC_JSON).expect("failed to parse spec.json");
    assert!(examples.len() > 600, "spec.json looks truncated");
    for ex in &examples {
        let id = ex["example"].as_u64().expect("example field");
        let markdown = ex["markdown"].as_str().expect("markdown field");
        assert_parity_all_options(&format!("spec example {id}"), markdown);
    }
}

#[test]
fn suite_inputs_are_byte_identical() {
    let mut total = 0usize;
    for (name, src) in SUITE_FILES {
        let inputs = extract_suite_inputs(src);
        assert!(
            !inputs.is_empty(),
            "no inputs extracted from suite/{name}.rs"
        );
        for (i, input) in inputs.iter().enumerate() {
            assert_parity_all_options(&format!("suite {name} #{i}"), input);
            total += 1;
        }
    }
    assert!(total > 1000, "expected the full suite corpus, got {total}");
}

#[test]
fn bench_fixtures_are_byte_identical() {
    for (name, source) in FIXTURES {
        assert_parity_all_options(name, source);
    }
}

/// Corners the generated suites don't reach.
const EDGE_CASES: &[(&str, &str)] = &[
    ("empty", ""),
    ("blank lines", "\n\n\n"),
    (
        "footnote reuse",
        "One[^a] two[^a] three[^b].\n\n[^a]: First note.\n\n[^b]: Second note with [^a] nested.\n",
    ),
    (
        "footnote uppercase identifier",
        "Ref[^Alpha].\n\n[^Alpha]: Body.\n",
    ),
    (
        "footnote definition without paragraph",
        "Ref[^x].\n\n[^x]:\n    - list only\n",
    ),
    ("footnote empty definition", "Ref[^x].\n\n[^x]:\n"),
    (
        "orphan footnote reference",
        "No definition here[^missing].\n",
    ),
    (
        "footnote with trailing emphasis",
        "Ref[^e].\n\n[^e]: ends with *emphasis*\n",
    ),
    (
        "hard break then indented text",
        "line one\\\n    indented continuation\n\nline\\\n\tTab start\n",
    ),
    (
        "hard break then emphasis",
        "one\\\n  *two* three\n\nfour\\\n  `code`\n\nfive\\\n  ![alt](img.png)\n",
    ),
    (
        "hard break then link",
        "one\\\n   [text](/url)\n\ntwo\\\n   [ref]\n\n[ref]: /u\n",
    ),
    ("hard break then break", "one\\\n\\\n  two\n"),
    ("hard break then inline math", "one\\\n  $x$ done\n"),
    (
        "hard break then footnote ref",
        "one\\\n  [^f] done\n\n[^f]: note\n",
    ),
    (
        "table alignments",
        "| a | b | c | d |\n|:--|--:|:-:|---|\n| 1 | 2 | 3 | 4 |\n| overflow | x | y | z | extra |\n| short |\n",
    ),
    (
        "table with inline markup",
        "| `code` | **b** |\n|---|---|\n| <b>raw</b> | [l](/u) |\n",
    ),
    ("table header only", "| a | b |\n|---|---|\n"),
    (
        "task list mixed",
        "- [x] done\n- [ ] todo\n- plain\n\n1. [x] ordered done\n2. [ ] ordered todo\n",
    ),
    (
        "loose task list",
        "- [x] done\n\n  second paragraph\n\n- [ ] todo\n",
    ),
    (
        "task list with block first child",
        "- [x]\n  > quote\n\n- [ ]\n",
    ),
    ("empty task item", "- [x]\n- [ ]\n"),
    ("ordered list start", "5. five\n6. six\n\n1. one\n"),
    (
        "nested lists loose and tight",
        "- a\n  - b\n\n    c\n- d\n\n1. x\n\n2. y\n",
    ),
    (
        "definition list",
        "Term\n: Details one\n: Details two\n\nOther\n\n: Loose details\n\n  more\n",
    ),
    (
        "math",
        "$$\nx = 1\n$$\n\nInline $a<b$ and $$block$$ here.\n",
    ),
    (
        "yaml frontmatter",
        "---\ntitle: Hi & <bye>\n---\n\nBody text.\n",
    ),
    (
        "toml frontmatter",
        "+++\ntitle = \"Hi\"\n+++\n\nBody text.\n",
    ),
    ("strikethrough", "~~gone~~ and ~single~ and ~~~triple~~~\n"),
    ("superscript subscript", "a^sup^ b~sub~ c\n"),
    (
        "raw html",
        "<div class=\"x\">\n\n*md*\n\n</div>\n\nInline <em>raw</em> & <script>a<b</script>\n",
    ),
    (
        "reference links and images",
        "[a] [b][c] ![d][c] ![e]\n\n[a]: /a \"A & B\"\n[c]: /c?x=1&y=2\n[e]: /e\n",
    ),
    (
        "unresolved references",
        "[nope] and ![nope] and [text][nope]\n",
    ),
    (
        "url escaping",
        "[a](/f%20o%o?q=1&r=2#f) ![b](\"quoted\".png) [c](<sp ace>)\n",
    ),
    (
        "attribute escaping",
        "[t](/u \"a \\\" b ' c ` d & e\") ![i](/u \"t&t\")\n",
    ),
    (
        "code blocks",
        "```rust meta here\nfn main() { let x = 1 < 2; }\n```\n\n```\n\n```\n\n    indented\n\n```lang&<>\ncode\n```\n",
    ),
    ("empty code block", "```\n```\n"),
    (
        "inline code line endings",
        "a `one\ntwo` b and `x\r\ny` c\n",
    ),
    (
        "blockquote and thematic break",
        "> quoted\n>\n> - list\n\n---\n\n> \n",
    ),
    (
        "headings",
        "# One\n## Two &amp; <b>\n### Three\n#### Four\n##### Five\n###### Six\n",
    ),
    (
        "entities and escapes",
        "&amp; &lt; &#65; \\* \\_ \\\\ &copy; &nbsp;\n",
    ),
    (
        "autolinks",
        "<https://a.b/c?d=1&e=2> www.a.b/c a@b.co <a@b.co>\n",
    ),
    (
        "directives without hname",
        ":::note\nHidden[^d]\n:::\n\n[^d]: def\n\n::leaf\n\nText :inline[x] here.\n",
    ),
    ("wikilinks", "[[Target]] and [[Target|Label]]\n"),
    ("heading attributes", "# Title {#custom-id .cls}\n"),
    (
        "mdx jsx and expressions",
        "import {A} from './a.js'\n\n<Chart values={[1,2]} />\n\nText {1 + 2} more.\n\n<Box>\n\ninner *md*\n\n</Box>\n",
    ),
    (
        "soft wraps with trailing spaces",
        "one   \ntwo\t\nthree\n\nfour \n  five\n",
    ),
    ("multibyte", "é😀 — «quotes» ≤ ≥\n\n- 日本語\n"),
];

#[test]
fn edge_cases_are_byte_identical() {
    for (label, source) in EDGE_CASES {
        assert_parity_all_options(label, source);
    }
}

/// Past the ninth reference the backref id, href, and `{reference}` substitution all grow a digit.
const BACKREF_SOURCES: &[(&str, &str)] = &[
    (
        "two definitions",
        "One[^a] two[^a] three[^b].\n\n[^a]: First.\n\n[^b]: Second.\n",
    ),
    (
        "twelve references",
        "a[^r] b[^r] c[^r] d[^r] e[^r] f[^r] g[^r] h[^r] i[^r] j[^r] k[^r] l[^r].\n\n[^r]: Many.\n",
    ),
    (
        "no paragraph to hang backrefs on",
        "x[^n] y[^n] z[^n].\n\n[^n]:\n    - list only\n",
    ),
    ("empty definition", "p[^e] q[^e].\n\n[^e]:\n"),
];

#[test]
fn custom_convert_options_are_byte_identical() {
    let cases: Vec<(&str, ConvertOptions)> = vec![
        (
            "custom label",
            ConvertOptions {
                footnote_label: "Notes & <refs>".to_string(),
                ..Default::default()
            },
        ),
        (
            "custom templates",
            ConvertOptions {
                footnote_back_content: Backref::Template("↩ back {reference}".to_string()),
                footnote_back_label: Backref::Template("Go to \"{reference}\"".to_string()),
                ..Default::default()
            },
        ),
        (
            "callbacks",
            ConvertOptions {
                footnote_back_content: Backref::Callback(Box::new(|n, k| format!("<{n}:{k}>"))),
                footnote_back_label: Backref::Callback(Box::new(|n, k| format!("label {n}/{k}"))),
                ..Default::default()
            },
        ),
    ];
    for (label, convert) in &cases {
        for (source_label, source) in BACKREF_SOURCES {
            assert_parity(
                &format!("{label} / {source_label}"),
                source,
                satteri_pulldown_cmark::DEFAULT_OPTIONS,
                convert,
            );
        }
    }
}

#[test]
fn node_data_falls_back_to_two_stage() {
    let (mut arena, _) =
        satteri_pulldown_cmark::parse("Hello\n", satteri_pulldown_cmark::DEFAULT_OPTIONS);
    let para_id = (0..arena.len() as u32)
        .find(|&id| {
            arena.get_node(id).node_type == satteri_ast::mdast::MdastNodeType::Paragraph as u8
        })
        .expect("paragraph");
    arena.set_node_data(para_id, br#"{"hName":"section"}"#.to_vec());
    assert!(
        try_mdast_to_html_fused(&arena, &ConvertOptions::default()).is_none(),
        "fused path must decline when a node carries data"
    );
    assert!(
        satteri_ast::mdast_to_html(&arena).contains("<section>Hello</section>"),
        "two-stage fallback lost the hName override"
    );
}

fn text_node(arena: &mut Arena<Mdast>, value: &str) -> u32 {
    let id = arena.alloc_node(MdastNodeType::Text as u8);
    let text = arena.alloc_string(value);
    arena.set_type_data(id, &text.as_bytes());
    id
}

fn list_item(arena: &mut Arena<Mdast>, data: &[u8], value: &str) -> u32 {
    let id = arena.alloc_node(MdastNodeType::ListItem as u8);
    arena.set_type_data(id, data);
    let para = arena.alloc_node(MdastNodeType::Paragraph as u8);
    let child = text_node(arena, value);
    arena.set_children(para, &[child]);
    arena.set_children(id, &[para]);
    id
}

fn tight_item() -> [u8; 2] {
    ListItemData {
        checked: 2,
        spread: false,
    }
    .to_bytes()
}

fn both_paths(label: &str, arena: &Arena<Mdast>) -> String {
    let convert = ConvertOptions::default();
    let expected = two_stage(arena, &convert);
    let fused = try_mdast_to_html_fused(arena, &convert)
        .unwrap_or_else(|| panic!("{label}: fused path declined without h data"));
    assert_eq!(expected, fused, "{label}");
    expected
}

/// Plugins hand-build list nodes, so a missing or short `type_data` blob must render, not panic.
#[test]
fn undersized_list_type_data_renders_on_both_paths() {
    let mut arena: Arena<Mdast> = Arena::new(String::new());
    let root = arena.alloc_node(MdastNodeType::Root as u8);
    let list = arena.alloc_node(MdastNodeType::List as u8);
    let item = list_item(&mut arena, &tight_item(), "a");
    arena.set_children(list, &[item]);
    arena.set_children(root, &[list]);
    assert_eq!(
        both_paths("list without type data", &arena),
        "<ul>\n<li>a</li>\n</ul>\n"
    );

    let mut arena: Arena<Mdast> = Arena::new(String::new());
    let root = arena.alloc_node(MdastNodeType::Root as u8);
    let list = arena.alloc_node(MdastNodeType::List as u8);
    arena.set_type_data(list, &[1u8]);
    let item = list_item(&mut arena, &[7u8], "b");
    arena.set_children(list, &[item]);
    arena.set_children(root, &[list]);
    assert_eq!(
        both_paths("one-byte list and item data", &arena),
        "<ul>\n<li>b</li>\n</ul>\n"
    );
}

/// `parent` is a raw id: a reparented item can land under a non-list, and a root item has none at all.
#[test]
fn list_item_without_a_list_parent_renders_on_both_paths() {
    let mut arena: Arena<Mdast> = Arena::new(String::new());
    let root = arena.alloc_node(MdastNodeType::Root as u8);
    let outer = arena.alloc_node(MdastNodeType::ListItem as u8);
    arena.set_type_data(outer, &tight_item());
    let inner = list_item(&mut arena, &tight_item(), "nested");
    arena.set_children(outer, &[inner]);
    arena.set_children(root, &[outer]);
    assert_eq!(
        both_paths("item under an item", &arena),
        "<li>\n<li>nested</li>\n</li>\n"
    );

    let mut arena: Arena<Mdast> = Arena::new(String::new());
    let orphan = list_item(&mut arena, &tight_item(), "orphan");
    assert_eq!(
        orphan, 0,
        "the orphan has to be the node the walk starts at"
    );
    assert_eq!(
        both_paths("item with no parent", &arena),
        "<li>orphan</li>\n"
    );

    let mut arena: Arena<Mdast> = Arena::new(String::new());
    let root = arena.alloc_node(MdastNodeType::Root as u8);
    let para = arena.alloc_node(MdastNodeType::Paragraph as u8);
    let item = list_item(&mut arena, &tight_item(), "c");
    arena.set_children(para, &[item]);
    arena.set_children(root, &[para]);
    both_paths("item under a paragraph", &arena);
}

/// Only h-shaped `data` needs the HAST pipeline; plugin-private blobs stay on the fused path.
#[test]
fn unrelated_node_data_stays_fused() {
    let (mut arena, _) =
        satteri_pulldown_cmark::parse("Hello\n", satteri_pulldown_cmark::DEFAULT_OPTIONS);
    arena.set_node_data(0, br#"{"unrelated":1}"#.to_vec());
    let convert = ConvertOptions::default();
    assert_eq!(
        try_mdast_to_html_fused(&arena, &convert),
        Some(two_stage(&arena, &convert))
    );
}
