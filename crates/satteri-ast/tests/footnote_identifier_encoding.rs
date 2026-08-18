//! Expectations derived by running remark@15 + remark-gfm@4 + remark-rehype.

use satteri_ast::mdast_to_html;

/// A paragraph each, so a punctuation identifier cannot pair into a math or code span.
fn html_for(identifier: &str) -> String {
    let md =
        format!("Text[^{identifier}].\n\nAgain[^{identifier}].\n\n[^{identifier}]: Note body.\n");
    let (arena, _) = satteri_pulldown_cmark::parse(&md, satteri_pulldown_cmark::DEFAULT_OPTIONS);
    mdast_to_html(&arena)
}

fn attribute_values(html: &str, attribute: &str) -> Vec<String> {
    let needle = format!(" {attribute}=\"");
    let mut values = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find(&needle) {
        let after = &rest[start + needle.len()..];
        let end = after.find('"').expect("unterminated attribute value");
        values.push(decode_character_references(&after[..end]));
        rest = &after[end..];
    }
    values
}

fn decode_character_references(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(ampersand) = rest.find('&') {
        out.push_str(&rest[..ampersand]);
        let tail = &rest[ampersand..];
        let (decoded, len) = if tail.starts_with("&amp;") {
            ('&', 5)
        } else if tail.starts_with("&quot;") {
            ('"', 6)
        } else if tail.starts_with("&lt;") {
            ('<', 4)
        } else if tail.starts_with("&gt;") {
            ('>', 4)
        } else if tail.starts_with("&#x27;") {
            ('\'', 6)
        } else if tail.starts_with("&#x60;") {
            ('`', 6)
        } else {
            panic!("unhandled character reference in {value}");
        };
        out.push(decoded);
        rest = &tail[len..];
    }
    out.push_str(rest);
    out
}

