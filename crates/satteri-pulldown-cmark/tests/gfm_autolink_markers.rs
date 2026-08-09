//! A deferred autolink marker must never reach `arena_build`; a `debug_assert`
//! there panics if one does, so rendering under a debug build is the check.

use satteri_pulldown_cmark::{Options, parse};

fn html(input: &str) -> String {
    let (arena, _) = parse(input, Options::ENABLE_GFM | Options::ENABLE_MATH);
    satteri_ast::mdast_to_html(&arena)
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
