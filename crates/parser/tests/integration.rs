//! Integration tests: new parser → HAST → HTML, and new parser → MDX compile.

use parser::{parse, ParseOptions};

#[test]
fn full_pipeline_heading_to_html() {
    let (arena, _) = parse("# Hello world\n", &ParseOptions::default());
    let html = tryckeri_hast::mdast_to_html(&arena);
    assert!(html.contains("<h1>"), "expected <h1>, got: {html}");
    assert!(html.contains("Hello world"), "expected text, got: {html}");
}

#[test]
fn full_pipeline_paragraph_to_html() {
    let (arena, _) = parse("hello **world**\n", &ParseOptions::default());
    let html = tryckeri_hast::mdast_to_html(&arena);
    assert!(html.contains("<p>"), "expected <p>, got: {html}");
    assert!(html.contains("<strong>"), "expected <strong>, got: {html}");
}

#[test]
fn full_pipeline_list_to_html() {
    let (arena, _) = parse("- a\n- b\n- c\n", &ParseOptions::default());
    let html = tryckeri_hast::mdast_to_html(&arena);
    assert!(html.contains("<ul>"), "expected <ul>, got: {html}");
    assert!(html.contains("<li>"), "expected <li>, got: {html}");
}

#[test]
fn full_pipeline_code_block_to_html() {
    let (arena, _) = parse("```rust\nfn main() {}\n```\n", &ParseOptions::default());
    let html = tryckeri_hast::mdast_to_html(&arena);
    assert!(html.contains("<pre>"), "expected <pre>, got: {html}");
    assert!(html.contains("<code"), "expected <code>, got: {html}");
    assert!(
        html.contains("fn main"),
        "expected code content, got: {html}"
    );
}

#[test]
fn full_pipeline_link_to_html() {
    let (arena, _) = parse("[click](https://example.com)\n", &ParseOptions::default());
    let html = tryckeri_hast::mdast_to_html(&arena);
    assert!(
        html.contains("href=\"https://example.com\""),
        "expected href, got: {html}"
    );
    assert!(html.contains("click"), "expected link text, got: {html}");
}

#[test]
fn full_pipeline_image_to_html() {
    let (arena, _) = parse("![alt](img.png)\n", &ParseOptions::default());
    let html = tryckeri_hast::mdast_to_html(&arena);
    assert!(
        html.contains("src=\"img.png\""),
        "expected src, got: {html}"
    );
}

#[test]
fn full_pipeline_blockquote_to_html() {
    let (arena, _) = parse("> quoted text\n", &ParseOptions::default());
    let html = tryckeri_hast::mdast_to_html(&arena);
    assert!(
        html.contains("<blockquote>"),
        "expected blockquote, got: {html}"
    );
}

#[test]
fn full_pipeline_html_block_to_html() {
    let (arena, _) = parse(
        "<div>raw html</div>\n\nparagraph\n",
        &ParseOptions::default(),
    );
    let html = tryckeri_hast::mdast_to_html(&arena);
    assert!(
        html.contains("<div>raw html</div>"),
        "expected raw html, got: {html}"
    );
}

#[test]
fn full_pipeline_inline_code_to_html() {
    let (arena, _) = parse("use `code` here\n", &ParseOptions::default());
    let html = tryckeri_hast::mdast_to_html(&arena);
    assert!(html.contains("<code>"), "expected inline code, got: {html}");
}

#[test]
fn full_pipeline_emphasis_to_html() {
    let (arena, _) = parse("*em* and **strong**\n", &ParseOptions::default());
    let html = tryckeri_hast::mdast_to_html(&arena);
    assert!(html.contains("<em>"), "expected em, got: {html}");
    assert!(html.contains("<strong>"), "expected strong, got: {html}");
}

#[test]
fn full_pipeline_thematic_break_to_html() {
    let (arena, _) = parse("---\n", &ParseOptions::default());
    let html = tryckeri_hast::mdast_to_html(&arena);
    assert!(html.contains("<hr"), "expected hr, got: {html}");
}

#[test]
fn full_pipeline_ordered_list_to_html() {
    let (arena, _) = parse("1. first\n2. second\n", &ParseOptions::default());
    let html = tryckeri_hast::mdast_to_html(&arena);
    assert!(html.contains("<ol>"), "expected <ol>, got: {html}");
}

#[test]
fn full_pipeline_table_to_html() {
    let (arena, _) = parse(
        "| a | b |\n|---|---|\n| 1 | 2 |\n",
        &ParseOptions::default(),
    );
    let html = tryckeri_hast::mdast_to_html(&arena);
    assert!(html.contains("<table>"), "expected <table>, got: {html}");
}

#[test]
fn full_pipeline_buffer_roundtrip_then_hast() {
    let (arena, _) = parse("# Hello\n\nworld\n", &ParseOptions::default());
    let buf = arena.to_raw_buffer();
    // Use the buffer path (simulating the NAPI path).
    let html_buf = tryckeri_hast::mdast_to_hast_buffer(&buf).unwrap();
    let html = tryckeri_hast::hast_buffer_to_html(&html_buf).unwrap();
    assert!(
        html.contains("<h1>"),
        "expected heading via buffer path, got: {html}"
    );
}

