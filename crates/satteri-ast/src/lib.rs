//! `satteri-ast`: MDAST and HAST node types, codecs, tree operations, and conversion.

pub mod commands;
pub mod convert;
mod emit;
mod fused_html;
mod generated;
pub mod hast;
pub mod mdast;
pub mod patch;
pub mod shared;
mod stack;
mod swar;
pub mod text_content;
pub mod walk;

/// Convert an mdast arena directly to an HTML string using default options.
pub fn mdast_to_html(arena: &satteri_arena::Arena<satteri_arena::Mdast>) -> String {
    mdast_to_html_with_options(arena, &hast::ConvertOptions::default())
}

/// Convert an mdast arena directly to an HTML string with the given conversion options.
pub fn mdast_to_html_with_options(
    arena: &satteri_arena::Arena<satteri_arena::Mdast>,
    options: &hast::ConvertOptions,
) -> String {
    match fused_html::mdast_to_html_fused(arena, options) {
        Some(html) => html,
        None => {
            let hast = hast::mdast_arena_to_hast_arena_with_options(arena, options);
            hast::hast_arena_to_html(&hast)
        }
    }
}

/// Emit HTML straight from the mdast arena, or `None` when the document needs the HAST pipeline.
pub fn try_mdast_to_html_fused(
    arena: &satteri_arena::Arena<satteri_arena::Mdast>,
    options: &hast::ConvertOptions,
) -> Option<String> {
    fused_html::mdast_to_html_fused(arena, options)
}
