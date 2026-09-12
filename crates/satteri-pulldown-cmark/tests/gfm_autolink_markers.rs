//! First-pass inline shortcuts must preserve link ownership, tree structure,
//! and source positions in both arena parsing modes and both HTML renderers.
//! Rendering in debug builds also catches unresolved autolink markers reaching
//! `arena_build`.

use satteri_arena::StringRef;
use satteri_ast::mdast::{MdastNodeType, decode_link_data};
use satteri_pulldown_cmark::{Event, Options, Parser, Tag, parse, parse_no_positions};

fn html(input: &str) -> String {
    let (arena, _) = parse(input, Options::ENABLE_GFM | Options::ENABLE_MATH);
    satteri_ast::mdast_to_html(&arena)
}

fn assert_html(input: &str, options: Options, expected: &str) {
    for parser in [parse, parse_no_positions] {
        let (arena, errors) = parser(input, options);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(satteri_ast::mdast_to_html(&arena), expected);
        let hast = satteri_ast::hast::mdast_arena_to_hast_arena(&arena);
        assert_eq!(satteri_ast::hast::hast_arena_to_html(&hast), expected);
    }
}

#[test]
fn marker_free_source_spans_do_not_hide_decoded_or_fallback_links() {
    let prose = "Ordinary words without inline markup. ".repeat(12) + "done";
    let source = format!(
        "é😀 {prose}\n\nhttps://example.com\n\n{prose}\n&#119;ww.example.com and a&#64;b.com\n\n[www.example.com"
    );
    let expected = format!(
        "<p>é😀 {prose}</p>\n<p><a href=\"https://example.com\">https://example.com</a></p>\n<p>{prose}\n<a href=\"http://www.example.com\">www.example.com</a> and <a href=\"mailto:a@b.com\">a@b.com</a></p>\n<p>[<a href=\"http://www.example.com\">www.example.com</a></p>\n"
    );
    assert_html(&source, Options::ENABLE_GFM, &expected);
}

#[test]
fn root_paragraph_termination_preserves_trailing_whitespace_and_backslashes() {
    for options in [Options::empty(), Options::ENABLE_GFM, Options::ENABLE_MATH] {
        for ending in ["", "\n", "\r", "\r\n", "\n\n", "\r\r", "\r\n\r\n"] {
            for whitespace in ["", " ", "  ", "\t", "\t  "] {
                assert_html(
                    &format!("before{whitespace}{ending}"),
                    options,
                    "<p>before</p>\n",
                );
            }
            for backslashes in ["\\", "\\\\"] {
                assert_html(
                    &format!("before{backslashes}{ending}"),
                    options,
                    "<p>before\\</p>\n",
                );
            }
        }
        assert_html(
            "before\\\n\nafter",
            options,
            "<p>before\\</p>\n<p>after</p>\n",
        );
        assert_html("before  \nnext", options, "<p>before<br>\nnext</p>\n");
    }
}

#[test]
fn ordinary_continuations_preserve_block_boundaries() {
    for (source, expected) in [
        (
            "before\nheader\n---\ncell",
            "<h2>before\nheader</h2>\n<p>cell</p>\n",
        ),
        (
            "before  \r\nnext\r\n\r\nafter",
            "<p>before<br>\nnext</p>\n<p>after</p>\n",
        ),
        (
            "before\nnext\n> quote",
            "<p>before\nnext</p>\n<blockquote>\n<p>quote</p>\n</blockquote>\n",
        ),
        (
            "one\ntwo\n\nbefore\nheader\n:-\ncell",
            "<p>one\ntwo</p>\n<p>before</p>\n<table>\n<thead>\n<tr>\n<th style=\"text-align: left\">header</th>\n</tr>\n</thead>\n<tbody>\n<tr>\n<td style=\"text-align: left\">cell</td>\n</tr>\n</tbody>\n</table>\n",
        ),
    ] {
        assert_html(source, Options::ENABLE_TABLES, expected);
    }
}

