use std::fmt;

pub trait ArenaKind: 'static {
    const KIND_TAG: u8;
    const NAME: &'static str;
    /// Discriminant of the document root node type. Both `MdastNodeType::Root`
    /// and `HastNodeType::Root` are `0`.
    const ROOT_TAG: u8 = 0;
}

#[derive(Debug)]
pub struct Mdast;

#[derive(Debug)]
pub struct Hast;

impl ArenaKind for Mdast {
    const KIND_TAG: u8 = 1;
    const NAME: &'static str = "mdast";
}

impl ArenaKind for Hast {
    const KIND_TAG: u8 = 2;
    const NAME: &'static str = "hast";
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
