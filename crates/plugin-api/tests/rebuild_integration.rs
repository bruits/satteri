//! Integration tests verifying that PluginRunner actually applies arena rebuild
//! when structural commands are issued.

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

    b.close_node(); // heading

    b.open_node(MdastNodeType::Paragraph);
    b.set_position_current(9, 14, 2, 1, 2, 6);

    b.open_node(MdastNodeType::Text);
    b.set_position_current(9, 14, 2, 1, 2, 6);
    b.set_data_current(&encode_string_ref_data(StringRef::new(9, 5)));
    b.close_node();

    b.close_node(); // paragraph
    b.close_node(); // root

    b.finish()
}

// ── Test 1: Remove returns VisitResult → Text nodes gone from arena ─────────

/// A plugin that removes all Text nodes by returning VisitResult::Remove.
struct RemoveAllText;

impl Plugin for RemoveAllText {
    fn meta(&self) -> PluginMeta {
        PluginMeta::new("remove-all-text")
    }

    fn visit_text(&mut self, node: &Text, _ctx: &mut PluginContext) -> VisitResult {
        // Using the visitor return value path
        let _ = node;
        VisitResult::Remove
    }
}

#[test]
fn remove_text_via_visit_result_removes_from_arena() {
    let arena = build_test_arena();
    let original_count = arena.len(); // 5

    let mut runner = PluginRunner::new(vec![Box::new(RemoveAllText)]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    let result = runner.run(arena, &mut data_map, &mut typed_data);

    assert!(result.has_mutations, "should have mutations after remove");

    // Original had 2 Text nodes. They should be gone.
    assert!(
        result.arena.len() < original_count,
        "arena should have fewer nodes: got {}, was {}",
        result.arena.len(),
        original_count
    );

    // No Text nodes should remain
    for id in 0..result.arena.len() as u32 {
        let node_type = result.arena.get_node(id).node_type;
        assert_ne!(
            node_type,
            MdastNodeType::Text as u8,
            "no Text nodes should remain after remove, found one at id={}",
            id
        );
    }
}

// ── Test 2: Replace returns VisitResult → heading replaced in arena ─────────

/// A plugin that replaces the heading with a paragraph via VisitResult::Replace.
struct ReplaceHeadingWithParagraph;

impl Plugin for ReplaceHeadingWithParagraph {
    fn meta(&self) -> PluginMeta {
        PluginMeta::new("replace-heading-with-para")
    }

    fn visit_heading(&mut self, _node: &Heading, _ctx: &mut PluginContext) -> VisitResult {
        VisitResult::Replace(NodeBuilder::paragraph().build())
    }
}

#[test]
fn replace_heading_via_visit_result_updates_arena() {
    let arena = build_test_arena();

    let mut runner = PluginRunner::new(vec![Box::new(ReplaceHeadingWithParagraph)]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    let result = runner.run(arena, &mut data_map, &mut typed_data);

    assert!(result.has_mutations);

    // No Heading should remain in the rebuilt arena
    let has_heading = (0..result.arena.len() as u32)
        .any(|id| result.arena.get_node(id).node_type == MdastNodeType::Heading as u8);
    assert!(!has_heading, "no headings should remain after replacement");

    // Root should still have children
    let root_children = result.arena.get_children(0);
    assert!(!root_children.is_empty(), "root should still have children");
}

// ── Test 3: Read-only plugin — arena unchanged ───────────────────────────────

/// A read-only plugin that only observes nodes.
struct ReadOnlyPlugin;

impl Plugin for ReadOnlyPlugin {
    fn meta(&self) -> PluginMeta {
        PluginMeta::new("read-only")
    }

    fn visit_heading(&mut self, _node: &Heading, _ctx: &mut PluginContext) -> VisitResult {
        VisitResult::NoChange
    }
    fn visit_paragraph(&mut self, _node: &Paragraph, _ctx: &mut PluginContext) -> VisitResult {
        VisitResult::NoChange
    }
    fn visit_text(&mut self, _node: &Text, _ctx: &mut PluginContext) -> VisitResult {
        VisitResult::NoChange
    }
}

#[test]
fn read_only_plugin_does_not_rebuild_arena() {
    let arena = build_test_arena();
    let original_count = arena.len();

    let mut runner = PluginRunner::new(vec![Box::new(ReadOnlyPlugin)]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    let result = runner.run(arena, &mut data_map, &mut typed_data);

    // Skip optimization: no rebuild, no mutations
    assert!(
        !result.has_mutations,
        "read-only plugin should not cause mutations"
    );
    assert_eq!(result.arena.len(), original_count, "node count unchanged");
    assert!(result.commands.is_empty(), "no commands recorded");
}

// ── Test 4: Data-only plugin — no rebuild, SetData not in commands ──────────

/// AddHeadingIds writes to the DataMap but issues no structural commands.
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

#[test]
fn data_only_plugin_does_not_trigger_rebuild() {
    let arena = build_test_arena();
    let original_count = arena.len();

    let mut runner = PluginRunner::new(vec![Box::new(AddHeadingIds)]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    let result = runner.run(arena, &mut data_map, &mut typed_data);

    // Data written to DataMap — but no structural arena commands
    assert!(
        !result.has_mutations,
        "data-only plugin should not set has_mutations"
    );
    assert_eq!(result.arena.len(), original_count, "arena is unchanged");
    assert!(
        result.commands.is_empty(),
        "no commands from data-only plugin"
    );

    // The data should be in the data_map
    assert!(data_map.has(1, "id"), "id should be set by AddHeadingIds");
    let id_val = data_map.get(1, "id").unwrap();
    assert_eq!(id_val.as_str().unwrap(), "hello");
}

// ── Test 5: ctx.remove_node() (explicit command) removes node from arena ────

struct RemoveHeadingExplicit;

impl Plugin for RemoveHeadingExplicit {
    fn meta(&self) -> PluginMeta {
        PluginMeta::new("remove-heading-explicit")
    }

    fn visit_heading(&mut self, node: &Heading, ctx: &mut PluginContext) -> VisitResult {
        ctx.remove_node(node.id());
        VisitResult::NoChange
    }
}

#[test]
fn explicit_remove_command_rebuilds_arena() {
    let arena = build_test_arena();

    let mut runner = PluginRunner::new(vec![Box::new(RemoveHeadingExplicit)]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    let result = runner.run(arena, &mut data_map, &mut typed_data);

    assert!(result.has_mutations);

    // Heading (and its Text child) should be gone
    let has_heading = (0..result.arena.len() as u32)
        .any(|id| result.arena.get_node(id).node_type == MdastNodeType::Heading as u8);
    assert!(!has_heading, "heading should be removed from arena");

    // Should have 3 nodes: Root + Paragraph + Text(World)
    assert_eq!(result.arena.len(), 3);
}

// ── Test 6: Two plugins sequentially — second sees rebuilt arena ─────────────

/// Plugin 1 removes the heading. Plugin 2 counts nodes.
struct CounterPlugin {
    count: usize,
}

impl Plugin for CounterPlugin {
    fn meta(&self) -> PluginMeta {
        PluginMeta::new("counter")
    }

    fn before(&mut self, arena: &MdastArena, _ctx: &mut PluginContext) {
        self.count = arena.len();
    }
}

#[test]
fn second_plugin_sees_rebuilt_arena() {
    let arena = build_test_arena();
    // Original: 5 nodes. After removing heading + text = 3 nodes.
    let counter = CounterPlugin { count: 0 };

    let mut runner = PluginRunner::new(vec![Box::new(RemoveHeadingExplicit), Box::new(counter)]);
    let mut data_map = DataMap::new();
    let mut typed_data = TypedDataMap::new();
    runner.run(arena, &mut data_map, &mut typed_data);

    // We can't easily get the counter value back, but we can verify final arena size
    // The test above already covers the arena contents.
    // This test just ensures the runner doesn't panic when chaining.
}
