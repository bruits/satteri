//! Shared constants used by both MDAST and HAST command/codec paths.

// Property value type constants (used in HAST element properties and command wire format)
pub const PROP_STRING: u8 = 0;
pub const PROP_BOOL_TRUE: u8 = 1;
pub const PROP_BOOL_FALSE: u8 = 2;
pub const PROP_SPACE_SEP: u8 = 3;
pub const PROP_COMMA_SEP: u8 = 4;

pub const PROP_INT: u8 = 5;
pub const PROP_NULL: u8 = 6;

// MDX JSX attribute kinds (used in both MDAST and HAST MDX JSX element type_data)
pub const MDX_ATTR_BOOLEAN_PROP: u8 = 0; // name only, no value
pub const MDX_ATTR_LITERAL_PROP: u8 = 1; // name="literal"
pub const MDX_ATTR_EXPRESSION_PROP: u8 = 2; // name={expr}
pub const MDX_ATTR_SPREAD: u8 = 3; // {...expr}

use crate::commands::JsNodeAttribute;
use satteri_arena::{ArenaBuilder, StringRef};

/// Encode JSX attributes from a JS node into the arena tuple format.
/// Used by both MDAST and HAST MDX JSX element paths.
pub fn encode_js_jsx_attrs(
    builder: &mut ArenaBuilder,
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
