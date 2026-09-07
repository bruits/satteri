//! HAST property name → HTML/SVG attribute name mapping, re-exported from
//! [`satteri_property_info`].

pub use satteri_property_info::{is_known_property, property_to_attribute};

/// ECMAScript includes BOM but excludes NEL from its whitespace set.
pub(crate) fn trim_js_whitespace(value: &str) -> &str {
    value.trim_matches(|c: char| (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}')
}
