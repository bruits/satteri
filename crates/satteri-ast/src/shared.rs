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

#[cfg(feature = "mdx")]
use crate::commands::JsNodeAttribute;
#[cfg(feature = "mdx")]
use satteri_arena::{ArenaBuilder, ArenaKind, StringRef};

/// Encode JSX attributes from a JS node into the arena tuple format.
/// Used by both MDAST and HAST MDX JSX element paths; generic over `K`
/// since attribute encoding only needs to allocate strings into the arena
/// and doesn't dispatch on `node_type`.
#[cfg(feature = "mdx")]
pub fn encode_js_jsx_attrs<K: ArenaKind>(
    builder: &mut ArenaBuilder<K>,
    attrs: Option<&[JsNodeAttribute]>,
) -> Vec<(u8, StringRef, StringRef)> {
    let Some(attrs) = attrs else {
        return Vec::new();
    };
    attrs
        .iter()
        .map(|attr| match attr {
            JsNodeAttribute::Attribute { name, value } => {
                let n = builder.alloc_string(name);
                match value {
                    None => (MDX_ATTR_BOOLEAN_PROP, n, StringRef::empty()),
                    Some(serde_json::Value::String(s)) => {
                        let v = builder.alloc_string(s);
                        (MDX_ATTR_LITERAL_PROP, n, v)
                    }
                    Some(serde_json::Value::Object(obj)) => {
                        let expr = obj.get("value").and_then(|v| v.as_str()).unwrap_or("");
                        let v = builder.alloc_string(expr);
                        (MDX_ATTR_EXPRESSION_PROP, n, v)
                    }
                    _ => (MDX_ATTR_BOOLEAN_PROP, n, StringRef::empty()),
                }
            }
            JsNodeAttribute::ExpressionAttribute { value } => {
                let v = builder.alloc_string(value);
                (MDX_ATTR_SPREAD, StringRef::empty(), v)
            }
        })
        .collect()
}
