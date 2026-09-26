use satteri_ast::mdast_to_html;
#[cfg(feature = "mdx")]
use satteri_pulldown_cmark::MDX_OPTIONS;
use satteri_pulldown_cmark::{
    DEFAULT_OPTIONS, Event, Options, Parser, Tag, document, parse, parse_no_positions,
};

#[test]
fn source_spans_preserve_delimiters_continuations_and_autolinks() {
    // Expected HTML is independent of whether inline values borrow source text.
    let cases = [
        (
            "- Foo\n\n      bar\n\n\n      baz\n\t",
            "<ul>\n<li>\n<p>Foo</p>\n<pre><code>bar\n\n\nbaz\n</code></pre>\n</li>\n</ul>\n",
        ),
        (
            "> - Foo\n>\n>       bar\n>\n>\n>       baz\n>\t",
            "<blockquote>\n<ul>\n<li>\n<p>Foo</p>\n<pre><code>bar\n\n\nbaz\n</code></pre>\n</li>\n</ul>\n</blockquote>\n",
        ),
        ("    foo\n\t\n", "<pre><code>foo\n</code></pre>\n"),
        (
            "é😀 &amp; WWW.foo.com\r\n tail",
            "<p>é😀 &amp; <a href=\"http://WWW.foo.com\">WWW.foo.com</a>\r\ntail</p>\n",
        ),
        (
            "[*label*](/a&amp;b \"a &amp; b\") ![alt](/a\\*b \"title\")",
            "<p><a href=\"/a&amp;b\" title=\"a &amp; b\"><em>label</em></a> <img src=\"/a*b\" alt=\"alt\" title=\"title\"></p>\n",
        ),
        (
            "[*label*](/a \"title\") [ref][r]\n\n[r]: /target \"reference title\"",
            "<p><a href=\"/a\" title=\"title\"><em>label</em></a> <a href=\"/target\" title=\"reference title\">ref</a></p>\n",
        ),
        (
            "<https://example.com> <a@example.com> www.example.com",
            "<p><a href=\"https://example.com\">https://example.com</a> <a href=\"mailto:a@example.com\">a@example.com</a> <a href=\"http://www.example.com\">www.example.com</a></p>\n",
        ),
        (
            "*plain* **plain_word** ~~plain~~ ^plain^ ~plain~",
            "<p><em>plain</em> <strong>plain_word</strong> <del>plain</del> ^plain^ <del>plain</del></p>\n",
        ),
        (
            "*雪😀* **plain\\*word** ~~a &amp; b~~",
            "<p><em>雪😀</em> <strong>plain*word</strong> <del>a &amp; b</del></p>\n",
        ),
        (
            "` open ``two`` then ` close",
            "<p><code>open ``two`` then</code> close</p>\n",
        ),
        (
            "`` open `one` then `` close",
            "<p><code>open `one` then</code> close</p>\n",
        ),
        (
            "***word*** and **word**_tail",
            "<p><em><strong>word</strong></em> and <strong>word</strong>_tail</p>\n",
        ),
        (
            "![`code` **strong**](path)",
            "<p><img src=\"path\" alt=\"code strong\"></p>\n",
        ),
        ("alpha\nbeta\ngamma", "<p>alpha\nbeta\ngamma</p>\n"),
        ("alpha  \nbeta", "<p>alpha<br>\nbeta</p>\n"),
        ("alpha\\\nbeta", "<p>alpha<br>\nbeta</p>\n"),
        ("alpha\r\nbeta", "<p>alpha\r\nbeta</p>\n"),
        ("alpha\n---\nbeta", "<h2>alpha</h2>\n<p>beta</p>\n"),
        (
            "alpha\n|a|b|\n|-|-|\n|1|2|",
            "<p>alpha</p>\n<table>\n<thead>\n<tr>\n<th>a</th>\n<th>b</th>\n</tr>\n</thead>\n<tbody>\n<tr>\n<td>1</td>\n<td>2</td>\n</tr>\n</tbody>\n</table>\n",
        ),
        ("alpha\n    continuation", "<p>alpha\ncontinuation</p>\n"),
        (
            "alpha\n> quote",
            "<p>alpha</p>\n<blockquote>\n<p>quote</p>\n</blockquote>\n",
        ),
        (
            "alpha\n- item",
            "<p>alpha</p>\n<ul>\n<li>item</li>\n</ul>\n",
        ),
        ("term\n: definition", "<p>term\n: definition</p>\n"),
        (
            "`` x `` `   ` `雪😀`",
            "<p><code>x</code> <code>   </code> <code>雪😀</code></p>\n",
        ),
    ];
    for (source, expected) in cases {
        let (arena, errors) = parse_no_positions(source, Options::from_bits_truncate(3230));
        assert!(errors.is_empty(), "{source:?}");
        assert_eq!(mdast_to_html(&arena), expected, "{source:?}");
    }
}

