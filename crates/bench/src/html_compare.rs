fn main() {
    let sources = [
        "# Hello\n\nWorld",
        "## Heading 2\n\n### Heading 3",
        "Some **bold** and *italic* text",
        "[link](https://example.com)\n\n![alt](img.png)",
        "- item 1\n- item 2\n- item 3",
        "1. first\n2. second\n3. third",
        "> blockquote\n>\n> continued",
        "```rust\nfn main() {}\n```",
        "inline `code` here",
        "---\n\nparagraph",
        "| a | b |\n|---|---|\n| 1 | 2 |",
        "- [x] done\n- [ ] todo",
        "text with a [link](url \"title\") here",
        "![image](src \"title\")",
        "***bold and italic***",
        "~~deleted~~",
        "just text",
        "# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6",
    ];

    let opts = satteri_pulldown_cmark::DEFAULT_OPTIONS;
    for (i, source) in sources.iter().enumerate() {
        let (arena, _) = satteri_pulldown_cmark::parse(source, opts);
        let html = satteri_ast::mdast_to_html(&arena);
        println!("Source #{}: {} bytes of HTML", i, html.len());
    }
}