#[test]
fn plain_inline_links_preserve_boundaries_and_ownership() {
    for (source, expected) in [
        (
            "[a](/a \"title\") [b](/b 'second title') [c](/c \"\") [d](/d )",
            "<p><a href=\"/a\" title=\"title\">a</a> <a href=\"/b\" title=\"second title\">b</a> <a href=\"/c\">c</a> <a href=\"/d\">d</a></p>\n",
        ),
        (
            "[a](/a) [ b ](/b) [3]()",
            "<p><a href=\"/a\">a</a> <a href=\"/b\"> b </a> <a href=\"\">3</a></p>\n",
        ),
        (
            "[a](/a) `[b](/b)` [outer [inner](/i)](/o)",
            "<p><a href=\"/a\">a</a> <code>[b](/b)</code> [outer <a href=\"/i\">inner</a>](/o)</p>\n",
        ),
        (
            "[x](https://a.b/點看%zz?q=a&b)é😀]",
            "<p><a href=\"https://a.b/%E9%BB%9E%E7%9C%8B%zz?q=a&amp;b\">x</a>é😀]</p>\n",
        ),
    ] {
        for options in [Options::empty(), Options::ENABLE_GFM] {
            assert_html(source, options, expected);
        }
    }
    // Both scanned and unscanned deferred URLs can own the apparent opener.
    for suffix in ["", " _"] {
        assert_html(
            &format!("[a](https://x.y) http://x.y[x](https://x.y){suffix}"),
            Options::ENABLE_GFM,
            &format!(
                "<p><a href=\"https://x.y\">a</a> <a href=\"http://x.y%5Bx\">http://x.y[x</a>](<a href=\"https://x.y\">https://x.y</a>){suffix}</p>\n"
            ),
        );
    }
}

#[test]
fn plain_inline_destinations_and_titles_keep_source_boundaries() {
    for (source, expected) in [
        (
            "é😀 [x](<> \"\" \t) [y](</p> 'a b' \t)",
            "<p>é😀 <a href=\"\">x</a> <a href=\"/p\" title=\"a b\">y</a></p>\n",
        ),
        (
            "[x](<a\\&b> \"title\") [y](a&amp;b) [z](點看)",
            "<p><a href=\"a&amp;b\" title=\"title\">x</a> <a href=\"a&amp;b\">y</a> <a href=\"%E9%BB%9E%E7%9C%8B\">z</a></p>\n",
        ),
    ] {
        for options in [Options::empty(), Options::ENABLE_GFM] {
            assert_html(source, options, expected);
        }
    }
    for len in [0, 1, u16::MAX as usize, u16::MAX as usize + 1] {
        let value = "a".repeat(len);
        for destination in [value.clone(), format!("<{value}>")] {
            assert_html(
                &format!("é😀 [x]({destination}) tail"),
                Options::empty(),
                &format!("<p>é😀 <a href=\"{value}\">x</a> tail</p>\n"),
            );
        }
        let title_attr = if len == 0 {
            String::new()
        } else {
            format!(" title=\"{value}\"")
        };
        assert_html(
            &format!("é😀 [x](/u \"{value}\" \t) tail"),
            Options::empty(),
            &format!("<p>é😀 <a href=\"/u\"{title_attr}>x</a> tail</p>\n"),
        );
    }
}

#[test]
fn plain_inline_labels_keep_positions_at_the_compact_length_boundary() {
    for len in [1, u16::MAX as usize, u16::MAX as usize + 1] {
        let label = "a".repeat(len);
        let source = format!("é😀 [{label}](/u) tail");
        let expected = format!("<p>é😀 <a href=\"/u\">{label}</a> tail</p>\n");
        assert_html(&source, Options::empty(), &expected);
        let (arena, errors) = parse(&source, Options::empty());
        assert!(errors.is_empty());
        let link = (0..arena.len() as u32)
            .find(|&id| arena.get_node(id).node_type == MdastNodeType::Link as u8)
            .unwrap();
        let children = arena.get_children(link);
        assert_eq!(children.len(), 1);
        let text = arena.get_node(children[0]);
        let parent = arena.get_node(link);
        assert_eq!(text.parent, link);
        assert_eq!(text.start_offset as usize, source.find('[').unwrap() + 1);
        assert_eq!(text.end_offset - text.start_offset, len as u32);
        assert_eq!(text.start_line, parent.start_line);
        assert_eq!(text.start_column, parent.start_column + 1);
        assert_eq!(text.end_column, text.start_column + len as u32);
    }
}

