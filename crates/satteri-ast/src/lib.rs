//! `satteri-ast`: MDAST and HAST node types, codecs, tree operations, and conversion.

pub mod commands;
pub mod convert;
mod generated;
pub mod hast;
pub mod mdast;
pub mod rebuild;
pub mod shared;
pub mod text_content;
pub mod walk;

/// Convert an mdast arena directly to an HTML string using default options.
pub fn mdast_to_html(arena: &satteri_arena::Arena<satteri_arena::Mdast>) -> String {
    let hast = hast::mdast_arena_to_hast_arena(arena);
    hast::hast_arena_to_html(&hast)
}

/// Convert an mdast arena directly to an HTML string with the given conversion options.
pub fn mdast_to_html_with_options(
    arena: &satteri_arena::Arena<satteri_arena::Mdast>,
    options: &hast::ConvertOptions,
) -> String {
    let hast = hast::mdast_arena_to_hast_arena_with_options(arena, options);
    hast::hast_arena_to_html(&hast)
}
