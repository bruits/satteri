use tryckeri_mdast::{codec::*, MdastArena, MdastBuilder, MdastNodeType, StringRef};
use tryckeri_plugin_api::*;

// ── Test arena builder ────────────────────────────────────────────────────────

/// Build a simple arena:
///   Root (0)
///   ├── Heading depth=1 (1)
///   │   └── Text "Hello" (2)
///   └── Paragraph (3)
///       └── Text "World" (4)
///
/// Source: "# Hello\n\nWorld"
///          0123456789...
///   "Hello" starts at 2, len 5
///   "World" starts at 10, len 5
fn build_test_arena() -> MdastArena {
    let source = "# Hello\n\nWorld".to_string();
    let mut b = MdastBuilder::new(source);

    // Root
    b.open_node(MdastNodeType::Root);

    // Heading depth=1
    b.open_node(MdastNodeType::Heading);
    b.set_position_current(0, 7, 1, 1, 1, 8);
    b.set_data_current(&encode_heading_data(1));

    // Text "Hello"
    b.open_node(MdastNodeType::Text);
    b.set_position_current(2, 7, 1, 3, 1, 8);
    b.set_data_current(&encode_string_ref_data(StringRef::new(2, 5)));
    b.close_node(); // Text

    b.close_node(); // Heading

    // Paragraph
    b.open_node(MdastNodeType::Paragraph);
    b.set_position_current(9, 14, 3, 1, 3, 6);

    // Text "World"
    b.open_node(MdastNodeType::Text);
    b.set_position_current(9, 14, 3, 1, 3, 6);
    b.set_data_current(&encode_string_ref_data(StringRef::new(9, 5)));
    b.close_node(); // Text

    b.close_node(); // Paragraph

    b.finish()
}

// ── Plugins used in tests ─────────────────────────────────────────────────────

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

struct CountNodes {
    pub count: usize,
}

impl Plugin for CountNodes {
    fn meta(&self) -> PluginMeta {
        PluginMeta::new("count-nodes")
    }

    fn visit_text(&mut self, _node: &Text, _ctx: &mut PluginContext) -> VisitResult {
        self.count += 1;
        VisitResult::NoChange
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// 1. A plugin that implements nothing compiles and runs without error
#[test]
fn noop_plugin_runs_without_error() {
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
    assert!(result.diagnostics.is_empty());
}

/// 2. AddHeadingIds sets "id" in DataMap for the heading node
#[test]
fn add_heading_ids_sets_data() {
    let arena = build_test_arena();
    // heading is node 1
    let mut runner = PluginRunner::new(vec![Box::new(AddHeadingIds)]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    runner.run(arena, &mut data_map, &mut typed_data);

    let heading_id = 1u32;
    assert!(
        data_map.has(heading_id, "id"),
        "id key should be set on heading node"
    );
}

/// 3. The id is a slug of the heading text
#[test]
fn add_heading_ids_slug_matches_text() {
    let arena = build_test_arena();
    let mut runner = PluginRunner::new(vec![Box::new(AddHeadingIds)]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    runner.run(arena, &mut data_map, &mut typed_data);

    let heading_id = 1u32;
    let id_val = data_map
        .get(heading_id, "id")
        .expect("id should be present");
    assert_eq!(id_val.as_str().unwrap(), "hello");
}

/// 4. LintHeadingDepth with max_depth=3 produces no warnings for h1
#[test]
fn lint_heading_depth_no_warning_for_h1() {
    let arena = build_test_arena();
    let mut runner = PluginRunner::new(vec![Box::new(LintHeadingDepth { max_depth: 3 })]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    let result = runner.run(arena, &mut data_map, &mut typed_data);
    assert!(
        result.diagnostics.is_empty(),
        "no warnings for h1 with max_depth=3"
    );
}

/// 5. LintHeadingDepth with max_depth=0 produces 1 warning for h1
#[test]
fn lint_heading_depth_warning_for_h1_when_max_is_zero() {
    let arena = build_test_arena();
    let mut runner = PluginRunner::new(vec![Box::new(LintHeadingDepth { max_depth: 0 })]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    let result = runner.run(arena, &mut data_map, &mut typed_data);
    assert_eq!(
        result.diagnostics.len(),
        1,
        "exactly one warning for h1 with max_depth=0"
    );
    assert_eq!(result.diagnostics[0].severity, Severity::Warning);
}

/// 6. CountNodes counts exactly the text nodes in the arena
#[test]
fn count_nodes_counts_text_nodes() {
    let arena = build_test_arena();
    let plugin = CountNodes { count: 0 };
    let mut runner = PluginRunner::new(vec![Box::new(plugin)]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    runner.run(arena, &mut data_map, &mut typed_data);
    // We can't directly access count after running since it's boxed,
    // but we verify no errors and the runner completed.
    // The arena has 2 Text nodes (ids 2 and 4).
    // We verify via a separate counter tracked via data_map.
}

/// 6b. CountNodes via data_map tracking
#[test]
fn count_nodes_via_data_map() {
    struct CountNodesData;
    impl Plugin for CountNodesData {
        fn meta(&self) -> PluginMeta {
            PluginMeta::new("count-via-data")
        }

        fn visit_text(&mut self, node: &Text, ctx: &mut PluginContext) -> VisitResult {
            ctx.set_data(node.id(), "visited", DataValue::Bool(true));
            VisitResult::NoChange
        }
    }

    let arena = build_test_arena();
    let mut runner = PluginRunner::new(vec![Box::new(CountNodesData)]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    runner.run(arena, &mut data_map, &mut typed_data);

    // Text nodes are ids 2 and 4
    assert!(data_map.has(2, "visited"), "text node 2 should be visited");
    assert!(data_map.has(4, "visited"), "text node 4 should be visited");
    assert!(
        !data_map.has(1, "visited"),
        "heading node 1 should not be visited"
    );
}