#[test]
fn literal_links_keep_labels_parents_and_source_positions() {
    for (label, url) in [
        ("https://x.y/點看", "https://x.y/點看"),
        ("WWW.EXAMPLE.COM", "http://WWW.EXAMPLE.COM"),
        ("_a@b.com", "mailto:_a@b.com"),
        ("www.a@b.com", "mailto:www.a@b.com"),
    ] {
        for (before, after) in [("", ""), ("**", "**"), ("# ", ""), ("> ", "")] {
            let source = format!("é\n\n{before}{label}{after}");
            for positions in [true, false] {
                let parser = if positions { parse } else { parse_no_positions };
                let (arena, errors) = parser(&source, Options::ENABLE_GFM);
                assert!(errors.is_empty());
                let links: Vec<_> = (0..arena.len() as u32)
                    .filter(|&id| arena.get_node(id).node_type == MdastNodeType::Link as u8)
                    .collect();
                assert_eq!(links.len(), 1, "{source:?}");
                let id = links[0];
                let link = arena.get_node(id);
                let data = decode_link_data(arena.get_type_data(id));
                assert_eq!(arena.get_str(data.url), url);
                assert!(arena.get_str(data.title).is_empty());
                assert!(arena.get_children(link.parent).contains(&id));
                let children = arena.get_children(id);
                assert_eq!(children.len(), 1);
                let text = arena.get_node(children[0]);
                assert_eq!(text.node_type, MdastNodeType::Text as u8);
                assert_eq!(text.parent, id);
                assert_eq!(
                    arena.get_str(StringRef::from_bytes(arena.get_type_data(children[0]))),
                    label
                );
                let start = source.find(label).unwrap() as u32;
                for node in [link, text] {
                    assert_eq!(
                        (node.start_offset, node.end_offset),
                        (start, start + label.len() as u32)
                    );
                    if positions {
                        assert_eq!((node.start_line, node.end_line), (3, 3));
                        assert_eq!(node.start_column, before.len() as u32 + 1);
                    } else {
                        assert_eq!((node.start_line, node.end_line), (0, 0));
                    }
                }
            }
        }
    }
}

#[test]
fn overlapping_protocol_candidates_preserve_one_complete_url() {
    for count in [160, 10240] {
        let url = "http://x.y/".repeat(count);
        assert_html(
            &format!("[a] {url}"),
            Options::ENABLE_GFM,
            &format!("<p>[a] <a href=\"{url}\">{url}</a></p>\n"),
        );
    }
}

#[test]
fn deferred_protocols_respect_the_construct_that_owns_them() {
    for (suffix, expected) in [
        ("[x](https://x.y \"title\")", vec!["https://x.y"]),
        ("`https://x.y`", vec![]),
        ("<span title=\"https://x.y\">text</span>", vec![]),
        (
            "[outer [inner](/i)](https://x.y)",
            vec!["/i", "https://x.y"],
        ),
        ("[x](http://.x.y)", vec!["http://.x.y"]),
        ("[x](https://bad_host.y)", vec!["https://bad_host.y"]),
        (r"[x](https://x.y/a\(b\))", vec!["https://x.y/a(b)"]),
        ("[^n](https://x.y)\n\n[^n]: note", vec!["https://x.y"]),
        ("$https://x.y$", vec![]),
    ] {
        let source = format!("[a](https://x.y) {suffix}");
        let options = Options::ENABLE_GFM | Options::ENABLE_FOOTNOTES | Options::ENABLE_MATH;
        for parser in [parse, parse_no_positions] {
            let (arena, errors) = parser(&source, options);
            assert!(errors.is_empty());
            let urls: Vec<_> = (0..arena.len() as u32)
                .filter(|&id| arena.get_node(id).node_type == MdastNodeType::Link as u8)
                .map(|id| arena.get_str(decode_link_data(arena.get_type_data(id)).url))
                .collect();
            assert_eq!(
                urls,
                [vec!["https://x.y"], expected.clone()].concat(),
                "{source:?}"
            );
            let output = satteri_ast::mdast_to_html(&arena);
            let hast = satteri_ast::hast::mdast_arena_to_hast_arena(&arena);
            assert_eq!(output, satteri_ast::hast::hast_arena_to_html(&hast));
            // An unrelated paragraph must not change later link ownership.
            let (prefixed, errors) = parser(&format!("_\n\n{source}"), options);
            assert!(errors.is_empty());
            assert_eq!(
                satteri_ast::mdast_to_html(&prefixed),
                format!("<p>_</p>\n{output}")
            );
        }
    }
}

