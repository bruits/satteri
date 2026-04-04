//! HTML output tests: verify that mdast_to_html produces correct HTML
//! for all supported Markdown constructs.

use tryckeri_hast::mdast_to_html;

fn html(md: &str) -> String {
    let (arena, _errors) = tryckeri_pulldown_cmark::parse(md, tryckeri_pulldown_cmark::DEFAULT_OPTIONS);
    mdast_to_html(&arena)
}

#[test]
fn heading_h1() {
    assert_eq!(html("# Heading 1"), "<h1>Heading 1</h1>");
}

#[test]
fn heading_h2() {
    assert_eq!(html("## Heading 2"), "<h2>Heading 2</h2>");
}

#[test]
fn heading_h6() {
    assert_eq!(html("###### Heading 6"), "<h6>Heading 6</h6>");
}

#[test]
fn paragraph() {
    assert_eq!(html("A simple paragraph."), "<p>A simple paragraph.</p>");
}

#[test]
fn emphasis() {
    assert_eq!(html("Some *emphasized* text."), "<p>Some <em>emphasized</em> text.</p>");
}

#[test]
fn strong() {
    assert_eq!(html("Some **strong** text."), "<p>Some <strong>strong</strong> text.</p>");
}

#[test]
fn inline_code() {
    assert_eq!(html("Use `inline code` here."), "<p>Use <code>inline code</code> here.</p>");
}

#[test]
fn link_without_title() {
    assert_eq!(
        html("[example](https://example.com)"),
        "<p><a href=\"https://example.com\">example</a></p>"
    );
}

#[test]
fn link_with_title() {
    assert_eq!(
        html("[example](https://example.com \"Example Title\")"),
        "<p><a href=\"https://example.com\" title=\"Example Title\">example</a></p>"
    );
}

#[test]
fn image_without_title() {
    assert_eq!(
        html("![alt text](image.png)"),
        "<p><img src=\"image.png\" alt=\"alt text\"></p>"
    );
}

#[test]
fn fenced_code_block() {
    assert_eq!(
        html("```rust\nfn main() {}\n```"),
        "<pre><code class=\"language-rust\">fn main() {}\n</code></pre>"
    );
}

#[test]
fn code_block_no_language() {
    assert_eq!(
        html("```\nplain code\n```"),
        "<pre><code>plain code\n</code></pre>"
    );
}

#[test]
fn blockquote() {
    assert_eq!(
        html("> This is a blockquote."),
        "<blockquote><p>This is a blockquote.</p></blockquote>"
    );
}

#[test]
fn unordered_list() {
    assert_eq!(
        html("- Item 1\n- Item 2\n- Item 3"),
        "<ul><li>Item 1</li><li>Item 2</li><li>Item 3</li></ul>"
    );
}

#[test]
fn ordered_list() {
    assert_eq!(
        html("1. First\n2. Second\n3. Third"),
        "<ol><li>First</li><li>Second</li><li>Third</li></ol>"
    );
}

#[test]
fn table() {
    let result = html("| A | B |\n|---|---|\n| 1 | 2 |");
    assert!(result.contains("<table>"));
    assert!(result.contains("<th>A</th>"));
    assert!(result.contains("<td>1</td>"));
}

#[test]
fn thematic_break() {
    assert_eq!(html("---"), "<hr>");
}

#[test]
fn hard_line_break() {
    assert_eq!(
        html("Line one  \nLine two"),
        "<p>Line one<br>Line two</p>"
    );
}

#[test]
fn text_escaping() {
    assert_eq!(
        html("a < b & c > d"),
        "<p>a &lt; b &amp; c &gt; d</p>"
    );
}

#[test]
fn multiple_paragraphs() {
    let result = html("First paragraph.\n\nSecond paragraph.");
    assert!(result.contains("<p>First paragraph.</p>"));
    assert!(result.contains("<p>Second paragraph.</p>"));
}
