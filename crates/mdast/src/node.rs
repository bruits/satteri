use std::mem::size_of;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MdastNodeType {
    Root = 0,
    Paragraph = 1,
    Heading = 2,
    ThematicBreak = 3,
    Blockquote = 4,
    List = 5,
    ListItem = 6,
    Html = 7,
    Code = 8,
    Definition = 9,
    Text = 10,
    Emphasis = 11,
    Strong = 12,
    InlineCode = 13,
    Break = 14,
    Link = 15,
    Image = 16,
    LinkReference = 17,
    ImageReference = 18,
    FootnoteDefinition = 19,
    FootnoteReference = 20,
    Table = 21,
    TableRow = 22,
    TableCell = 23,
    Delete = 24,
    Yaml = 25,
    Toml = 26,
    Math = 27,
    InlineMath = 28,
    // MDX types start at 100
    MdxJsxFlowElement = 100,
    MdxJsxTextElement = 101,
    MdxFlowExpression = 102,
    MdxTextExpression = 103,
    MdxjsEsm = 104,
}

impl MdastNodeType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(MdastNodeType::Root),
            1 => Some(MdastNodeType::Paragraph),
            2 => Some(MdastNodeType::Heading),
            3 => Some(MdastNodeType::ThematicBreak),
            4 => Some(MdastNodeType::Blockquote),
            5 => Some(MdastNodeType::List),
            6 => Some(MdastNodeType::ListItem),
            7 => Some(MdastNodeType::Html),
            8 => Some(MdastNodeType::Code),
            9 => Some(MdastNodeType::Definition),
            10 => Some(MdastNodeType::Text),
            11 => Some(MdastNodeType::Emphasis),
            12 => Some(MdastNodeType::Strong),
            13 => Some(MdastNodeType::InlineCode),
            14 => Some(MdastNodeType::Break),
            15 => Some(MdastNodeType::Link),
            16 => Some(MdastNodeType::Image),
            17 => Some(MdastNodeType::LinkReference),
            18 => Some(MdastNodeType::ImageReference),
            19 => Some(MdastNodeType::FootnoteDefinition),
            20 => Some(MdastNodeType::FootnoteReference),
            21 => Some(MdastNodeType::Table),
            22 => Some(MdastNodeType::TableRow),
            23 => Some(MdastNodeType::TableCell),
            24 => Some(MdastNodeType::Delete),
            25 => Some(MdastNodeType::Yaml),
            26 => Some(MdastNodeType::Toml),
            27 => Some(MdastNodeType::Math),
            28 => Some(MdastNodeType::InlineMath),
            100 => Some(MdastNodeType::MdxJsxFlowElement),
            101 => Some(MdastNodeType::MdxJsxTextElement),
            102 => Some(MdastNodeType::MdxFlowExpression),
            103 => Some(MdastNodeType::MdxTextExpression),
            104 => Some(MdastNodeType::MdxjsEsm),
            _ => None,
        }
    }
}

/// A reference into the source string — no allocation, just offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
pub struct StringRef {
    pub offset: u32,
    pub len: u32,
}

impl StringRef {
    pub fn new(offset: u32, len: u32) -> Self {
        Self { offset, len }
    }

    pub fn empty() -> Self {
        Self { offset: 0, len: 0 }
    }

    pub fn is_empty(self) -> bool {
        self.len == 0
    }
}

/// All positions use byte offsets and 1-based line/column numbers from the
/// source text.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MdastNode {
    pub id: u32,
    pub node_type: u8,
    pub _pad: [u8; 3],
    pub parent: u32,
    pub start_offset: u32,
    pub end_offset: u32,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    /// Index into MdastArena::children where this node's children start.
    pub children_start: u32,
    pub children_count: u32,
    /// Byte offset into MdastArena::type_data for this node's extra data.
    pub data_offset: u32,
    pub data_len: u32,
}

pub const NODE_STRUCT_SIZE: usize = size_of::<MdastNode>();

impl MdastNode {
    pub fn new(id: u32, node_type: MdastNodeType) -> Self {
        MdastNode {
            id,
            node_type: node_type as u8,
            _pad: [0u8; 3],
            parent: u32::MAX, // sentinel: no parent
            start_offset: 0,
            end_offset: 0,
            start_line: 0,
            start_column: 0,
            end_line: 0,
            end_column: 0,
            children_start: 0,
            children_count: 0,
            data_offset: 0,
            data_len: 0,
        }
    }

    pub fn node_type(&self) -> Option<MdastNodeType> {
        MdastNodeType::from_u8(self.node_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arena_node_size_pinned() {
        // The struct has 13 u32 fields (4 bytes each) + 1 u8 + 3-byte pad
        // = 52 bytes total (with #[repr(C)] no trailing padding is added).
        // This test pins the size so accidental field additions are caught.
        assert_eq!(
            size_of::<MdastNode>(),
            52,
            "MdastNode size changed — update NODE_STRUCT_SIZE callers"
        );
    }

    #[test]
    fn string_ref_is_8_bytes() {
        assert_eq!(size_of::<StringRef>(), 8);
    }

    #[test]
    fn node_type_round_trip() {
        for raw in [
            0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 100, 101, 102, 103, 104,
        ] {
            let nt = MdastNodeType::from_u8(raw).expect("known discriminant");
            assert_eq!(nt as u8, raw);
        }
    }

    #[test]
    fn unknown_node_type_returns_none() {
        assert!(MdastNodeType::from_u8(99).is_none());
        assert!(MdastNodeType::from_u8(29).is_none());
    }
}