#[test]
fn deferred_autolinks_preserve_partially_consumed_strong_delimiters() {
    // Baseline outputs: a URL may consume the opener and only part of the text.
    // The remaining bytes must not retain a previously resolved strong node.
    let cases = [
        (
            "[x](y)http://example.com/**word**",
            "<p><a href=\"y\">x</a><a href=\"http://example.com/**word\">http://example.com/**word</a>**</p>\n",
        ),
        (
            "[x](y)https://example.com/**word**",
            "<p><a href=\"y\">x</a><a href=\"https://example.com/**word\">https://example.com/**word</a>**</p>\n",
        ),
        (
            "[x](y)http://example.com/**two words**",
            "<p><a href=\"y\">x</a><a href=\"http://example.com/**two\">http://example.com/**two</a> words**</p>\n",
        ),
        (
            "[x](y)http://example.com/**word** **next**",
            "<p><a href=\"y\">x</a><a href=\"http://example.com/**word\">http://example.com/**word</a>** <strong>next</strong></p>\n",
        ),
        (
            "[x](y)http://example.com/ **word**",
            "<p><a href=\"y\">x</a><a href=\"http://example.com/\">http://example.com/</a> <strong>word</strong></p>\n",
        ),
        (
            "[x](y)www.example.com/**word**",
            "<p><a href=\"y\">x</a><a href=\"http://www.example.com/\">www.example.com/</a><strong>word</strong></p>\n",
        ),
        (
            "[x](y)http://example.com/\n**word**",
            "<p><a href=\"y\">x</a><a href=\"http://example.com/\">http://example.com/</a>\n<strong>word</strong></p>\n",
        ),
        (
            "[x](y)http://example.com/**word**\n\n**next**",
            "<p><a href=\"y\">x</a><a href=\"http://example.com/**word\">http://example.com/**word</a>**</p>\n<p><strong>next</strong></p>\n",
        ),
    ];
    for options in [
        DEFAULT_OPTIONS,
        #[cfg(feature = "mdx")]
        MDX_OPTIONS,
    ] {
        for (source, expected) in cases {
            for track_positions in [true, false] {
                let (arena, errors) = if track_positions {
                    parse(source, options)
                } else {
                    parse_no_positions(source, options)
                };
                assert!(errors.is_empty(), "{source:?}");
                assert_eq!(mdast_to_html(&arena), expected, "{source:?}");
                let (borrowed, errors) = document::parse(source, options, track_positions);
                assert!(errors.is_empty(), "{source:?}");
                assert_eq!(mdast_to_html(&borrowed), expected, "{source:?}");
            }
        }
        let events: Vec<_> = Parser::new_ext(cases[0].0, options).collect();
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, Event::Start(Tag::Strong)))
        );
        let text: String = events
            .into_iter()
            .filter_map(|event| match event {
                Event::Text(text) => Some(text.into_string()),
                _ => None,
            })
            .collect();
        assert_eq!(text, "xhttp://example.com/**word**");
    }
}
