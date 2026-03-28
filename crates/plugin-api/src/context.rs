use crate::commands::{Command, NewNode};
use crate::data::{DataMap, DataValue, TypedDataMap};
use tryckeri_mdast::MdastArena;

/// Context passed to Rust plugin visitor methods and before/after hooks.
pub struct PluginContext<'a> {
    arena: &'a MdastArena,
    pub(crate) data_map: &'a mut DataMap,
    pub(crate) typed_data: &'a mut TypedDataMap,
    commands: Vec<Command>,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub message: String,
    pub node_id: Option<u32>,
    pub severity: Severity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl<'a> PluginContext<'a> {
    pub(crate) fn new(
        arena: &'a MdastArena,
        data_map: &'a mut DataMap,
        typed_data: &'a mut TypedDataMap,
    ) -> Self {
        Self {
            arena,
            data_map,
            typed_data,
            commands: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    // ── Arena access ──────────────────────────────────────────────────────────

    pub fn arena(&self) -> &MdastArena {
        self.arena
    }

    /// Extract all text content from a subtree (depth-first concatenation)
    pub fn extract_text(&self, node_id: u32) -> String {
        use tryckeri_mdast::codec::decode_string_ref_data;
        use tryckeri_mdast::MdastNodeType;
        let node = self.arena.get_node(node_id);
        if node.node_type == MdastNodeType::Text as u8
            || node.node_type == MdastNodeType::InlineCode as u8
        {
            let data = self.arena.get_type_data(node_id);
            if !data.is_empty() {
                let string_ref = decode_string_ref_data(data);
                return self.arena.get_str(string_ref).to_string();
            }
            return String::new();
        }
        let children = self.arena.get_children(node_id).to_vec();
        children
            .iter()
            .map(|&child_id| self.extract_text(child_id))
            .collect::<Vec<_>>()
            .join("")
    }

    // ── Untyped data (interoperable with JS node.data) ────────────────────────

    pub fn set_data(&mut self, node_id: u32, key: &str, value: DataValue) {
        self.data_map.set(node_id, key, value);
    }

    pub fn get_data(&self, node_id: u32, key: &str) -> Option<&DataValue> {
        self.data_map.get(node_id, key)
    }

    // ── Typed data (Rust-only, fast) ──────────────────────────────────────────

    pub fn set_typed_data<T: std::any::Any + Send + Sync>(&mut self, node_id: u32, value: T) {
        self.typed_data.set(node_id, value);
    }

    pub fn get_typed_data<T: std::any::Any + Send + Sync>(&self, node_id: u32) -> Option<&T> {
        self.typed_data.get(node_id)
    }

    // ── Structural mutation commands ──────────────────────────────────────────

    pub fn replace_node(&mut self, node_id: u32, new_node: NewNode) {
        self.commands.push(Command::Replace { node_id, new_node });
    }

    pub fn remove_node(&mut self, node_id: u32) {
        self.commands.push(Command::Remove { node_id });
    }

    pub fn insert_before(&mut self, node_id: u32, new_node: NewNode) {
        self.commands
            .push(Command::InsertBefore { node_id, new_node });
    }

    pub fn insert_after(&mut self, node_id: u32, new_node: NewNode) {
        self.commands
            .push(Command::InsertAfter { node_id, new_node });
    }

    pub fn wrap_node(&mut self, node_id: u32, parent_node: NewNode) {
        self.commands.push(Command::Wrap {
            node_id,
            parent_node,
        });
    }

    pub fn prepend_child(&mut self, node_id: u32, child_node: NewNode) {
        self.commands.push(Command::PrependChild {
            node_id,
            child_node,
        });
    }

    pub fn append_child(&mut self, node_id: u32, child_node: NewNode) {
        self.commands.push(Command::AppendChild {
            node_id,
            child_node,
        });
    }

    // ── Diagnostics ───────────────────────────────────────────────────────────

    pub fn report(&mut self, message: impl Into<String>, node_id: Option<u32>, severity: Severity) {
        self.diagnostics.push(Diagnostic {
            message: message.into(),
            node_id,
            severity,
        });
    }

    pub fn error(&mut self, message: impl Into<String>, node_id: Option<u32>) {
        self.report(message, node_id, Severity::Error);
    }

    pub fn warn(&mut self, message: impl Into<String>, node_id: Option<u32>) {
        self.report(message, node_id, Severity::Warning);
    }

    // ── Take results ─────────────────────────────────────────────────────────

    pub(crate) fn take_commands(self) -> (Vec<Command>, Vec<Diagnostic>) {
        (self.commands, self.diagnostics)
    }
}
