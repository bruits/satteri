use tryckeri_mdast::{codec::*, MdastArena, MdastBuilder, MdastNodeType, StringRef};
use tryckeri_plugin_api::*;

fn build_test_arena() -> MdastArena {
    let source = "# Hello\n\nWorld".to_string();
    let mut b = MdastBuilder::new(source);

    b.open_node(MdastNodeType::Root);

    b.open_node(MdastNodeType::Heading);
    b.set_position_current(0, 7, 1, 1, 1, 8);
    b.set_data_current(&encode_heading_data(1));

    b.open_node(MdastNodeType::Text);
    b.set_position_current(2, 7, 1, 3, 1, 8);
    b.set_data_current(&encode_string_ref_data(StringRef::new(2, 5)));
    b.close_node();

    b.close_node();

    b.open_node(MdastNodeType::Paragraph);
    b.set_position_current(9, 14, 3, 1, 3, 6);

    b.open_node(MdastNodeType::Text);
    b.set_position_current(9, 14, 3, 1, 3, 6);
    b.set_data_current(&encode_string_ref_data(StringRef::new(9, 5)));
    b.close_node();

    b.close_node();

    b.finish()
}

/// Plugin that issues a Replace command for the heading node
struct ReplaceHeading;

impl Plugin for ReplaceHeading {
    fn meta(&self) -> PluginMeta {
        PluginMeta::new("replace-heading")
    }

    fn visit_heading(&mut self, node: &Heading, ctx: &mut PluginContext) -> VisitResult {
        let new_node = NodeBuilder::heading(2).build();
        ctx.replace_node(node.id(), new_node);
        VisitResult::NoChange
    }
}

/// Plugin that issues a Remove command for the heading node
struct RemoveHeading;

impl Plugin for RemoveHeading {
    fn meta(&self) -> PluginMeta {
        PluginMeta::new("remove-heading")
    }

    fn visit_heading(&mut self, node: &Heading, ctx: &mut PluginContext) -> VisitResult {
        ctx.remove_node(node.id());
        VisitResult::NoChange
    }
}

/// 1. ctx.replace_node() adds a Replace command to the result
#[test]
fn replace_command_is_recorded() {
    let arena = build_test_arena();
    let mut runner = PluginRunner::new(vec![Box::new(ReplaceHeading)]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    let result = runner.run(arena, &mut data_map, &mut typed_data);

    assert!(!result.commands.is_empty(), "commands should not be empty");
    let has_replace = result
        .commands
        .iter()
        .any(|c| matches!(c, Command::Replace { .. }));
    assert!(has_replace, "should have a Replace command");
}

/// 2. ctx.remove_node() adds a Remove command
#[test]
fn remove_command_is_recorded() {
    let arena = build_test_arena();
    let mut runner = PluginRunner::new(vec![Box::new(RemoveHeading)]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    let result = runner.run(arena, &mut data_map, &mut typed_data);

    let has_remove = result
        .commands
        .iter()
        .any(|c| matches!(c, Command::Remove { .. }));
    assert!(has_remove, "should have a Remove command");
}

/// 3. has_mutations is false when no commands issued
#[test]
fn no_mutations_when_no_commands() {
    struct NoopPlugin;
    impl Plugin for NoopPlugin {
        fn meta(&self) -> PluginMeta {
            PluginMeta::new("noop")
        }
    }

    let arena = build_test_arena();
    let mut runner = PluginRunner::new(vec![Box::new(NoopPlugin)]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    let result = runner.run(arena, &mut data_map, &mut typed_data);
    assert!(!result.has_mutations);
}

/// 4. has_mutations is true when commands are issued
#[test]
fn has_mutations_when_commands_issued() {
    let arena = build_test_arena();
    let mut runner = PluginRunner::new(vec![Box::new(RemoveHeading)]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    let result = runner.run(arena, &mut data_map, &mut typed_data);
    assert!(result.has_mutations);
}

/// 5. NodeBuilder::heading(1).build() produces a Built node with Heading type
#[test]
fn node_builder_heading_produces_built_node() {
    let new_node = NodeBuilder::heading(1).build();
    match new_node {
        NewNode::Built(built) => {
            assert_eq!(built.node_type, MdastNodeType::Heading);
            // Verify depth encoded correctly
            let depth = tryckeri_mdast::codec::decode_heading_data(&built.data_bytes).depth;
            assert_eq!(depth, 1);
        }
        NewNode::Raw(_) => panic!("expected Built node, got Raw"),
    }
}

/// Extra: NodeBuilder::raw produces a Raw node
#[test]
fn node_builder_raw_produces_raw_node() {
    let new_node = NodeBuilder::raw("# Test");
    match new_node {
        NewNode::Raw(s) => assert_eq!(s, "# Test"),
        NewNode::Built(_) => panic!("expected Raw node"),
    }
}

/// Extra: VisitResult::Replace from visitor method adds Replace command
#[test]
fn visit_result_replace_adds_command() {
    struct ReplaceViaVisitResult;
    impl Plugin for ReplaceViaVisitResult {
        fn meta(&self) -> PluginMeta {
            PluginMeta::new("replace-via-visit")
        }
        fn visit_heading(&mut self, _node: &Heading, _ctx: &mut PluginContext) -> VisitResult {
            VisitResult::Replace(NodeBuilder::paragraph().build())
        }
    }

    let arena = build_test_arena();
    let mut runner = PluginRunner::new(vec![Box::new(ReplaceViaVisitResult)]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    let result = runner.run(arena, &mut data_map, &mut typed_data);
    assert!(result.has_mutations);
    let has_replace = result
        .commands
        .iter()
        .any(|c| matches!(c, Command::Replace { .. }));
    assert!(has_replace);
}
