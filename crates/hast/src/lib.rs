//! `tryckeri-hast` — HAST conversion and HTML serialization.

pub mod codec;
pub mod convert;
pub mod from_binary;
pub mod node;
pub mod node_types;
pub mod serialize;
pub mod to_binary;

pub use convert::mdast_to_hast;
pub use from_binary::{hast_buffer_to_html, render_node};
pub use node::{HastArena, HastBuilder, HastNode, HastNodeType, Property, PropertyValue};
pub use serialize::hast_to_html;
pub use to_binary::mdast_to_hast_buffer;

/// Convert an mdast arena directly to an HTML string.
pub fn mdast_to_html(arena: &tryckeri_mdast::MdastArena) -> String {
    let hast = mdast_to_hast(arena);
    hast_to_html(&hast)
}