#[test]
fn event_links_keep_destinations_when_candidates_fire_or_are_consumed() {
    for (source, urls) in [
        (
            "a@b.com www.x.y https://x.y",
            vec!["a@b.com", "http://www.x.y", "https://x.y"],
        ),
        (
            "[a]() www.x.y [b]()https://x.y [c]()a@b.com",
            vec!["", "http://www.x.y", "", "https://x.y", "", "a@b.com"],
        ),
        (
            "[www.x.y](https://x.y \"title\") `www.x.y` <https://x.y>",
            vec!["https://x.y", "https://x.y"],
        ),
    ] {
        let actual: Vec<_> = Parser::new_ext(source, Options::ENABLE_GFM)
            .filter_map(|event| match event {
                Event::Start(Tag::Link { dest_url, .. }) => Some(dest_url.into_string()),
                _ => None,
            })
            .collect();
        assert_eq!(actual, urls, "{source:?}");
    }
}

#[test]
fn candidate_free_raw_source_still_links_decoded_triggers() {
    for (input, expected) in [
        (
            "&#104;&#116;&#116;&#112;&#58;&#47;&#47;example&#46;com",
            "<a href=\"http://example.com\">http://example.com</a>",
        ),
        (
            "&#87;&#87;&#87;&#46;EXAMPLE&#46;COM",
            "<a href=\"http://WWW.EXAMPLE.COM\">WWW.EXAMPLE.COM</a>",
        ),
        ("a&#64;b&#46;com", "<a href=\"mailto:a@b.com\">a@b.com</a>"),
        (r"a\@b\.com", "<a href=\"mailto:a@b.com\">a@b.com</a>"),
        (
            r"www\.example\.com",
            "<a href=\"http://www.example.com\">www.example.com</a>",
        ),
        ("[a&#64;b&#46;com](/u)", "<a href=\"/u\">a@b.com</a>"),
    ] {
        assert_html(input, Options::ENABLE_GFM, &format!("<p>{expected}</p>\n"));
    }
}

#[test]
fn fallback_url_prefixes_preserve_trails_and_email_precedence() {
    assert_html(
        "[www.x.y/a@b.com). &#104;ttps://x.y/z). a&#64;b.com",
        Options::ENABLE_GFM,
        "<p>[<a href=\"http://www.x.y/a@b.com\">www.x.y/a@b.com</a>). <a href=\"https://x.y/z\">https://x.y/z</a>). <a href=\"mailto:a@b.com\">a@b.com</a></p>\n",
    );
}

