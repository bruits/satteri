//! Integration tests for the MDAST→HAST binary pipeline.

use mdast_arena::raw_buffer::BUFFER_MAGIC;
use tryckeri_hast::{hast_buffer_to_html, mdast_to_hast_buffer};

fn parse_to_mdast_buf(md: &str) -> Vec<u8> {
    let (arena, _) = parser::parse(md, &parser::ParseOptions::default());
    arena.to_raw_buffer()
}

#[test]
fn hast_buffer_has_correct_magic() {
    let mdast_buf = parse_to_mdast_buf("# Hello");
    let hast_buf = mdast_to_hast_buffer(&mdast_buf).expect("conversion failed");
    assert_eq!(
        &hast_buf[..4],
        &BUFFER_MAGIC,
        "HAST buffer must start with MDAR magic"
    );
}

#[test]
fn heading_produces_h1() {
    let mdast_buf = parse_to_mdast_buf("# Hello World");
    let hast_buf = mdast_to_hast_buffer(&mdast_buf).expect("conversion failed");
    let html = hast_buffer_to_html(&hast_buf).expect("html failed");
    assert!(html.contains("<h1>"), "expected <h1> in: {html}");
    assert!(html.contains("Hello World"), "expected text in: {html}");
    assert!(html.contains("</h1>"), "expected </h1> in: {html}");
}

#[test]
fn paragraph_produces_p() {
    let mdast_buf = parse_to_mdast_buf("Hello, world!");
    let hast_buf = mdast_to_hast_buffer(&mdast_buf).expect("conversion failed");
    let html = hast_buffer_to_html(&hast_buf).expect("html failed");
    assert!(html.contains("<p>"), "expected <p> in: {html}");
    assert!(html.contains("Hello, world!"), "expected text in: {html}");
    assert!(html.contains("</p>"), "expected </p> in: {html}");
}

#[test]
fn code_block_produces_pre_code() {
    let mdast_buf = parse_to_mdast_buf("```rust\nfn main() {}\n```");
    let hast_buf = mdast_to_hast_buffer(&mdast_buf).expect("conversion failed");
    let html = hast_buffer_to_html(&hast_buf).expect("html failed");
    assert!(html.contains("<pre>"), "expected <pre> in: {html}");
    assert!(
        html.contains(r#"class="language-rust""#),
        "expected language class in: {html}"
    );
    assert!(
        html.contains("fn main()"),
        "expected code content in: {html}"
    );
}

#[test]
fn link_produces_anchor() {
    let mdast_buf = parse_to_mdast_buf("[example](https://example.com)");
    let hast_buf = mdast_to_hast_buffer(&mdast_buf).expect("conversion failed");
    let html = hast_buffer_to_html(&hast_buf).expect("html failed");
    assert!(
        html.contains(r#"href="https://example.com""#),
        "expected href in: {html}"
    );
    assert!(html.contains("example"), "expected text in: {html}");
}

#[test]
fn image_produces_img() {
    let mdast_buf = parse_to_mdast_buf("![alt text](https://example.com/img.png)");
    let hast_buf = mdast_to_hast_buffer(&mdast_buf).expect("conversion failed");
    let html = hast_buffer_to_html(&hast_buf).expect("html failed");
    assert!(html.contains("<img"), "expected <img in: {html}");
    assert!(
        html.contains(r#"src="https://example.com/img.png""#),
        "expected src in: {html}"
    );
    assert!(
        html.contains(r#"alt="alt text""#),
        "expected alt in: {html}"
    );
}

#[test]
fn text_is_escaped() {
    let mdast_buf = parse_to_mdast_buf("a < b & c > d");
    let hast_buf = mdast_to_hast_buffer(&mdast_buf).expect("conversion failed");
    let html = hast_buffer_to_html(&hast_buf).expect("html failed");
    assert!(html.contains("&lt;"), "expected &lt; in: {html}");
    assert!(html.contains("&amp;"), "expected &amp; in: {html}");
    assert!(html.contains("&gt;"), "expected &gt; in: {html}");
}

#[test]
fn thematic_break_is_void() {
    let mdast_buf = parse_to_mdast_buf("---");
    let hast_buf = mdast_to_hast_buffer(&mdast_buf).expect("conversion failed");
    let html = hast_buffer_to_html(&hast_buf).expect("html failed");
    assert!(html.contains("<hr>"), "expected <hr> in: {html}");
    assert!(
        !html.contains("</hr>"),
        "hr should be void, no </hr> in: {html}"
    );
}

#[test]
fn ordered_list_with_start() {
    let mdast_buf = parse_to_mdast_buf("3. first\n4. second");
    let hast_buf = mdast_to_hast_buffer(&mdast_buf).expect("conversion failed");
    let html = hast_buffer_to_html(&hast_buf).expect("html failed");
    assert!(html.contains("<ol"), "expected <ol in: {html}");
    assert!(
        html.contains(r#"start="3""#),
        "expected start attr in: {html}"
    );
}

#[test]
fn inline_emphasis_and_strong() {
    let mdast_buf = parse_to_mdast_buf("*em* and **strong**");
    let hast_buf = mdast_to_hast_buffer(&mdast_buf).expect("conversion failed");
    let html = hast_buffer_to_html(&hast_buf).expect("html failed");
    assert!(html.contains("<em>"), "expected <em> in: {html}");
    assert!(html.contains("<strong>"), "expected <strong> in: {html}");
}

#[test]
fn table_structure() {
    let md = "| a | b |\n|---|---|\n| 1 | 2 |";
    let mdast_buf = parse_to_mdast_buf(md);
    let hast_buf = mdast_to_hast_buffer(&mdast_buf).expect("conversion failed");
    let html = hast_buffer_to_html(&hast_buf).expect("html failed");
    assert!(html.contains("<table>"), "expected <table> in: {html}");
    assert!(html.contains("<thead>"), "expected <thead> in: {html}");
    assert!(html.contains("<tbody>"), "expected <tbody> in: {html}");
    assert!(html.contains("<th>"), "expected <th> in: {html}");
    assert!(html.contains("<td>"), "expected <td> in: {html}");
}
