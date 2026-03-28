use crate::context::PluginContext;
use crate::typed_nodes::*;
use tryckeri_mdast::{MdastArena, MdastNodeType};

/// Metadata about a plugin.
#[derive(Debug, Clone)]
pub struct PluginMeta {
    pub name: &'static str,
    pub version: Option<&'static str>,
    pub description: Option<&'static str>,
}

impl PluginMeta {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            version: None,
            description: None,
        }
    }
}

/// The result of a visitor method — either no change, or a replacement.
pub enum VisitResult {
    /// No structural change (plugin may have written to data_map via ctx)
    NoChange,
    /// Replace this node with a new one
    Replace(crate::commands::NewNode),
    /// Remove this node
    Remove,
}

impl VisitResult {
    pub fn no_change() -> Self {
        Self::NoChange
    }
    pub fn replace(node: crate::commands::NewNode) -> Self {
        Self::Replace(node)
    }
    pub fn remove() -> Self {
        Self::Remove
    }
}

/// The Rust plugin trait.
///
/// Implement only the visitor methods you need. Default implementations
/// return NoChange (no-op) so unimplemented visitors have zero overhead.
pub trait Plugin: Send + Sync {
    fn meta(&self) -> PluginMeta;

    /// Called once before any files are processed.
    fn init(&mut self) {}

    /// Called before each file.
    fn before(&mut self, _arena: &MdastArena, _ctx: &mut PluginContext) {}

    /// Called after each file.
    fn after(&mut self, _arena: &MdastArena, _ctx: &mut PluginContext) {}

    // ── Node visitors — implement only what you need ──────────────────────────

    fn visit_heading(&mut self, _node: &Heading, _ctx: &mut PluginContext) -> VisitResult {
        VisitResult::NoChange
    }
    fn visit_paragraph(&mut self, _node: &Paragraph, _ctx: &mut PluginContext) -> VisitResult {
        VisitResult::NoChange
    }
    fn visit_text(&mut self, _node: &Text, _ctx: &mut PluginContext) -> VisitResult {
        VisitResult::NoChange
    }
    fn visit_link(&mut self, _node: &Link, _ctx: &mut PluginContext) -> VisitResult {
        VisitResult::NoChange
    }
    fn visit_image(&mut self, _node: &Image, _ctx: &mut PluginContext) -> VisitResult {
        VisitResult::NoChange
    }
    fn visit_code(&mut self, _node: &Code, _ctx: &mut PluginContext) -> VisitResult {
        VisitResult::NoChange
    }
    fn visit_list(&mut self, _node: &NodeView, _ctx: &mut PluginContext) -> VisitResult {
        VisitResult::NoChange
    }
    fn visit_list_item(&mut self, _node: &NodeView, _ctx: &mut PluginContext) -> VisitResult {
        VisitResult::NoChange
    }
    fn visit_blockquote(&mut self, _node: &NodeView, _ctx: &mut PluginContext) -> VisitResult {
        VisitResult::NoChange
    }
    fn visit_emphasis(&mut self, _node: &NodeView, _ctx: &mut PluginContext) -> VisitResult {
        VisitResult::NoChange
    }
    fn visit_strong(&mut self, _node: &NodeView, _ctx: &mut PluginContext) -> VisitResult {
        VisitResult::NoChange
    }
    fn visit_inline_code(&mut self, _node: &Text, _ctx: &mut PluginContext) -> VisitResult {
        VisitResult::NoChange
    }
    fn visit_html(&mut self, _node: &Text, _ctx: &mut PluginContext) -> VisitResult {
        VisitResult::NoChange
    }
    fn visit_table(&mut self, _node: &NodeView, _ctx: &mut PluginContext) -> VisitResult {
        VisitResult::NoChange
    }

    /// Optional: full arena access for wholesale rewrites. Return None to leave unchanged.
    fn transform_root(
        &mut self,
        _arena: &MdastArena,
        _ctx: &mut PluginContext,
    ) -> Option<MdastArena> {
        None
    }
}

/// A generic node view for nodes that don't have type-specific fields.
pub struct NodeView<'a> {
    pub(crate) node_id: u32,
    pub(crate) arena: &'a MdastArena,
}

impl<'a> NodeView<'a> {
    pub fn id(&self) -> u32 {
        self.node_id
    }
    pub fn children(&self) -> &[u32] {
        self.arena.get_children(self.node_id)
    }
    pub fn position(&self) -> NodePosition {
        NodePosition::from_node(self.arena.get_node(self.node_id))
    }
    pub fn node_type(&self) -> MdastNodeType {
        let raw = self.arena.get_node(self.node_id).node_type;
        MdastNodeType::from_u8(raw).unwrap_or(MdastNodeType::Root)
    }
}
