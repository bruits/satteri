use satteri_arena::{Arena, ArenaBuilder, Mdast, StringRef};
use satteri_ast::mdast::{codec::*, MdastNodeType};
use satteri_plugin_api::*;

fn build_test_arena() -> Arena<Mdast> {
    let source = "# Hello\n\nWorld".to_string();
    let mut b = ArenaBuilder::<Mdast>::new(source);

    b.open_node(MdastNodeType::Root as u8);

    b.open_node(MdastNodeType::Heading as u8);
    b.set_position_current(0, 7, 1, 1, 1, 8);
    b.set_data_current(&encode_heading_data(1));

    b.open_node(MdastNodeType::Text as u8);
    b.set_position_current(2, 7, 1, 3, 1, 8);
    b.set_data_current(&encode_string_ref_data(StringRef::new(2, 5)));
    b.close_node();

    b.close_node();

    b.open_node(MdastNodeType::Paragraph as u8);
    b.set_position_current(9, 14, 3, 1, 3, 6);

    b.open_node(MdastNodeType::Text as u8);
    b.set_position_current(9, 14, 3, 1, 3, 6);
    b.set_data_current(&encode_string_ref_data(StringRef::new(9, 5)));
    b.close_node();

    b.close_node();

    b.finish()
}

/// 1. Empty plugin list: returns same arena unchanged, no mutations
#[test]
fn empty_plugin_list_no_mutations() {
    let arena = build_test_arena();
    let node_count = arena.len();
    let mut runner = PluginRunner::new(vec![]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    let result = runner.run(arena, &mut data_map, &mut typed_data);
    assert!(!result.has_mutations);
    assert!(result.commands.is_empty());
    assert!(result.diagnostics.is_empty());
    assert_eq!(result.arena.len(), node_count);
}

/// 2. Single read-only plugin, no mutations, no diagnostics
#[test]
fn single_read_only_plugin_no_mutations() {
    struct ReadOnly;
    impl Plugin for ReadOnly {
        fn meta(&self) -> PluginMeta {
            PluginMeta::new("read-only")
        }
        fn visit_heading(&mut self, _node: &Heading, _ctx: &mut PluginContext) -> VisitResult {
            VisitResult::NoChange
        }
    }

    let arena = build_test_arena();
    let mut runner = PluginRunner::new(vec![Box::new(ReadOnly)]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    let result = runner.run(arena, &mut data_map, &mut typed_data);
    assert!(!result.has_mutations);
    assert!(result.diagnostics.is_empty());
}

/// 3. AddHeadingIds + LintHeadingDepth run in sequence, both work, data from first visible
fn slugify(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_lowercase().next().unwrap()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

struct AddHeadingIds;

impl Plugin for AddHeadingIds {
    fn meta(&self) -> PluginMeta {
        PluginMeta::new("add-heading-ids")
    }

    fn visit_heading(&mut self, node: &Heading, ctx: &mut PluginContext) -> VisitResult {
        let text = ctx.extract_text(node.id());
        let id = slugify(&text);
        ctx.set_data(node.id(), "id", DataValue::String(id));
        VisitResult::NoChange
    }
}

struct LintHeadingDepth {
    max_depth: u8,
}

impl Plugin for LintHeadingDepth {
    fn meta(&self) -> PluginMeta {
        PluginMeta::new("lint-heading-depth")
    }

    fn visit_heading(&mut self, node: &Heading, ctx: &mut PluginContext) -> VisitResult {
        if node.depth() > self.max_depth {
            ctx.warn(
                format!(
                    "Heading depth {} exceeds max {}",
                    node.depth(),
                    self.max_depth
                ),
                Some(node.id()),
            );
        }
        VisitResult::NoChange
    }
}

#[test]
fn two_plugins_run_in_sequence() {
    let arena = build_test_arena();
    let mut runner = PluginRunner::new(vec![
        Box::new(AddHeadingIds),
        Box::new(LintHeadingDepth { max_depth: 3 }),
    ]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    let result = runner.run(arena, &mut data_map, &mut typed_data);

    // First plugin set data
    assert!(data_map.has(1, "id"), "id should be set by AddHeadingIds");
    let id_val = data_map.get(1, "id").unwrap();
    assert_eq!(id_val.as_str().unwrap(), "hello");

    // Second plugin produced no warnings (h1 <= 3)
    assert!(
        result.diagnostics.is_empty(),
        "no warnings for h1 with max_depth=3"
    );
}

/// 4. before and after are called around the traversal
#[test]
fn before_and_after_hooks_called() {
    struct HookTracker;

    impl Plugin for HookTracker {
        fn meta(&self) -> PluginMeta {
            PluginMeta::new("hook-tracker")
        }

        fn before(&mut self, _arena: &Arena<Mdast>, ctx: &mut PluginContext) {
            ctx.set_data(0, "before", DataValue::Bool(true));
        }

        fn after(&mut self, _arena: &Arena<Mdast>, ctx: &mut PluginContext) {
            ctx.set_data(0, "after", DataValue::Bool(true));
        }
    }

    let arena = build_test_arena();
    let mut runner = PluginRunner::new(vec![Box::new(HookTracker)]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    runner.run(arena, &mut data_map, &mut typed_data);

    assert!(
        data_map.has(0, "before"),
        "before hook should have been called"
    );
    assert!(
        data_map.has(0, "after"),
        "after hook should have been called"
    );
}

/// 5. Multiple plugins run in order (counter shows plugin 1 before plugin 2)
#[test]
fn plugins_run_in_order() {
    // Plugin 1 sets a counter key to 1
    // Plugin 2 reads the counter and verifies it is already set (from plugin 1)
    struct SetCounter;
    impl Plugin for SetCounter {
        fn meta(&self) -> PluginMeta {
            PluginMeta::new("set-counter")
        }
        fn before(&mut self, _arena: &Arena<Mdast>, ctx: &mut PluginContext) {
            ctx.set_data(0, "order", DataValue::Int(1));
        }
    }

    struct VerifyCounter;
    impl Plugin for VerifyCounter {
        fn meta(&self) -> PluginMeta {
            PluginMeta::new("verify-counter")
        }
        fn before(&mut self, _arena: &Arena<Mdast>, ctx: &mut PluginContext) {
            // Should already see the data set by plugin 1
            let existing = ctx.get_data(0, "order");
            // Update to 2 to show we ran
            let next = match existing {
                Some(DataValue::Int(v)) => v + 1,
                _ => 99, // sentinel if not found
            };
            ctx.set_data(0, "order", DataValue::Int(next));
        }
    }

    let arena = build_test_arena();
    let mut runner = PluginRunner::new(vec![Box::new(SetCounter), Box::new(VerifyCounter)]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    runner.run(arena, &mut data_map, &mut typed_data);

    let order = data_map.get(0, "order").expect("order should be set");
    // Plugin 1 set to 1, plugin 2 incremented to 2
    assert_eq!(
        order.as_int().unwrap(),
        2,
        "plugins should run in order: 1 then 2"
    );
}

/// A plugin can replace the document root via commands (e.g. from `after`,
/// or a future root visitor emitting commands anchored on node 0).
#[test]
fn plugin_replaces_root_via_commands() {
    struct ReplaceRoot;
    impl Plugin for ReplaceRoot {
        fn meta(&self) -> PluginMeta {
            PluginMeta::new("replace-root")
        }
        fn after(&mut self, _arena: &Arena<Mdast>, ctx: &mut PluginContext) {
            ctx.replace_node(0, NodeBuilder::paragraph().build());
        }
    }

    let arena = build_test_arena();
    let mut runner = PluginRunner::new(vec![Box::new(ReplaceRoot)]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    let result = runner.run(arena, &mut data_map, &mut typed_data);

    assert!(
        result.diagnostics.is_empty(),
        "root replace should apply cleanly: {:?}",
        result
            .diagnostics
            .iter()
            .map(|d| &d.message)
            .collect::<Vec<_>>()
    );
    assert!(result.has_mutations);
    assert_eq!(
        result.arena.get_node(0).node_type,
        MdastNodeType::Paragraph as u8,
        "the replacement's shape lands on node 0"
    );
    assert_eq!(result.arena.get_children(0).len(), 0);
}

/// A plugin can wrap the document root: the wrapper takes over node 0 and
/// the old root (children intact) becomes its first child.
#[test]
fn plugin_wraps_root_via_commands() {
    struct WrapRoot;
    impl Plugin for WrapRoot {
        fn meta(&self) -> PluginMeta {
            PluginMeta::new("wrap-root")
        }
        fn after(&mut self, _arena: &Arena<Mdast>, ctx: &mut PluginContext) {
            ctx.wrap_node(0, NodeBuilder::new(MdastNodeType::Blockquote).build());
        }
    }

    let arena = build_test_arena();
    let mut runner = PluginRunner::new(vec![Box::new(WrapRoot)]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    let result = runner.run(arena, &mut data_map, &mut typed_data);

    assert!(result.diagnostics.is_empty());
    assert_eq!(
        result.arena.get_node(0).node_type,
        MdastNodeType::Blockquote as u8
    );
    let wrapped = result.arena.get_children(0);
    assert_eq!(wrapped.len(), 1);
    let old_root = wrapped[0];
    assert_eq!(
        result.arena.get_node(old_root).node_type,
        MdastNodeType::Root as u8
    );
    assert_eq!(
        result.arena.get_children(old_root).len(),
        2,
        "old root keeps its children under the wrapper"
    );
}

/// A plugin can remove the document root, leaving an empty document.
#[test]
fn plugin_removes_root_via_commands() {
    struct RemoveRoot;
    impl Plugin for RemoveRoot {
        fn meta(&self) -> PluginMeta {
            PluginMeta::new("remove-root")
        }
        fn after(&mut self, _arena: &Arena<Mdast>, ctx: &mut PluginContext) {
            ctx.remove_node(0);
        }
    }

    let arena = build_test_arena();
    let mut runner = PluginRunner::new(vec![Box::new(RemoveRoot)]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    let result = runner.run(arena, &mut data_map, &mut typed_data);

    assert!(result.diagnostics.is_empty());
    assert_eq!(
        result.arena.get_node(0).node_type,
        MdastNodeType::Root as u8
    );
    assert_eq!(result.arena.get_children(0).len(), 0);
}
