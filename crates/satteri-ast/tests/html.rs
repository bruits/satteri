//! HTML output tests: verify that mdast_to_html produces correct HTML
//! for all supported Markdown constructs.

use satteri_ast::mdast_to_html;

fn html(md: &str) -> String {
    let (arena, _errors) =
        satteri_pulldown_cmark::parse(md, satteri_pulldown_cmark::DEFAULT_OPTIONS);
    mdast_to_html(&arena)
}

fn html_with(md: &str, opts: satteri_pulldown_cmark::Options) -> String {
    let (arena, _errors) = satteri_pulldown_cmark::parse(md, opts);
    mdast_to_html(&arena)
}

fn smart(md: &str) -> String {
    let opts = satteri_pulldown_cmark::DEFAULT_OPTIONS
        | satteri_pulldown_cmark::Options::ENABLE_SMART_PUNCTUATION;
    html_with(md, opts)
}

#[test]
fn heading_h1() {
    assert_eq!(html("# Heading 1"), "<h1>Heading 1</h1>\n");
}

#[test]
fn heading_h2() {
    assert_eq!(html("## Heading 2"), "<h2>Heading 2</h2>\n");
}

#[test]
fn heading_h6() {
    assert_eq!(html("###### Heading 6"), "<h6>Heading 6</h6>\n");
}

#[test]
fn paragraph() {
    assert_eq!(html("A simple paragraph."), "<p>A simple paragraph.</p>\n");
}

#[test]
fn emphasis() {
    assert_eq!(
        html("Some *emphasized* text."),
        "<p>Some <em>emphasized</em> text.</p>\n"
    );
}

#[test]
fn strong() {
    assert_eq!(
        html("Some **strong** text."),
        "<p>Some <strong>strong</strong> text.</p>\n"
    );
}

#[test]
fn inline_code() {
    assert_eq!(
        html("Use `inline code` here."),
        "<p>Use <code>inline code</code> here.</p>\n"
    );
}

#[test]
fn link_without_title() {
    assert_eq!(
        html("[example](https://example.com)"),
        "<p><a href=\"https://example.com\">example</a></p>\n"
    );
}

#[test]
fn link_with_title() {
    assert_eq!(
        html("[example](https://example.com \"Example Title\")"),
        "<p><a href=\"https://example.com\" title=\"Example Title\">example</a></p>\n"
    );
}

#[test]
fn image_without_title() {
    assert_eq!(
        html("![alt text](image.png)"),
        "<p><img src=\"image.png\" alt=\"alt text\"></p>\n"
    );
}

#[test]
fn fenced_code_block() {
    assert_eq!(
        html("```rust\nfn main() {}\n```"),
        "<pre><code class=\"language-rust\">fn main() {}\n</code></pre>\n"
    );
}

#[test]
fn code_block_no_language() {
    assert_eq!(
        html("```\nplain code\n```"),
        "<pre><code>plain code\n</code></pre>\n"
    );
}

#[test]
fn blockquote() {
    assert_eq!(
        html("> This is a blockquote."),
        "<blockquote>\n<p>This is a blockquote.</p>\n</blockquote>\n"
    );
}

#[test]
fn unordered_list() {
    assert_eq!(
        html("- Item 1\n- Item 2\n- Item 3"),
        "<ul>\n<li>Item 1</li>\n<li>Item 2</li>\n<li>Item 3</li>\n</ul>\n"
    );
}

