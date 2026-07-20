//! HAST property name → HTML/SVG attribute name mapping.
//!
//! The mapping lives in the [`satteri-property-info`](satteri_property_info)
//! crate (a Rust port of `property-information`); this module re-exports it so
//! the renderer and MDX code can keep their existing import path.

pub use satteri_property_info::property_to_attribute;
