use tryckeri_mdast::MdastNodeType;

/// A structural mutation command queued during plugin execution.
/// Applied after the plugin finishes (same as JS).
#[derive(Debug, Clone)]
pub enum Command {
    /// Replace a node with a new subtree
    Replace { node_id: u32, new_node: NewNode },
    /// Remove a node entirely
    Remove { node_id: u32 },
    /// Insert a new node before the target
    InsertBefore { node_id: u32, new_node: NewNode },
    /// Insert a new node after the target
    InsertAfter { node_id: u32, new_node: NewNode },
    /// Wrap a node in a new parent
    Wrap { node_id: u32, parent_node: NewNode },
    /// Prepend a child to a node
    PrependChild { node_id: u32, child_node: NewNode },
    /// Append a child to a node
    AppendChild { node_id: u32, child_node: NewNode },
    /// Set a scalar field on a node (used for simple property changes)
    SetData {
        node_id: u32,
        key: String,
        value: crate::data::DataValue,
    },
}

/// A new node to be inserted into the arena.
/// In Phase 5, this is a simple enum. The builder in PluginContext
/// creates these to queue for arena rebuild.
#[derive(Debug, Clone)]
pub enum NewNode {
    /// A raw Markdown string that Rust parses (the `raw` escape hatch)
    Raw(String),
    /// A fully specified node (built with NodeBuilder)
    Built(BuiltNode),
}

/// A node specification built with NodeBuilder
#[derive(Debug, Clone)]
pub struct BuiltNode {
    pub node_type: MdastNodeType,
    pub children: Vec<NewNode>,
    /// Type-specific data bytes (same format as arena type_data)
    pub data_bytes: Vec<u8>,
    /// Optional position override
    pub position: Option<crate::typed_nodes::NodePosition>,
}

/// Builder for constructing new nodes to pass to commands.
pub struct NodeBuilder {
    node_type: MdastNodeType,
    children: Vec<NewNode>,
    data_bytes: Vec<u8>,
}

impl NodeBuilder {
    pub fn new(node_type: MdastNodeType) -> Self {
        Self {
            node_type,
            children: Vec::new(),
            data_bytes: Vec::new(),
        }
    }

    /// Add a child node (another builder or raw string)
    pub fn child(mut self, child: NewNode) -> Self {
        self.children.push(child);
        self
    }

    /// Add multiple children
    pub fn children(mut self, children: impl IntoIterator<Item = NewNode>) -> Self {
        self.children.extend(children);
        self
    }

    /// Set raw type-data bytes (use codec encode_* functions)
    pub fn data_bytes(mut self, bytes: Vec<u8>) -> Self {
        self.data_bytes = bytes;
        self
    }

    /// Finalize into a NewNode
    pub fn build(self) -> NewNode {
        NewNode::Built(BuiltNode {
            node_type: self.node_type,
            children: self.children,
            data_bytes: self.data_bytes,
            position: None,
        })
    }
}

/// Convenience constructors
impl NodeBuilder {
    pub fn heading(depth: u8) -> Self {
        use tryckeri_mdast::codec::encode_heading_data;
        Self::new(MdastNodeType::Heading).data_bytes(encode_heading_data(depth))
    }

    pub fn paragraph() -> Self {
        Self::new(MdastNodeType::Paragraph)
    }

    pub fn text(value_offset: u32, value_len: u32) -> Self {
        use tryckeri_mdast::{codec::encode_string_ref_data, StringRef};
        let string_ref = StringRef {
            offset: value_offset,
            len: value_len,
        };
        Self::new(MdastNodeType::Text).data_bytes(encode_string_ref_data(string_ref))
    }

    /// Create a text node with a raw string (for when we don't have source offsets)
    /// This uses NewNode::Raw internally
    pub fn raw(markdown: impl Into<String>) -> NewNode {
        NewNode::Raw(markdown.into())
    }
}
