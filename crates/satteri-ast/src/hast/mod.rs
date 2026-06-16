//! HAST-specific node types, codecs, and rendering.

pub mod codec;
pub(crate) mod generated;
pub mod node;
pub mod properties;
pub mod render;

pub use crate::convert::{
    mdast_arena_to_hast_arena, mdast_arena_to_hast_arena_with_options, Backref, ConvertOptions,
};
pub use node::HastNodeType;
pub use render::{hast_arena_to_html, render_node};

/// Collect concatenated text content from a HAST arena.
///
/// Mirrors `hast-util-to-string`: text nodes contribute their value,
/// other nodes recurse into children.
pub fn text_content(arena: &satteri_arena::Arena<satteri_arena::Hast>, node_id: u32) -> String {
    crate::text_content::text_content(arena, node_id, |nt| match HastNodeType::from_u8(nt) {
        Some(
            HastNodeType::Text | HastNodeType::MdxFlowExpression | HastNodeType::MdxTextExpression,
        ) => Some(0),
        _ => None,
    })
}