const CASES: &[(&str, &str)] = &[
    ("a!b", "a!b"),
    ("a\"b", "a%22b"),
    ("a#b", "a#b"),
    ("a$b", "a$b"),
    ("a%b", "a%25b"),
    ("a&b", "a&b"),
    ("a'b", "a'b"),
    ("a(b", "a(b"),
    ("a)b", "a)b"),
    ("a*b", "a*b"),
    ("a+b", "a+b"),
    ("a,b", "a,b"),
    ("a-b", "a-b"),
    ("a.b", "a.b"),
    ("a/b", "a/b"),
    ("a0b", "a0b"),
    ("a1b", "a1b"),
    ("a2b", "a2b"),
    ("a3b", "a3b"),
    ("a4b", "a4b"),
    ("a5b", "a5b"),
    ("a6b", "a6b"),
    ("a7b", "a7b"),
    ("a8b", "a8b"),
    ("a9b", "a9b"),
    ("a:b", "a:b"),
    ("a;b", "a;b"),
    ("a<b", "a%3Cb"),
    ("a=b", "a=b"),
    ("a>b", "a%3Eb"),
    ("a?b", "a?b"),
    ("a@b", "a@b"),
    ("aAb", "aab"),
    ("aBb", "abb"),
    ("aCb", "acb"),
    ("aDb", "adb"),
    ("aEb", "aeb"),
    ("aFb", "afb"),
    ("aGb", "agb"),
    ("aHb", "ahb"),
    ("aIb", "aib"),
    ("aJb", "ajb"),
    ("aKb", "akb"),
    ("aLb", "alb"),
    ("aMb", "amb"),
    ("aNb", "anb"),
    ("aOb", "aob"),
    ("aPb", "apb"),
    ("aQb", "aqb"),
    ("aRb", "arb"),
    ("aSb", "asb"),
    ("aTb", "atb"),
    ("aUb", "aub"),
    ("aVb", "avb"),
    ("aWb", "awb"),
    ("aXb", "axb"),
    ("aYb", "ayb"),
    ("aZb", "azb"),
    ("a\\b", "a%5Cb"),
    ("a^b", "a%5Eb"),
    ("a_b", "a_b"),
    ("a`b", "a%60b"),
    ("aab", "aab"),
    ("abb", "abb"),
    ("acb", "acb"),
    ("adb", "adb"),
    ("aeb", "aeb"),
    ("afb", "afb"),
    ("agb", "agb"),
    ("ahb", "ahb"),
    ("aib", "aib"),
    ("ajb", "ajb"),
    ("akb", "akb"),
    ("alb", "alb"),
    ("amb", "amb"),
    ("anb", "anb"),
    ("aob", "aob"),
    ("apb", "apb"),
    ("aqb", "aqb"),
    ("arb", "arb"),
    ("asb", "asb"),
    ("atb", "atb"),
    ("aub", "aub"),
    ("avb", "avb"),
    ("awb", "awb"),
    ("axb", "axb"),
    ("ayb", "ayb"),
    ("azb", "azb"),
    ("a{b", "a%7Bb"),
    ("a|b", "a%7Cb"),
    ("a}b", "a%7Db"),
    ("a~b", "a~b"),
    ("café", "caf%C3%A9"),
    ("naïve", "na%C3%AFve"),
    ("straße", "strasse"),
    ("ÿ", "%C3%BF"),
    ("żółw", "%C5%BC%C3%B3%C5%82w"),
    ("Ωμέγα", "%CF%89%CE%BC%CE%AD%CE%B3%CE%B1"),
    ("привет", "%D0%BF%D1%80%D0%B8%D0%B2%D0%B5%D1%82"),
    ("日本", "%E6%97%A5%E6%9C%AC"),
    ("a日本b", "a%E6%97%A5%E6%9C%ACb"),
    ("한국어", "%ED%95%9C%EA%B5%AD%EC%96%B4"),
    ("שלום", "%D7%A9%D7%9C%D7%95%D7%9D"),
    ("مرحبا", "%D9%85%D8%B1%D8%AD%D8%A8%D8%A7"),
    ("☃", "%E2%98%83"),
    ("😀", "%F0%9F%98%80"),
    ("😀😀", "%F0%9F%98%80%F0%9F%98%80"),
    ("👍🏽", "%F0%9F%91%8D%F0%9F%8F%BD"),
    ("👩\u{200d}💻", "%F0%9F%91%A9%E2%80%8D%F0%9F%92%BB"),
    ("cafe\u{301}", "cafe%CC%81"),
    ("a\u{a0}b", "a%C2%A0b"),
    ("a\u{ad}b", "a%C2%ADb"),
    ("a\u{200b}b", "a%E2%80%8Bb"),
    ("a\u{feff}b", "a%EF%BB%BFb"),
    ("𠀀", "%F0%A0%80%80"),
    ("a%41b", "a%41b"),
    ("a%c3%a9b", "a%c3%a9b"),
    ("a%20b", "a%20b"),
    ("a%2gb", "a%2gb"),
    ("a%zzb", "a%zzb"),
    ("ab%", "ab%25"),
    ("ab%4", "ab%254"),
    ("ab%41", "ab%41"),
    ("a%%b", "a%25%25b"),
    ("%", "%25"),
    ("a%25b", "a%25b"),
    ("%ab", "%ab"),
    ("foo", "foo"),
    ("FOO", "foo"),
    ("FoO", "foo"),
    ("CAFÉ", "caf%C3%A9"),
    ("ıI", "ii"),
    ("İi", "i%CC%87i"),
    ("ΣΣ", "%CF%83%CF%82"),
    ("a\"%b", "a%22%25b"),
    ("a<&>b", "a%3C&%3Eb"),
    ("a%b&c<d>e\"f'g", "a%25b&c%3Cd%3Ee%22f'g"),
    ("1", "1"),
    ("12", "12"),
    ("a-b_c.d~e", "a-b_c.d~e"),
];

#[test]
fn footnote_fragments_match_remark() {
    for &(identifier, fragment) in CASES {
        let html = html_for(identifier);
        assert_eq!(
            attribute_values(&html, "href"),
            vec![
                format!("#user-content-fn-{fragment}"),
                format!("#user-content-fn-{fragment}"),
                format!("#user-content-fnref-{fragment}"),
                format!("#user-content-fnref-{fragment}-2"),
            ],
            "href set for {identifier:?}: {html}"
        );
        assert_eq!(
            attribute_values(&html, "id"),
            vec![
                format!("user-content-fnref-{fragment}"),
                format!("user-content-fnref-{fragment}-2"),
                "footnote-label".to_string(),
                format!("user-content-fn-{fragment}"),
            ],
            "id set for {identifier:?}: {html}"
        );
    }
}
