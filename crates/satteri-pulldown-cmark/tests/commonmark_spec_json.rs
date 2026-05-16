use satteri_pulldown_cmark::Options;
use serde_json::Value;

// CommonMark spec.json examples whose markdown/expected fields contain
// U+00A0 (non-breaking space) in spec.txt but were normalized to ASCII
// spaces when spec.json was generated. The bytes-on-disk mismatch makes
// these unrunnable against the JSON file — see spec.txt-driven `suite::spec`
// for the authoritative coverage.
const SPEC_JSON_NBSP_NORMALIZED: &[u64] = &[25, 333, 353, 507];

#[test]
fn commonmark_spec_json_conformance() {
    let raw = include_str!("../third_party/CommonMark/spec.json");
    let examples: Vec<Value> = serde_json::from_str(raw).expect("failed to parse spec.json");

    let mut failures: Vec<String> = Vec::new();
    let mut ran = 0usize;

    for ex in &examples {
        let example_id = ex["example"].as_u64().expect("example field");
        if SPEC_JSON_NBSP_NORMALIZED.contains(&example_id) {
            continue;
        }

        let markdown = ex["markdown"].as_str().expect("markdown field");
        let expected_html = ex["html"].as_str().expect("html field");
        let section = ex["section"].as_str().expect("section field");

        let (arena, _) = satteri_pulldown_cmark::parse(markdown, Options::empty());
        let actual = satteri_ast::mdast_to_html(&arena);

        let expected_norm = html_standardize(expected_html);
        let actual_norm = html_standardize(&actual);

        ran += 1;

        if expected_norm != actual_norm {
            failures.push(format!(
                "example {example_id} ({section}):\n  input:    {markdown:?}\n  expected: {expected_html:?}\n  actual:   {actual:?}"
            ));
        }
    }

    if !failures.is_empty() {
        panic!(
            "{} of {ran} CommonMark spec.json examples failed:\n\n{}",
            failures.len(),
            failures.join("\n\n"),
        );
    }
}

fn html_standardize(s: &str) -> String {
    let mut result = s.to_string();
    result = result
        .replace("<br>", "<br />")
        .replace("<br/>", "<br />")
        .replace("<hr>", "<hr />")
        .replace("<hr/>", "<hr />");
    result = result
        .replace("&#x3C;", "&lt;")
        .replace("&#x3E;", "&gt;")
        .replace("&#x26;", "&amp;")
        .replace("&#x22;", "&quot;");
    result = result.replace("&gt;", ">");
    // spec.json encodes literal " as &quot; in expected output, while
    // satteri (and spec.txt) emit raw ". Normalize for comparison.
    result = result.replace("&quot;", "\"");
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
    while result.contains(">\n<") || result.contains(">\n\n<") {
        result = result.replace(">\n\n<", "><");
        result = result.replace(">\n<", "><");
    }
    result
}
