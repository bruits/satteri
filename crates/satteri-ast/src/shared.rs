//! Shared constants used by both MDAST and HAST command/codec paths.
//!
//! The constant values are generated from the wire-constant tables in
//! `satteri-layout-codegen/src/schema.rs`; this module is their canonical
//! import path.

#[cfg(feature = "mdx")]
pub use crate::generated::wire_constants::{
    MDX_ATTR_BOOLEAN_PROP, MDX_ATTR_EXPRESSION_PROP, MDX_ATTR_LITERAL_PROP, MDX_ATTR_SPREAD,
};
pub use crate::generated::wire_constants::{
    PROP_BOOL_FALSE, PROP_BOOL_TRUE, PROP_COMMA_SEP, PROP_INT, PROP_NULL, PROP_SPACE_SEP,
    PROP_STRING,
};
