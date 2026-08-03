//! Marker lifetime for deferred GFM autolink candidates.
//!
//! A candidate detected behind a `[` becomes a zero-width `MaybeAutolink`
//! item that `handle_inline_pass1` must resolve — by firing it or by
//! unlinking it — before `arena_build` runs. Constructs that consume an item
//! range (code spans, inline HTML, math, JSX, a resolved link destination)
//! drop any marker inside it, which is the second half of the invariant.
//!
//! `arena_build` carries a `debug_assert` for a marker that reaches it, so
//! every input here is really two assertions: the expected HTML, and no
//! marker escaping. Detection-time guards currently keep most of these
//! shapes from producing a marker at all; the cases stay so that removing
//! those guards is measured against them.

use satteri_pulldown_cmark::{parse, Options};

fn html(input: &str) -> String {
    let (arena, _) = parse(input, Options::ENABLE_GFM | Options::ENABLE_MATH);
    satteri_ast::mdast_to_html(&arena)
}

/// Every shape where a candidate sits inside something that owns its bytes.
/// The leading `[` is what makes the first pass defer rather than commit.
#[test]
fn a_marker_never_reaches_the_arena() {
    for input in [
        // Consumed by a code span.
        "[a] `www.x.y` b",
        "[a] ``www.x.y`` b",
        "[a] `http://x.y` b",
        // Consumed by inline HTML.
        "[a] <span title=www.x.y> b",
        "[a] <!-- www.x.y --> b",
        // Consumed by a pointed autolink.
        "[a] <http://x.y/z> b",
        "[a] <q@x.y> b",
        // Consumed by a math span.
        "[a] $www.x.y$ b",
        // Consumed by a link destination that resolves.
        "[a] [b](www.x.y) c",
        "[a] [b](http://x.y/z) c",
        "[a] ![b](www.x.y) c",
        // Consumed by a link label that resolves.
        "[a] [www.x.y](/u) c",
        "[a] [q@x.y](/u) c",
        // Reference links, whose splice takes a different route.
        "[a] [www.x.y][r] c\n\n[r]: /u",
        "[a] [www.x.y] c\n\n[www.x.y]: /u",
        // The candidate is the whole label of an image.
        "[a] ![www.x.y](/u) c",
    ] {
        // Both assertions live in `html`: the debug build panics on an
        // escaped marker, and a non-empty render proves the block survived.
        assert!(!html(input).is_empty(), "no output for {input:?}");
    }
}

/// A blocked candidate leaves its bytes as ordinary text, so the delimiters
/// inside it tokenize normally — the point of not skipping the URL bytes.
/// Expectations measured against remark-parse@11 + remark-gfm@4.
#[test]
fn a_blocked_candidate_leaves_its_bytes_to_other_constructs() {
    assert_eq!(
        html("[a www.x.y/*b*c"),
        "<p>[a <a href=\"http://www.x.y/\">www.x.y/</a><em>b</em>c</p>\n"
    );
    assert_eq!(html("[a `www.x.y` b"), "<p>[a <code>www.x.y</code> b</p>\n");
}

/// A blocked marker stays in the sibling chain as a zero-width text node.
/// The emphasis resolver takes the node after a delimiter run to be the span's
/// first child by arena index, so a marker removed from the chain would leave
/// that index pointing past the span's real content.
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

/// A firing candidate discards whatever the URL bytes tokenized into, and an
/// orphaned closer falls back to literal text.
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
