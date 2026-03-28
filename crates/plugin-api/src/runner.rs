use crate::commands::{BuiltNode, Command, NewNode};
use crate::context::{Diagnostic, PluginContext};
use crate::data::{DataMap, TypedDataMap};
use crate::plugin::{NodeView, Plugin, VisitResult};
use crate::typed_nodes::*;
use tryckeri_mdast::rebuild::{rebuild, Patch};
use tryckeri_mdast::{MdastArena, MdastBuilder, MdastNodeType};

/// Result of running plugins against an arena.
pub struct PluginRunResult {
    /// The (possibly modified) arena — same instance if no mutations, rebuilt if mutations occurred.
    pub arena: MdastArena,
    pub commands: Vec<Command>,
    pub diagnostics: Vec<Diagnostic>,
    pub has_mutations: bool,
}

/// Runs a list of Rust plugins sequentially against an arena.
pub struct PluginRunner {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginRunner {
    pub fn new(plugins: Vec<Box<dyn Plugin>>) -> Self {
        Self { plugins }
    }

    /// Initialize all plugins (call init on each).
    pub fn init(&mut self) {
        for plugin in &mut self.plugins {
            plugin.init();
        }
    }

    /// Run all plugins against an arena. Returns the result.
    pub fn run(
        &mut self,
        arena: MdastArena,
        data_map: &mut DataMap,
        typed_data: &mut TypedDataMap,
    ) -> PluginRunResult {
        let mut all_commands: Vec<Command> = Vec::new();
        let mut all_diagnostics: Vec<Diagnostic> = Vec::new();
        let mut current_arena = arena;

        for plugin in &mut self.plugins {
            let mut ctx = PluginContext::new(&current_arena, data_map, typed_data);

            // Call before
            plugin.before(&current_arena, &mut ctx);

            // Walk the arena depth-first, dispatch to typed visitor methods
            let node_count = current_arena.len() as u32;
            for node_id in 0..node_count {
                let node = current_arena.get_node(node_id);
                let node_type_byte = node.node_type;

                let result = dispatch_visitor(
                    plugin.as_mut(),
                    node_type_byte,
                    node_id,
                    &current_arena,
                    &mut ctx,
                );

                match result {
                    VisitResult::Replace(new_node) => {
                        ctx.replace_node(node_id, new_node);
                    }
                    VisitResult::Remove => {
                        ctx.remove_node(node_id);
                    }
                    VisitResult::NoChange => {}
                }
            }

            // Call after
            plugin.after(&current_arena, &mut ctx);

            let (commands, diagnostics) = ctx.take_commands();
            let has_cmds = !commands.is_empty();
            all_diagnostics.extend(diagnostics);

            if has_cmds {
                // Convert commands to patches and rebuild the arena
                let patches = commands_to_patches(commands.iter().collect(), &current_arena);
                if !patches.is_empty() {
                    current_arena = rebuild(&current_arena, &patches);
                }
                all_commands.extend(commands);
            }
            // else: skip optimization — current_arena passes through unchanged
            // (Data mutations are already applied via data_map directly)
        }

        let has_mutations = !all_commands.is_empty();

        PluginRunResult {
            arena: current_arena,
            commands: all_commands,
            diagnostics: all_diagnostics,
            has_mutations,
        }
    }
}

/// Convert a list of Commands into Patches.
/// SetData commands are skipped (they are applied directly through the DataMap,
/// not via arena structural mutation).
/// NewNode::Raw commands are skipped (need parser, Phase 8).
fn commands_to_patches(commands: Vec<&Command>, arena: &MdastArena) -> Vec<Patch> {
    commands
        .into_iter()
        .filter_map(|cmd| match cmd {
            Command::Replace { node_id, new_node } => built_node_to_arena(new_node, arena.source())
                .map(|sub| Patch::Replace {
                    node_id: *node_id,
                    new_tree: sub,
                }),
            Command::Remove { node_id } => Some(Patch::Remove { node_id: *node_id }),
            Command::InsertBefore { node_id, new_node } => {
                built_node_to_arena(new_node, arena.source()).map(|sub| Patch::InsertBefore {
                    node_id: *node_id,
                    new_tree: sub,
                })
            }
            Command::InsertAfter { node_id, new_node } => {
                built_node_to_arena(new_node, arena.source()).map(|sub| Patch::InsertAfter {
                    node_id: *node_id,
                    new_tree: sub,
                })
            }
            Command::Wrap {
                node_id,
                parent_node,
            } => built_node_to_arena(parent_node, arena.source()).map(|sub| Patch::Wrap {
                node_id: *node_id,
                parent_tree: sub,
            }),
            Command::PrependChild {
                node_id,
                child_node,
            } => built_node_to_arena(child_node, arena.source()).map(|sub| Patch::PrependChild {
                node_id: *node_id,
                child_tree: sub,
            }),
            Command::AppendChild {
                node_id,
                child_node,
            } => built_node_to_arena(child_node, arena.source()).map(|sub| Patch::AppendChild {
                node_id: *node_id,
                child_tree: sub,
            }),
            Command::SetData { .. } => {
                // Already applied via DataMap in PluginContext — no arena rebuild needed
                None
            }
        })
        .collect()
}

/// Convert a NewNode into a mini MdastArena for use as a patch sub-tree.
/// Returns None for Raw nodes (parser integration is Phase 8).
fn built_node_to_arena(new_node: &NewNode, source: &str) -> Option<MdastArena> {
    match new_node {
        NewNode::Raw(_) => None, // Phase 8
        NewNode::Built(built) => {
            let mut builder = MdastBuilder::new(source.to_string());
            emit_built_node(built, &mut builder);
            Some(builder.finish())
        }
    }
}

/// Recursively emit a BuiltNode into the builder.
fn emit_built_node(built: &BuiltNode, builder: &mut MdastBuilder) {
    builder.open_node(built.node_type);
    if !built.data_bytes.is_empty() {
        builder.set_data_current(&built.data_bytes);
    }
    for child in &built.children {
        match child {
            NewNode::Built(child_built) => emit_built_node(child_built, builder),
            NewNode::Raw(_) => {} // skip
        }
    }
    builder.close_node();
}

/// Dispatch a node to the appropriate typed visitor method.
/// Returns VisitResult from the plugin.
fn dispatch_visitor(
    plugin: &mut dyn Plugin,
    node_type_byte: u8,
    node_id: u32,
    arena: &MdastArena,
    ctx: &mut PluginContext,
) -> VisitResult {
    match MdastNodeType::from_u8(node_type_byte) {
        Some(MdastNodeType::Heading) => plugin.visit_heading(&Heading { node_id, arena }, ctx),
        Some(MdastNodeType::Paragraph) => {
            plugin.visit_paragraph(&Paragraph { node_id, arena }, ctx)
        }
        Some(MdastNodeType::Text) => plugin.visit_text(&Text { node_id, arena }, ctx),
        Some(MdastNodeType::Link) => plugin.visit_link(&Link { node_id, arena }, ctx),
        Some(MdastNodeType::Image) => plugin.visit_image(&Image { node_id, arena }, ctx),
        Some(MdastNodeType::Code) => plugin.visit_code(&Code { node_id, arena }, ctx),
        Some(MdastNodeType::List) => plugin.visit_list(&NodeView { node_id, arena }, ctx),
        Some(MdastNodeType::ListItem) => plugin.visit_list_item(&NodeView { node_id, arena }, ctx),
        Some(MdastNodeType::Blockquote) => {
            plugin.visit_blockquote(&NodeView { node_id, arena }, ctx)
        }
        Some(MdastNodeType::Emphasis) => plugin.visit_emphasis(&NodeView { node_id, arena }, ctx),
        Some(MdastNodeType::Strong) => plugin.visit_strong(&NodeView { node_id, arena }, ctx),
        Some(MdastNodeType::InlineCode) => plugin.visit_inline_code(&Text { node_id, arena }, ctx),
        Some(MdastNodeType::Html) => plugin.visit_html(&Text { node_id, arena }, ctx),
        Some(MdastNodeType::Table) => plugin.visit_table(&NodeView { node_id, arena }, ctx),
        _ => VisitResult::NoChange,
    }
}
