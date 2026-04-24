use satteri_pulldown_cmark::Options;

#[rustfmt::skip]
mod suite;

#[inline(never)]
#[allow(clippy::too_many_arguments)]
pub fn test_markdown_html(
    input: &str,
    output: &str,
    base_options: u32,
    smart_punct: bool,
    metadata_blocks: bool,
    subscript: bool,
    wikilinks: bool,
    deflists: bool,
    container_extensions: bool,
) {
    let mut opts = Options::from_bits_truncate(base_options);
    if smart_punct {
        opts.insert(Options::ENABLE_SMART_PUNCTUATION);
    }
    if metadata_blocks {
        opts.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
        opts.insert(Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS);
    }
    if subscript {
        opts.insert(Options::ENABLE_SUBSCRIPT);
    }
    if wikilinks {
        opts.insert(Options::ENABLE_WIKILINKS);
    }
    if deflists {
        opts.insert(Options::ENABLE_DEFINITION_LIST);
    }
    if container_extensions {
        opts.insert(Options::ENABLE_CONTAINER_EXTENSIONS);
    }

    let (arena, _) = satteri_pulldown_cmark::parse(input, opts);
    let s = satteri_ast::mdast_to_html(&arena);

    assert_eq!(html_standardize(output), html_standardize(&s));
}

fn html_standardize(s: &str) -> String {
    let mut result = s.to_string();
    // Normalize void element self-closing style
    result = result
        .replace("<br>", "<br />")
        .replace("<br/>", "<br />")
        .replace("<hr>", "<hr />")
        .replace("<hr/>", "<hr />");
    // Normalize HTML entity encoding style (hex vs named, > encoding)
    result = result
        .replace("&#x3C;", "&lt;")
        .replace("&#x3E;", "&gt;")
        .replace("&#x26;", "&amp;")
        .replace("&#x22;", "&quot;");
    // rehype-stringify doesn't encode >, but our renderer does — normalize
    result = result.replace("&gt;", ">");
    // Normalize <img ...> to <img ... /> (and similar void elements)
    for tag in ["img", "input"] {
        let open = format!("<{tag}");
        let mut i = 0;
        while let Some(pos) = result[i..].find(&open) {
            let abs = i + pos;
            if let Some(end) = result[abs..].find('>') {
                let tag_end = abs + end;
                if !result[..tag_end].ends_with('/') {
                    result.insert(tag_end, ' ');
                    result.insert(tag_end + 1, '/');
                }
                i = tag_end + 3;
            } else {
                break;
            }
        }
    }
    // Collapse any whitespace-only gap between tags
    while result.contains(">\n<") || result.contains(">\n\n<") {
        result = result.replace(">\n\n<", "><");
        result = result.replace(">\n<", "><");
    }
    result
}