#[test]
fn long_plain_lines_preserve_inline_markers_and_breaks() {
    for len in [127, 128, 129, 1024, 4096] {
        let line = "a".repeat(len);
        for newline in ["\n", "\r", "\r\n"] {
            assert_html(
                &format!("{line}{newline}second"),
                Options::ENABLE_GFM,
                &format!("<p>{line}{newline}second</p>\n"),
            );
            assert_html(
                &format!("{line}  {newline}second"),
                Options::ENABLE_GFM,
                &format!("<p>{line}<br>\nsecond</p>\n"),
            );
        }
        assert_html(
            &format!("{line} *em* and a@b.com"),
            Options::ENABLE_GFM,
            &format!("<p>{line} <em>em</em> and <a href=\"mailto:a@b.com\">a@b.com</a></p>\n"),
        );
        assert_html(
            &format!("[{line}](https://x.y/?a=1&b=2 \"title\")"),
            Options::ENABLE_GFM,
            &format!("<p><a href=\"https://x.y/?a=1&amp;b=2\" title=\"title\">{line}</a></p>\n"),
        );
    }
}

#[test]
fn wide_tables_keep_each_plain_cell() {
    for columns in [2, 64, 256] {
        let cell = "word ".repeat(32).trim_end().to_owned();
        let row = vec![cell.as_str(); columns].join(" | ");
        let divider = vec!["---"; columns].join(" | ");
        let cells = format!("<th>{cell}</th>\n").repeat(columns);
        assert_html(
            &format!("{row}\n{divider}"),
            Options::ENABLE_GFM,
            &format!("<table>\n<thead>\n<tr>\n{cells}</tr>\n</thead>\n</table>\n"),
        );
    }
}

#[test]
fn candidate_free_source_keeps_other_gfm_constructs() {
    assert_html(
        "~~whether~~\n\n- [x] whichever\n\n| whole | thing |\n| - | - |\n| works | well |\n",
        Options::ENABLE_GFM,
        "<p><del>whether</del></p>\n<ul class=\"contains-task-list\">\n<li class=\"task-list-item\"><input type=\"checkbox\" checked disabled> whichever</li>\n</ul>\n<table>\n<thead>\n<tr>\n<th>whole</th>\n<th>thing</th>\n</tr>\n</thead>\n<tbody>\n<tr>\n<td>works</td>\n<td>well</td>\n</tr>\n</tbody>\n</table>\n",
    );
}

#[test]
fn repeated_autolinks_preserve_every_link_within_and_across_lines() {
    for count in [160, 10240] {
        for separator in [" ", "\n", "\r\n"] {
            let source = format!("someone+tag@example.com{separator}").repeat(count);
            let link = "<a href=\"mailto:someone+tag@example.com\">someone+tag@example.com</a>";
            let expected = format!("<p>{}</p>\n", vec![link; count].join(separator));
            assert_html(&source, Options::ENABLE_GFM, &expected);
        }
    }
}

#[test]
fn prefix_scanning_covers_all_autolink_triggers() {
    let source = "_a@example.com https://example.com WWW.EXAMPLE.COM ".repeat(160);
    let links = "<a href=\"mailto:_a@example.com\">_a@example.com</a> <a href=\"https://example.com\">https://example.com</a> <a href=\"http://WWW.EXAMPLE.COM\">WWW.EXAMPLE.COM</a> ".repeat(160);
    assert_html(
        &source,
        Options::ENABLE_GFM,
        &format!("<p>{}</p>\n", links.trim_end()),
    );
}

#[test]
fn blockers_after_committed_links_still_defer_later_candidates() {
    let prefix = "a@b.com ".repeat(4);
    let rendered_prefix = "<a href=\"mailto:a@b.com\">a@b.com</a> ".repeat(4);
    for (input, expected) in [
        ("[www.x.y](/u)", "<a href=\"/u\">www.x.y</a>"),
        ("[_a@b.com](/u)", "<a href=\"/u\">_a@b.com</a>"),
        ("`www.x.y`", "<code>www.x.y</code>"),
        ("`_a@b.com`", "<code>_a@b.com</code>"),
        ("<span title=www.x.y>", "<span title=www.x.y>"),
        ("<a@b.com>", "<a href=\"mailto:a@b.com\">a@b.com</a>"),
        ("[\nwww.x.y](/u)", "<a href=\"/u\">\nwww.x.y</a>"),
        ("`\n_a@b.com`", "<code> _a@b.com</code>"),
    ] {
        let source = format!("{prefix}{input} a@b.com");
        let expected =
            format!("<p>{rendered_prefix}{expected} <a href=\"mailto:a@b.com\">a@b.com</a></p>\n");
        assert_html(&source, Options::ENABLE_GFM, &expected);
    }
}

