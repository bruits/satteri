use satteri_pulldown_cmark::Options;

fn parse_to_html(input: &str) -> String {
    let (arena, _) = satteri_pulldown_cmark::parse(input, Options::empty());
    satteri_ast::mdast_to_html(&arena)
}

fn parse_to_html_ext(input: &str, opts: Options) -> String {
    let (arena, _) = satteri_pulldown_cmark::parse(input, opts);
    satteri_ast::mdast_to_html(&arena)
}

#[test]
fn html_test_1() {
    let original = r##"Little header

<script type="text/js">
function some_func() {
console.log("teeeest");
}


function another_func() {
console.log("fooooo");
}
</script>"##;
    let expected = r##"<p>Little header</p>
<script type="text/js">
function some_func() {
console.log("teeeest");
}


function another_func() {
console.log("fooooo");
}
</script>
"##;

    assert_eq!(expected, parse_to_html(original));
}

#[test]
fn html_test_2() {
    let original = r##"Little header

<script
type="text/js">
function some_func() {
console.log("teeeest");
}


function another_func() {
console.log("fooooo");
}
</script>"##;
    let expected = r##"<p>Little header</p>
<script
type="text/js">
function some_func() {
console.log("teeeest");
}


function another_func() {
console.log("fooooo");
}
</script>
"##;

    assert_eq!(expected, parse_to_html(original));
}

#[test]
fn html_test_3() {
    let original = r##"Little header

<?
<div></div>
<p>Useless</p>
?>"##;
    let expected = r##"<p>Little header</p>
<?
<div></div>
<p>Useless</p>
?>
"##;

    assert_eq!(expected, parse_to_html(original));
}

#[test]
fn html_test_4() {
    let original = r##"Little header

<!--
<div></div>
<p>Useless</p>
-->"##;
    let expected = r##"<p>Little header</p>
<!--
<div></div>
<p>Useless</p>
-->
"##;

    assert_eq!(expected, parse_to_html(original));
}

#[test]
fn html_test_5() {
    let original = r##"Little header

<![CDATA[
<div></div>
<p>Useless</p>
]]>"##;
    let expected = r##"<p>Little header</p>
<![CDATA[
<div></div>
<p>Useless</p>
]]>
"##;

    assert_eq!(expected, parse_to_html(original));
}

#[test]
fn html_test_6() {
    let original = r##"Little header

<!X
Some things are here...
>"##;
    let expected = r##"<p>Little header</p>
<!X
Some things are here...
>
"##;

    assert_eq!(expected, parse_to_html(original));
}

#[test]
fn html_test_7() {
    let original = r##"Little header
-----------

<script>
function some_func() {
console.log("teeeest");
}


function another_func() {
console.log("fooooo");
}
</script>"##;
    let expected = r##"<h2>Little header</h2>
<script>
function some_func() {
console.log("teeeest");
}


function another_func() {
console.log("fooooo");
}
</script>
"##;

    assert_eq!(expected, parse_to_html(original));
}

#[test]
fn html_test_8() {
    let original = "A | B\n---|---\nfoo | bar";
    let expected = "<table>\n<thead>\n<tr>\n<th>A</th>\n<th>B</th>\n</tr>\n</thead>\n<tbody>\n<tr>\n<td>foo</td>\n<td>bar</td>\n</tr>\n</tbody>\n</table>\n";

    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    assert_eq!(expected, parse_to_html_ext(original, opts));
}

#[test]
fn html_test_9() {
    let original = "---";
    let expected = "<hr>\n";
    assert_eq!(expected, parse_to_html(original));
}

#[test]
fn html_test_10() {
    let original = "* * *";
    let expected = "<hr>\n";
    assert_eq!(expected, parse_to_html(original));
}

#[test]
fn html_test_11() {
    let original = "hi ~~no~~";
    let expected = "<p>hi ~~no~~</p>\n";
    assert_eq!(expected, parse_to_html(original));
}

#[test]
fn newline_in_code() {
    let originals = ["`\n `x", "` \n`x"];
    let expected = "<p><code>  </code>x</p>\n";

    for original in originals {
        assert_eq!(expected, parse_to_html(original));
    }
}

