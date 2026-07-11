//! Recycled (pooled) arenas must behave exactly like fresh ones: a dirty arena from a previous document must never leak content into the next.

use satteri_arena::{Arena, ArenaKind, Hast};
use satteri_ast::hast::{
    hast_arena_to_html, mdast_arena_to_hast_arena_into, mdast_arena_to_hast_arena_with_options,
    ConvertOptions,
};
use satteri_pulldown_cmark::Options;

const DOC: &str =
    "# Héllo wörld\n\nSome *ünïcode…* text with [a link](https://example.com) and `code`.\n\n- one\n- twö\n\n| a | b |\n|---|---|\n| 1 | 2 |\n";

fn dirty_arena<K: ArenaKind>() -> Arena<K> {
    let mut arena = Arena::<K>::new("stale source from a previous document".to_string());
    let parent = arena.alloc_node(1);
    let child = arena.alloc_node(2);
    arena.set_children(parent, &[child]);
    arena.set_type_data(parent, &[0xAA; 24]);
    arena.alloc_string("stale interned string");
    arena.set_node_data(child, vec![0xBB; 64]);
    arena.cp_offsets.push((3, 5));
    arena.mdx = true;
    arena.parse_options = 0xDEAD_BEEF;
    arena
}

fn assert_arena_eq<K: ArenaKind>(fresh: &Arena<K>, reused: &Arena<K>, what: &str) {
    assert_eq!(fresh.nodes, reused.nodes, "{what}: nodes diverged");
    assert_eq!(fresh.children, reused.children, "{what}: children diverged");
    assert_eq!(
        fresh.type_data, reused.type_data,
        "{what}: type_data diverged"
    );
    assert_eq!(
        fresh.string_pool, reused.string_pool,
        "{what}: string_pool diverged"
    );
    assert_eq!(
        fresh.source_len, reused.source_len,
        "{what}: source_len diverged"
    );
    assert_eq!(
        fresh.node_data, reused.node_data,
        "{what}: node_data diverged"
    );
    assert_eq!(fresh.mdx, reused.mdx, "{what}: mdx flag diverged");
    assert_eq!(
        fresh.parse_options, reused.parse_options,
        "{what}: parse_options diverged"
    );
    assert_eq!(
        fresh.cp_offsets, reused.cp_offsets,
        "{what}: cp_offsets diverged"
    );
}

#[test]
fn parse_into_dirty_arena_matches_fresh_parse() {
    let opts = Options::ENABLE_GFM | Options::ENABLE_TABLES;
    let (fresh, _) = satteri_pulldown_cmark::parse(DOC, opts);
    let (reused, _) = satteri_pulldown_cmark::parse_into(DOC, opts, dirty_arena());
    assert!(
        !fresh.cp_offsets.is_empty(),
        "non-ASCII doc must populate cp_offsets or the comparison is vacuous"
    );
    assert_arena_eq(&fresh, &reused, "parse_into");
}

#[test]
fn parse_no_positions_into_dirty_arena_matches_fresh_parse() {
    let opts = Options::ENABLE_GFM | Options::ENABLE_TABLES;
    let (fresh, _) = satteri_pulldown_cmark::parse_no_positions(DOC, opts);
    let (reused, _) = satteri_pulldown_cmark::parse_no_positions_into(DOC, opts, dirty_arena());
    assert_arena_eq(&fresh, &reused, "parse_no_positions_into");
}

#[test]
fn convert_into_dirty_hast_arena_matches_fresh_convert() {
    let opts = Options::ENABLE_GFM | Options::ENABLE_TABLES;
    let (mdast, _) = satteri_pulldown_cmark::parse(DOC, opts);
    let convert_opts = ConvertOptions::default();
    let fresh = mdast_arena_to_hast_arena_with_options(&mdast, &convert_opts);
    let reused = mdast_arena_to_hast_arena_into(&mdast, &convert_opts, dirty_arena::<Hast>());
    assert_arena_eq(&fresh, &reused, "convert_into");
    assert_eq!(
        hast_arena_to_html(&fresh),
        hast_arena_to_html(&reused),
        "rendered HTML diverged"
    );
}

#[test]
fn back_to_back_reuse_of_the_same_arena_stays_clean() {
    let opts = Options::ENABLE_GFM;
    let (first, _) = satteri_pulldown_cmark::parse_into("first *doc*\n", opts, dirty_arena());
    let (second, _) = satteri_pulldown_cmark::parse_into(DOC, opts, first);
    let (fresh, _) = satteri_pulldown_cmark::parse(DOC, opts);
    assert_arena_eq(&fresh, &second, "second reuse");
}
