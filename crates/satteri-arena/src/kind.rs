use std::fmt;

pub trait ArenaKind: 'static {
    const KIND_TAG: u8;
    const NAME: &'static str;
    /// Discriminant of the document root node type. Both `MdastNodeType::Root`
    /// and `HastNodeType::Root` are `0`.
    const ROOT_TAG: u8 = 0;

    /// Rewrite every StringRef in one node's wire `type_data` blob through `remap`, which serialization uses to convert pool byte offsets to the UTF-16 units JS strings index by.
    fn remap_string_refs(node_type: u8, data: &mut [u8], remap: &mut dyn FnMut(u32) -> u32);
}

#[derive(Debug)]
pub struct Mdast;

#[derive(Debug)]
pub struct Hast;

impl ArenaKind for Mdast {
    const KIND_TAG: u8 = 1;
    const NAME: &'static str = "mdast";

    fn remap_string_refs(node_type: u8, data: &mut [u8], remap: &mut dyn FnMut(u32) -> u32) {
        crate::generated::remap_refs::remap_mdast_string_refs(node_type, data, remap);
    }
}

impl ArenaKind for Hast {
    const KIND_TAG: u8 = 2;
    const NAME: &'static str = "hast";

    fn remap_string_refs(node_type: u8, data: &mut [u8], remap: &mut dyn FnMut(u32) -> u32) {
        crate::generated::remap_refs::remap_hast_string_refs(node_type, data, remap);
    }
}

impl fmt::Display for Mdast {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(Self::NAME)
    }
}

impl fmt::Display for Hast {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(Self::NAME)
    }
}
