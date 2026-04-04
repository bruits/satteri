//! `tryckeri-mdast` — MDAST-specific node types, codecs, and tree operations.
//!
//! Generic arena infrastructure lives in `tryckeri-arena`.

pub mod codec;
pub mod commands;
pub mod node;
pub mod rebuild;
pub mod walk;

pub use codec::{
    decode_code_data, decode_definition_data, decode_expression_data,
    decode_heading_data, decode_image_data, decode_link_data,
    decode_list_data, decode_list_item_data, decode_math_data, decode_mdx_jsx_attr,
    decode_mdx_jsx_attr_count, decode_mdx_jsx_element_name, decode_reference_data,
    encode_code_data, encode_definition_data,
    encode_expression_data, encode_footnote_definition_data, encode_heading_data,
    encode_image_data, encode_link_data, encode_list_data, encode_list_item_data, encode_math_data,
    encode_mdx_jsx_element_data, encode_reference_data, encode_table_data,
    CodeData, ColumnAlign, DefinitionData, ExpressionData, HeadingData,
    ImageData, LinkData, ListData, ListItemData, MathData, MdxJsxElementData, ReferenceData,
};
pub use codec::{
    MDX_ATTR_BOOLEAN_PROP, MDX_ATTR_EXPRESSION_PROP, MDX_ATTR_LITERAL_PROP, MDX_ATTR_SPREAD,
};
pub use commands::{apply_commands, CommandError};
pub use node::MdastNodeType;
pub use rebuild::{rebuild, Patch};
pub use walk::{walk_and_collect, walk_and_collect_with_mode, Subscription, WalkMode};