#[test]
fn ordered_list() {
    assert_eq!(
        html("1. First\n2. Second\n3. Third"),
        "<ol>\n<li>First</li>\n<li>Second</li>\n<li>Third</li>\n</ol>\n"
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
    assert_eq!(html("---"), "<hr>\n");
}

#[test]
fn hard_line_break() {
    assert_eq!(
        html("Line one  \nLine two"),
        "<p>Line one<br>\nLine two</p>\n"
    );
}

#[test]
fn text_escaping() {
    assert_eq!(html("a < b & c > d"), "<p>a &lt; b &amp; c &gt; d</p>\n");
}

#[test]
fn multiple_paragraphs() {
    assert_eq!(
        html("First paragraph.\n\nSecond paragraph."),
        "<p>First paragraph.</p>\n<p>Second paragraph.</p>\n"
    );
}

// Smart punctuation (arena pipeline: parse → mdast → hast → HTML)

#[test]
fn smart_punctuation() {
    // Ellipsis, dashes, and curly quotes through the full arena path
    assert_eq!(
        smart("\"Hello,\" she said---it's an em-dash, an en--dash, and an ellipsis..."),
        "<p>\u{201c}Hello,\u{201d} she said\u{2014}it\u{2019}s an em-dash, an en\u{2013}dash, and an ellipsis\u{2026}</p>\n"
    );
}

// Task lists: mdast→hast adds `contains-task-list` on the list and
// `task-list-item` on each task `<li>`, plus an <input type=checkbox>.

#[test]
fn task_list_mixed() {
    assert_eq!(
        html("- [x] done\n- [ ] todo"),
        "<ul class=\"contains-task-list\">\n\
         <li class=\"task-list-item\"><input type=\"checkbox\" checked disabled> done</li>\n\
         <li class=\"task-list-item\"><input type=\"checkbox\" disabled> todo</li>\n\
         </ul>\n"
    );
}

#[test]
fn plain_list_has_no_task_class() {
    // Plain list: no contains-task-list / task-list-item classes, no <input>.
    assert_eq!(html("- a\n- b"), "<ul>\n<li>a</li>\n<li>b</li>\n</ul>\n");
}

// Code blocks: value always ends with a trailing newline, even when the
// source didn't have one before the closing fence.

#[test]
fn code_block_appends_trailing_newline_when_missing() {
    // Closing fence on the same line leaves pulldown-cmark with an
    // unterminated block whose value lacks a trailing newline. The
    // mdast→hast pass normalises it so the <code> text still ends with \n.
    assert_eq!(
        html("```js\nconsole.log(1)```"),
        "<pre><code class=\"language-js\">console.log(1)```\n</code></pre>\n"
    );
}

#[test]
fn code_block_preserves_single_trailing_newline() {
    // Source already has exactly one trailing newline — don't double it
    assert_eq!(
        html("```js\nconsole.log(1)\n```"),
        "<pre><code class=\"language-js\">console.log(1)\n</code></pre>\n"
    );
}

// Table column alignment: mdast→hast emits `style="text-align: ..."` on
// each cell (both <th> and <td>) based on the delimiter row.

#[test]
fn table_column_alignments() {
    assert_eq!(
        html("| a | b | c |\n| :--- | :---: | ---: |\n| 1 | 2 | 3 |\n"),
        "<table>\n\
         <thead>\n\
         <tr>\n\
         <th style=\"text-align: left\">a</th>\n\
         <th style=\"text-align: center\">b</th>\n\
         <th style=\"text-align: right\">c</th>\n\
         </tr>\n\
         </thead>\n\
         <tbody>\n\
         <tr>\n\
         <td style=\"text-align: left\">1</td>\n\
         <td style=\"text-align: center\">2</td>\n\
         <td style=\"text-align: right\">3</td>\n\
         </tr>\n\
         </tbody>\n\
         </table>\n"
    );
}

#[test]
fn table_no_alignment_omits_style() {
    assert_eq!(
        html("| a | b |\n|---|---|\n| 1 | 2 |"),
        "<table>\n\
         <thead>\n\
         <tr>\n\
         <th>a</th>\n\
         <th>b</th>\n\
         </tr>\n\
         </thead>\n\
         <tbody>\n\
         <tr>\n\
         <td>1</td>\n\
         <td>2</td>\n\
         </tr>\n\
         </tbody>\n\
         </table>\n"
    );
}

// Footnotes: pulldown-cmark-style output. References become
// `<sup class="footnote-reference"><a href="#id">N</a></sup>`; definitions
// become `<div class="footnote-definition" id="id"><sup class="footnote-definition-label">N</sup>…</div>`.
// Numbers are assigned in source order across both references and definitions.

#[test]
fn footnote_single_reference_and_definition() {
    assert_eq!(
        html("Here[^1].\n\n[^1]: the note"),
        concat!(
            "<p>Here",
            "<sup><a href=\"#user-content-fn-1\" id=\"user-content-fnref-1\"",
            " data-footnote-ref aria-describedby=\"footnote-label\">1</a></sup>",
            ".</p>\n",
            "<section data-footnotes class=\"footnotes\">",
            "<h2 class=\"sr-only\" id=\"footnote-label\">Footnotes</h2>\n",
            "<ol>\n",
            "<li id=\"user-content-fn-1\">\n",
            "<p>the note ",
            "<a href=\"#user-content-fnref-1\" data-footnote-backref=\"\"",
            " aria-label=\"Back to reference 1\" class=\"data-footnote-backref\">",
            "\u{21a9}</a></p>\n",
            "</li>\n",
            "</ol>\n",
            "</section>\n",
        )
    );
}

#[test]
fn footnote_numbers_follow_source_order() {
    // `b` is referenced before `a` in the body, so numbering becomes b=1, a=2
    // — and because the `<section>` iterates `footnoteOrder`, the list items
    // also appear in reference order (b then a) rather than definition order.
    assert_eq!(
        html("See[^b] then[^a].\n\n[^a]: first def\n[^b]: second def"),
        concat!(
            "<p>See",
            "<sup><a href=\"#user-content-fn-b\" id=\"user-content-fnref-b\"",
            " data-footnote-ref aria-describedby=\"footnote-label\">1</a></sup>",
            " then",
            "<sup><a href=\"#user-content-fn-a\" id=\"user-content-fnref-a\"",
            " data-footnote-ref aria-describedby=\"footnote-label\">2</a></sup>",
            ".</p>\n",
            "<section data-footnotes class=\"footnotes\">",
            "<h2 class=\"sr-only\" id=\"footnote-label\">Footnotes</h2>\n",
            "<ol>\n",
            "<li id=\"user-content-fn-b\">\n",
            "<p>second def ",
            "<a href=\"#user-content-fnref-b\" data-footnote-backref=\"\"",
            " aria-label=\"Back to reference 1\" class=\"data-footnote-backref\">",
            "\u{21a9}</a></p>\n",
            "</li>\n",
            "<li id=\"user-content-fn-a\">\n",
            "<p>first def ",
            "<a href=\"#user-content-fnref-a\" data-footnote-backref=\"\"",
            " aria-label=\"Back to reference 2\" class=\"data-footnote-backref\">",
            "\u{21a9}</a></p>\n",
            "</li>\n",
            "</ol>\n",
            "</section>\n",
        )
    );
}
