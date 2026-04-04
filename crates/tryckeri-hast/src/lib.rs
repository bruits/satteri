//! `tryckeri-hast` — HAST conversion and HTML serialization.

pub mod codec;
pub mod render;
pub(crate) mod html;
pub mod node_types;
pub mod text_content;
pub mod convert;

pub use render::{hast_arena_to_html, render_node};
pub use text_content::text_content;
pub use convert::mdast_arena_to_hast_arena;

/// Convert an mdast arena directly to an HTML string.
pub fn mdast_to_html(arena: &tryckeri_arena::Arena) -> String {
    let hast = mdast_arena_to_hast_arena(arena);
    hast_arena_to_html(&hast)
}