#[test]
fn math_ownership_after_a_committed_email_respects_options() {
    let link = "<a href=\"mailto:a@b.com\">a@b.com</a>";
    for (delimiter, options) in [
        ("$", Options::ENABLE_MATH),
        ("$", Options::ENABLE_MATH_SINGLE_DOLLAR),
        ("$$", Options::ENABLE_MATH_MULTI_DOLLAR),
    ] {
        let input = format!("a@b.com {delimiter}a@b.com{delimiter} a@b.com");
        assert_html(
            &input,
            Options::ENABLE_GFM | options,
            &format!(
                "<p>{link} <code class=\"language-math math-inline\">a@b.com</code> {link}</p>\n"
            ),
        );
        assert_html(
            &input,
            Options::ENABLE_GFM,
            &format!("<p>{link} {delimiter}{link}{delimiter} {link}</p>\n"),
        );
    }
}

#[test]
fn nested_directive_labels_do_not_reuse_the_outer_prefix() {
    let input = "a@b.com :note[_a@b.com `www.x.y`] [www.x.y](/u) a@b.com";
    for parser in [parse, parse_no_positions] {
        let (arena, errors) = parser(input, Options::ENABLE_GFM | Options::ENABLE_DIRECTIVE);
        assert!(errors.is_empty());
        let urls: Vec<_> = (0..arena.len() as u32)
            .filter(|&id| arena.get_node(id).node_type == MdastNodeType::Link as u8)
            .map(|id| arena.get_str(decode_link_data(arena.get_type_data(id)).url))
            .collect();
        assert_eq!(
            urls,
            ["mailto:a@b.com", "mailto:_a@b.com", "/u", "mailto:a@b.com"]
        );
    }
}

#[test]
fn email_urls_are_prefixed_once_in_construct_and_fallback_paths() {
    for (input, expected) in [
        ("<a@b.com>", "<a href=\"mailto:a@b.com\">a@b.com</a>"),
        (
            "<mailto:a@b.com>",
            "<a href=\"mailto:a@b.com\">mailto:a@b.com</a>",
        ),
        (
            "www.a@b.com",
            "<a href=\"mailto:www.a@b.com\">www.a@b.com</a>",
        ),
        ("a&#64;b.com", "<a href=\"mailto:a@b.com\">a@b.com</a>"),
        ("/a+b@c.com", "/a+<a href=\"mailto:b@c.com\">b@c.com</a>"),
        ("é.a@b.com", "é<a href=\"mailto:.a@b.com\">.a@b.com</a>"),
    ] {
        assert_html(input, Options::ENABLE_GFM, &format!("<p>{expected}</p>\n"));
    }

    let address = format!("{}@example.com", "a".repeat(256));
    let (arena, errors) = parse(&address, Options::ENABLE_GFM);
    assert!(errors.is_empty());
    assert_eq!(arena.source(), address);
    let paragraph = arena.get_children(0)[0];
    let link = arena.get_children(paragraph)[0];
    let node = arena.get_node(link);
    assert_eq!(node.node_type, MdastNodeType::Link as u8);
    assert_eq!(
        (node.start_offset, node.end_offset),
        (0, address.len() as u32)
    );
    let data = decode_link_data(arena.get_type_data(link));
    assert_eq!(arena.get_str(data.url), format!("mailto:{address}"));
}