#[test]
fn newline_start_end_of_code() {
    let original = "`\nx\n`x";
    let expected = "<p><code>x</code>x</p>\n";
    assert_eq!(expected, parse_to_html(original));
}

#[test]
fn trim_space_and_tab_at_end_of_paragraph() {
    let original = "one\ntwo \t";
    let expected = "<p>one\ntwo</p>\n";
    assert_eq!(expected, parse_to_html(original));
}

#[test]
fn newline_within_code() {
    let originals = ["`\nx \ny\n`x", "`x \ny`x", "`x\n y`x"];
    let expected = "<p><code>x  y</code>x</p>\n";

    for original in originals {
        assert_eq!(expected, parse_to_html(original));
    }
}

#[test]
fn trim_space_tab_nl_at_end_of_paragraph() {
    let original = "one\ntwo \t\n";
    let expected = "<p>one\ntwo</p>\n";
    assert_eq!(expected, parse_to_html(original));
}

#[test]
fn trim_space_nl_at_end_of_paragraph() {
    let original = "one\ntwo \n";
    let expected = "<p>one\ntwo</p>\n";
    assert_eq!(expected, parse_to_html(original));
}

#[test]
fn trim_space_before_soft_break() {
    let original = "one \ntwo";
    let expected = "<p>one\ntwo</p>\n";
    assert_eq!(expected, parse_to_html(original));
}

#[test]
fn issue_819() {
    let original = [
        "# \\",
        "# \\\n",
        "# \\\n\n",
        "# \\\r\n",
        "# \\\r\n\r\n",
        "# \\\n\r\n",
        "# \\\r\n\n",
    ];
    let expected = "<h1>\\</h1>";

    for orig in original {
        let s = parse_to_html(orig);
        assert_eq!(expected, s.trim_end_matches('\n'));
    }

    for orig in original {
        let mut opts = Options::empty();
        opts.insert(Options::ENABLE_HEADING_ATTRIBUTES);
        let s = parse_to_html_ext(orig, opts);
        assert_eq!(expected, s.trim_end_matches('\n'));
    }
}

fn parse_to_html_with_heading_attrs(input: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    parse_to_html_ext(input, opts)
}

#[test]
fn heading_attributes_emit_id_and_class() {
    let original = "## Heading {#explicit .custom}";
    let expected = "<h2 id=\"explicit\" class=\"custom\">Heading</h2>\n";
    assert_eq!(expected, parse_to_html_with_heading_attrs(original));
}

#[test]
fn heading_attributes_emit_multiple_classes() {
    let original = "# Title {.a .b}";
    let expected = "<h1 class=\"a b\">Title</h1>\n";
    assert_eq!(expected, parse_to_html_with_heading_attrs(original));
}

#[test]
fn heading_attributes_emit_custom_attributes() {
    let original = "### Note {#n data-key=value flag}";
    let expected = "<h3 id=\"n\" data-key=\"value\" flag=\"\">Note</h3>\n";
    assert_eq!(expected, parse_to_html_with_heading_attrs(original));
}

#[test]
fn heading_attributes_merge_shorthand_and_explicit() {
    let original = "## Heading {.c1 #x class=c2 id=y}";
    let expected = "<h2 id=\"y\" class=\"c1 c2\">Heading</h2>\n";
    assert_eq!(expected, parse_to_html_with_heading_attrs(original));
}

#[test]
fn heading_attributes_quoted_values_keep_spaces() {
    let original = "## Heading {data-x=\"quoted value\" title='also spaced'}";
    let expected = "<h2 data-x=\"quoted value\" title=\"also spaced\">Heading</h2>\n";
    assert_eq!(expected, parse_to_html_with_heading_attrs(original));
}

#[test]
fn heading_attributes_disabled_keeps_literal_text() {
    let original = "## Heading {#explicit .custom}";
    let expected = "<h2>Heading {#explicit .custom}</h2>\n";
    assert_eq!(expected, parse_to_html(original));
}
