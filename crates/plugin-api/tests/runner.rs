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

/// 1. Empty plugin list — returns same arena unchanged, no mutations
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

/// 2. Single read-only plugin — no mutations, no diagnostics
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

/// 3. AddHeadingIds + LintHeadingDepth run in sequence — both work, data from first visible
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

        fn before(&mut self, _arena: &MdastArena, ctx: &mut PluginContext) {
            ctx.set_data(0, "before", DataValue::Bool(true));
        }

        fn after(&mut self, _arena: &MdastArena, ctx: &mut PluginContext) {
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
        fn before(&mut self, _arena: &MdastArena, ctx: &mut PluginContext) {
            ctx.set_data(0, "order", DataValue::Int(1));
        }
    }

    struct VerifyCounter;
    impl Plugin for VerifyCounter {
        fn meta(&self) -> PluginMeta {
            PluginMeta::new("verify-counter")
        }
        fn before(&mut self, _arena: &MdastArena, ctx: &mut PluginContext) {
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
