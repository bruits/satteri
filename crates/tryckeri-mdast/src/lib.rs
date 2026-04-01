//! `tryckeri-mdast` — arena-allocated MDAST node structures.
//!
//! This crate provides:
//! - `MdastNodeType` enum and `MdastNode` struct
//! - `StringRef` for zero-copy string references into the source text
//! - `MdastArena` for owning all nodes and associated data
//! - `MdastBuilder` for incremental tree construction
//! - `LineIndex` for offset→(line, column) conversion
//! - Raw buffer export/import for Phase 2's zero-copy transfer layer
//! - Codec helpers for encoding/decoding type-specific data structs

pub mod arena;
pub mod builder;
pub mod codec;
pub mod commands;
pub mod line_index;
pub mod mdx_types;
pub mod node;
pub mod raw_buffer;
pub mod read_arena;
pub mod rebuild;
pub mod walk;

pub use arena::MdastArena;
pub use builder::MdastBuilder;
pub use codec::{
    decode_code_data, decode_definition_data, decode_expression_data,
    decode_footnote_definition_data, decode_heading_data, decode_image_data, decode_link_data,
    decode_list_data, decode_list_item_data, decode_math_data, decode_mdx_jsx_attr,
    decode_mdx_jsx_attr_count, decode_mdx_jsx_element_data, decode_mdx_jsx_element_name,
    decode_reference_data, decode_string_ref_data, decode_table_data, encode_code_data,
    encode_definition_data, encode_expression_data, encode_footnote_definition_data,
    encode_heading_data, encode_image_data, encode_link_data, encode_list_data,
    encode_list_item_data, encode_math_data, encode_mdx_jsx_element_data, encode_reference_data,
    encode_string_ref_data, encode_table_data, CodeData, ColumnAlign, DefinitionData,
    ExpressionData, FootnoteDefinitionData, HeadingData, ImageData, LinkData, ListData,
    ListItemData, MathData, MdxJsxElementData, ReferenceData, TableData,
};
pub use codec::{
    MDX_ATTR_BOOLEAN_PROP, MDX_ATTR_EXPRESSION_PROP, MDX_ATTR_LITERAL_PROP, MDX_ATTR_SPREAD,
};
pub use commands::{apply_commands, CommandError};
pub use line_index::LineIndex;
pub use node::{MdastNode, MdastNodeType, StringRef, NODE_STRUCT_SIZE};
pub use raw_buffer::{BufferError, BufferHeader, MdastView, BUFFER_MAGIC, BUFFER_VERSION};
pub use read_arena::ReadMdast;
pub use rebuild::{rebuild, Patch};
pub use walk::{walk_and_collect, walk_and_collect_with_mode, Subscription, WalkMode};