#[test]
fn full_pipeline_complex_document() {
    let md = r#"# Title

Some **bold** and *italic* text with `code`.

- item 1
- item 2

> A blockquote

```js
console.log("hello");
```

[link](https://example.com)

---
"#;
    let (arena, _) = parse(md, &ParseOptions::default());
    let html = tryckeri_hast::mdast_to_html(&arena);
    assert!(html.contains("<h1>"), "heading: {html}");
    assert!(html.contains("<strong>"), "bold: {html}");
    assert!(html.contains("<em>"), "italic: {html}");
    assert!(html.contains("<code>"), "code: {html}");
    assert!(html.contains("<ul>"), "list: {html}");
    assert!(html.contains("<blockquote>"), "blockquote: {html}");
    assert!(html.contains("<pre>"), "code block: {html}");
    assert!(html.contains("href="), "link: {html}");
    assert!(html.contains("<hr"), "hr: {html}");
}

// ---------------------------------------------------------------------------
// JSX tag pairing
// ---------------------------------------------------------------------------

#[test]
fn jsx_inline_with_children() {
    // `a <b>c</b> d` — inline JSX with text children.
    let (arena, _) = parse("a <b>c</b> d", &ParseOptions::mdx());
    let jsx = (0..arena.len() as u32)
        .map(|i| arena.get_node(i))
        .find(|n| n.node_type == tryckeri_mdast::MdastNodeType::MdxJsxTextElement as u8)
        .expect("should have MdxJsxTextElement");
    // The JSX element should have children (the text "c").
    assert!(
        jsx.children_count > 0,
        "JSX element should have children, got {}",
        jsx.children_count
    );
}

#[test]
fn jsx_fragment_with_children() {
    let (arena, _) = parse("<>a</>", &ParseOptions::mdx());
    let jsx = (0..arena.len() as u32)
        .map(|i| arena.get_node(i))
        .find(|n| n.node_type == tryckeri_mdast::MdastNodeType::MdxJsxTextElement as u8)
        .expect("should have MdxJsxTextElement for fragment");
    assert!(
        jsx.children_count > 0,
        "Fragment should have children, got {}",
        jsx.children_count
    );
}

#[test]
fn jsx_self_closing_no_children() {
    let (arena, _) = parse("<Component />", &ParseOptions::mdx());
    let jsx = (0..arena.len() as u32)
        .map(|i| arena.get_node(i))
        .find(|n| {
            n.node_type == tryckeri_mdast::MdastNodeType::MdxJsxFlowElement as u8
                || n.node_type == tryckeri_mdast::MdastNodeType::MdxJsxTextElement as u8
        })
        .expect("should have MDX JSX");
    assert_eq!(
        jsx.children_count, 0,
        "Self-closing should have no children, got {}",
        jsx.children_count
    );
}

#[test]
fn jsx_flow_with_children() {
    // Multi-line JSX with content.
    let (arena, _) = parse("<x>\n  b\n</x>", &ParseOptions::mdx());
    let jsx = (0..arena.len() as u32)
        .map(|i| arena.get_node(i))
        .find(|n| n.node_type == tryckeri_mdast::MdastNodeType::MdxJsxFlowElement as u8)
        .expect("should have MdxJsxFlowElement");
    assert!(
        jsx.children_count > 0,
        "Flow JSX element should have children, got {}",
        jsx.children_count
    );
}

#[test]
fn jsx_flow_with_children_html() {
    let (arena, _) = parse("<h1>asd</h1>\n# qwe", &ParseOptions::mdx());
    let html = tryckeri_hast::mdast_to_html(&arena);
    // MDX JSX elements have no HTML representation — they're only used in the
    // MDX→JS compilation path. The markdown heading should still be present.
    assert!(
        !html.contains("asd"),
        "MDX JSX should not render as HTML, got: {html}"
    );
    assert!(html.contains("qwe"), "expected qwe in: {html}");
}

// ---------------------------------------------------------------------------
// MDX pipeline
// ---------------------------------------------------------------------------

#[test]
fn mdx_compile_via_new_parser() {
    let md = r#"import {Chart} from './chart.js'

# Hello

<Chart values={[1, 2, 3]} />

Some {expression} here.
"#;
    let (arena, _) = parse(md, &ParseOptions::mdx());
    // Verify the arena has MDX nodes.
    let has_esm = (0..arena.len() as u32)
        .any(|i| arena.get_node(i).node_type == tryckeri_mdast::MdastNodeType::MdxjsEsm as u8);
    assert!(has_esm, "should have ESM node");

    let has_jsx = (0..arena.len() as u32).any(|i| {
        arena.get_node(i).node_type == tryckeri_mdast::MdastNodeType::MdxJsxFlowElement as u8
    });
    assert!(has_jsx, "should have JSX flow node");
}

#[test]
fn task_list_to_html() {
    let (arena, _) = parse("- [ ] unchecked\n- [x] checked\n", &ParseOptions::default());
    let html = tryckeri_hast::mdast_to_html(&arena);
    eprintln!("HTML output: {html}");
    assert!(
        html.contains("type=\"checkbox\""),
        "expected checkbox input, got: {html}"
    );
    assert!(
        html.contains("checked"),
        "expected checked attribute, got: {html}"
    );
}

#[test]
fn task_list_loose_to_html() {
    let (arena, _) = parse(
        "- [ ] unchecked\n\n- [x] checked\n",
        &ParseOptions::default(),
    );
    let html = tryckeri_hast::mdast_to_html(&arena);
    eprintln!("HTML output (loose): {html}");
    assert!(
        html.contains("type=\"checkbox\""),
        "expected checkbox input, got: {html}"
    );
}
