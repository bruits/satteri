//! MDAST-specific node types and codecs.

pub mod codec;
pub(crate) mod generated;
pub mod node;

pub use codec::*;
pub use node::MdastNodeType;

/// Options for `text_content`, matching `mdast-util-to-string`.
pub struct TextContentOptions {
    /// Include `alt` text from image nodes. Default: `true`.
    pub include_image_alt: bool,
    /// Include `value` from HTML nodes. Default: `true`.
    pub include_html: bool,
}

impl Default for TextContentOptions {
    fn default() -> Self {
        Self {
            include_image_alt: true,
            include_html: true,
        }
    }
}

/// Collect concatenated text content from an MDAST arena with default options.
///
/// Mirrors `mdast-util-to-string`: collects `value` from Text, InlineCode,
/// and InlineMath nodes, `alt` from Image nodes, and recurses into children
/// for everything else.
pub fn text_content(arena: &satteri_arena::Arena<satteri_arena::Mdast>, node_id: u32) -> String {
    text_content_with_options(arena, node_id, &TextContentOptions::default())
}

/// Collect concatenated text content with configurable options.
pub fn text_content_with_options(
    arena: &satteri_arena::Arena<satteri_arena::Mdast>,
    node_id: u32,
    options: &TextContentOptions,
) -> String {
    let include_image_alt = options.include_image_alt;
    let include_html = options.include_html;
    crate::text_content::text_content(arena, node_id, |nt| match MdastNodeType::from_u8(nt) {
        Some(MdastNodeType::Text | MdastNodeType::InlineCode) => Some(0),
        Some(MdastNodeType::InlineMath) => Some(8),
        Some(MdastNodeType::Html) if include_html => Some(0),
        Some(MdastNodeType::Image) if include_image_alt => Some(8),
        _ => None,
    })
}
