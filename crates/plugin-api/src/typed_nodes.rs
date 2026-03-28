use tryckeri_mdast::codec::*;
use tryckeri_mdast::{MdastArena, MdastNode, MdastNodeType};

/// Position info extracted from an MdastNode
#[derive(Debug, Clone, Copy)]
pub struct NodePosition {
    pub start_offset: u32,
    pub end_offset: u32,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

impl NodePosition {
    pub fn from_node(node: &MdastNode) -> Self {
        Self {
            start_offset: node.start_offset,
            end_offset: node.end_offset,
            start_line: node.start_line,
            start_column: node.start_column,
            end_line: node.end_line,
            end_column: node.end_column,
        }
    }
}

/// A typed view over a Heading node in the arena.
pub struct Heading<'a> {
    pub(crate) node_id: u32,
    pub(crate) arena: &'a MdastArena,
}

impl<'a> Heading<'a> {
    pub fn id(&self) -> u32 {
        self.node_id
    }

    pub fn depth(&self) -> u8 {
        let data = self.arena.get_type_data(self.node_id);
        decode_heading_data(data).depth
    }

    pub fn children(&self) -> &[u32] {
        self.arena.get_children(self.node_id)
    }

    pub fn position(&self) -> NodePosition {
        NodePosition::from_node(self.arena.get_node(self.node_id))
    }
}

/// A typed view over a Text node (also used for InlineCode and Html).
pub struct Text<'a> {
    pub(crate) node_id: u32,
    pub(crate) arena: &'a MdastArena,
}

impl<'a> Text<'a> {
    pub fn id(&self) -> u32 {
        self.node_id
    }

    pub fn value(&self) -> &str {
        let data = self.arena.get_type_data(self.node_id);
        let string_ref = decode_string_ref_data(data);
        self.arena.get_str(string_ref)
    }

    pub fn position(&self) -> NodePosition {
        NodePosition::from_node(self.arena.get_node(self.node_id))
    }
}

/// A typed view over a Link node.
pub struct Link<'a> {
    pub(crate) node_id: u32,
    pub(crate) arena: &'a MdastArena,
}

impl<'a> Link<'a> {
    pub fn id(&self) -> u32 {
        self.node_id
    }

    pub fn url(&self) -> &str {
        let data = self.arena.get_type_data(self.node_id);
        let link = decode_link_data(data);
        self.arena.get_str(link.url)
    }

    pub fn title(&self) -> Option<&str> {
        let data = self.arena.get_type_data(self.node_id);
        let link = decode_link_data(data);
        if link.title.len > 0 {
            Some(self.arena.get_str(link.title))
        } else {
            None
        }
    }

    pub fn children(&self) -> &[u32] {
        self.arena.get_children(self.node_id)
    }

    pub fn position(&self) -> NodePosition {
        NodePosition::from_node(self.arena.get_node(self.node_id))
    }
}

/// A typed view over a Paragraph node.
pub struct Paragraph<'a> {
    pub(crate) node_id: u32,
    pub(crate) arena: &'a MdastArena,
}

impl<'a> Paragraph<'a> {
    pub fn id(&self) -> u32 {
        self.node_id
    }
    pub fn children(&self) -> &[u32] {
        self.arena.get_children(self.node_id)
    }
    pub fn position(&self) -> NodePosition {
        NodePosition::from_node(self.arena.get_node(self.node_id))
    }
}

/// A typed view over an Image node.
pub struct Image<'a> {
    pub(crate) node_id: u32,
    pub(crate) arena: &'a MdastArena,
}

impl<'a> Image<'a> {
    pub fn id(&self) -> u32 {
        self.node_id
    }

    pub fn url(&self) -> &str {
        let data = self.arena.get_type_data(self.node_id);
        let img = decode_image_data(data);
        self.arena.get_str(img.url)
    }

    pub fn alt(&self) -> &str {
        let data = self.arena.get_type_data(self.node_id);
        let img = decode_image_data(data);
        self.arena.get_str(img.alt)
    }

    pub fn title(&self) -> Option<&str> {
        let data = self.arena.get_type_data(self.node_id);
        let img = decode_image_data(data);
        if img.title.len > 0 {
            Some(self.arena.get_str(img.title))
        } else {
            None
        }
    }

    pub fn position(&self) -> NodePosition {
        NodePosition::from_node(self.arena.get_node(self.node_id))
    }
}

/// A typed view over a Code node.
pub struct Code<'a> {
    pub(crate) node_id: u32,
    pub(crate) arena: &'a MdastArena,
}

impl<'a> Code<'a> {
    pub fn id(&self) -> u32 {
        self.node_id
    }

    pub fn lang(&self) -> Option<&str> {
        let data = self.arena.get_type_data(self.node_id);
        let code = decode_code_data(data);
        if code.lang.len > 0 {
            Some(self.arena.get_str(code.lang))
        } else {
            None
        }
    }

    pub fn meta(&self) -> Option<&str> {
        let data = self.arena.get_type_data(self.node_id);
        let code = decode_code_data(data);
        if code.meta.len > 0 {
            Some(self.arena.get_str(code.meta))
        } else {
            None
        }
    }

    /// The code content (the text inside the fence).
    /// For Code nodes, the value is stored as a StringRef in the source after the CodeData.
    /// However, in this arena implementation Code stores lang/meta but the value
    /// is typically the node's text child or stored separately.
    /// We read it from the source via offset/len stored in the node's position range,
    /// or from a child Text node. For now, return via children text extraction.
    pub fn value(&self) -> &str {
        // Code nodes store lang/meta in type_data; the code value is in a child Text node
        // or stored as additional StringRef data after CodeData. Check children first.
        let children = self.arena.get_children(self.node_id);
        if let Some(&child_id) = children.first() {
            let child_node = self.arena.get_node(child_id);
            if child_node.node_type == MdastNodeType::Text as u8 {
                let data = self.arena.get_type_data(child_id);
                if !data.is_empty() {
                    let string_ref = decode_string_ref_data(data);
                    return self.arena.get_str(string_ref);
                }
            }
        }
        // Fallback: if no child text, return empty
        ""
    }

    pub fn position(&self) -> NodePosition {
        NodePosition::from_node(self.arena.get_node(self.node_id))
    }
}