#[test]
fn cached_prefix_does_not_cross_blocks_or_table_cells() {
    let input = "a@b.com\n\n[www.x.y](/u)\n\n# _a@b.com\n\n| a@b.com | [www.x.y](/u) |\n| - | - |\n| `a@b.com` | _a@b.com |\n\n> [www.x.y](/u)\n>\n> a@b.com\n\n- [www.x.y](/u)\n- a@b.com\n";
    let expected = "<p><a href=\"mailto:a@b.com\">a@b.com</a></p>\n<p><a href=\"/u\">www.x.y</a></p>\n<h1><a href=\"mailto:_a@b.com\">_a@b.com</a></h1>\n<table>\n<thead>\n<tr>\n<th><a href=\"mailto:a@b.com\">a@b.com</a></th>\n<th><a href=\"/u\">www.x.y</a></th>\n</tr>\n</thead>\n<tbody>\n<tr>\n<td><code>a@b.com</code></td>\n<td><a href=\"mailto:_a@b.com\">_a@b.com</a></td>\n</tr>\n</tbody>\n</table>\n<blockquote>\n<p><a href=\"/u\">www.x.y</a></p>\n<p><a href=\"mailto:a@b.com\">a@b.com</a></p>\n</blockquote>\n<ul>\n<li><a href=\"/u\">www.x.y</a></li>\n<li><a href=\"mailto:a@b.com\">a@b.com</a></li>\n</ul>\n";
    assert_html(input, Options::ENABLE_GFM, expected);
}

/// The leading `[` is what makes the candidate defer rather than commit.
#[test]
fn a_marker_never_reaches_the_arena() {
    for input in [
        "[a] `www.x.y` b",
        "[a] ``www.x.y`` b",
        "[a] `http://x.y` b",
        "[a] <span title=www.x.y> b",
        "[a] <!-- www.x.y --> b",
        "[a] <http://x.y/z> b",
        "[a] <q@x.y> b",
        "[a] $www.x.y$ b",
        "[a] [b](www.x.y) c",
        "[a] [b](http://x.y/z) c",
        "[a] ![b](www.x.y) c",
        "[a] [www.x.y](/u) c",
        "[a] [q@x.y](/u) c",
        "[a] [www.x.y][r] c\n\n[r]: /u",
        "[a] [www.x.y] c\n\n[www.x.y]: /u",
        "[a] ![www.x.y](/u) c",
    ] {
        assert!(!html(input).is_empty(), "no output for {input:?}");
    }
}

#[test]
fn a_blocked_candidate_leaves_its_bytes_to_other_constructs() {
    assert_eq!(
        html("[a www.x.y/*b*c"),
        "<p>[a <a href=\"http://www.x.y/\">www.x.y/</a><em>b</em>c</p>\n"
    );
    assert_eq!(html("[a `www.x.y` b"), "<p>[a <code>www.x.y</code> b</p>\n");
}

/// The emphasis resolver takes a span's first child by arena index, so a
/// marker dropped from the chain would push that index past the real content.
#[test]
fn a_blocked_marker_keeps_its_place_in_the_sibling_chain() {
    assert_eq!(
        html("[~www.foo.bar~](/x)"),
        "<p><a href=\"/x\"><del>www.foo.bar</del></a></p>\n"
    );
    assert_eq!(
        html("[*www.foo.bar*](/x)"),
        "<p><a href=\"/x\"><em>www.foo.bar</em></a></p>\n"
    );
}

/// An empty destination is the only shape that lands a zero-width marker
/// exactly on the preceding link's splice boundary.
#[test]
fn a_marker_at_a_links_end_offset_survives_the_splice() {
    assert_eq!(
        html("[a]()https://x.y/z"),
        "<p><a href=\"\">a</a><a href=\"https://x.y/z\">https://x.y/z</a></p>\n"
    );
    assert_eq!(
        html("[a]()www.x.y/z"),
        "<p><a href=\"\">a</a><a href=\"http://www.x.y/z\">www.x.y/z</a></p>\n"
    );
}

#[test]
fn a_firing_candidate_splices_away_interior_delimiters() {
    assert_eq!(
        html("[a] www.x.y/*c d*e"),
        "<p>[a] <a href=\"http://www.x.y/*c\">www.x.y/*c</a> d*e</p>\n"
    );
    assert_eq!(
        html("[a] www.x.y/[c d](/x)"),
        "<p>[a] <a href=\"http://www.x.y/%5Bc\">www.x.y/[c</a> d](/x)</p>\n"
    );
}
