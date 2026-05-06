//! `satteri-ast`: MDAST and HAST node types, codecs, tree operations, and conversion.

pub mod commands;
pub mod convert;
pub mod hast;
pub mod mdast;
pub mod rebuild;
pub mod shared;
pub mod text_content;
pub mod walk;

/// Convert an mdast arena directly to an HTML string.
pub fn mdast_to_html(arena: &satteri_arena::Arena<satteri_arena::Mdast>) -> String {
    let hast = hast::mdast_arena_to_hast_arena(arena);
    hast::hast_arena_to_html(&hast)
}
