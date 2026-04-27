# Plan: support `data.hName`, `data.hProperties`, `data.hChildren` in mdast→hast

## Why

Today the Rust mdast→hast converter (`crates/satteri-ast/src/convert.rs`) hardcodes the mapping from every mdast node type to its hast shape. Unknown types (directives, custom types injected by plugins, etc.) are dropped.

`mdast-util-to-hast` — the JS library satteri parallels — gives plugin authors two ways to influence conversion:

1. `handlers` passed to `toHast`
2. `data.hName` / `data.hProperties` / `data.hChildren` set on the mdast node itself

Option 1 requires a JS-callable hook inside the Rust converter (expensive, per-node FFI). Option 2 is pure data: a plugin mutates the mdast tree, and the converter reads the extra fields when emitting. That's what this plan covers.

Once this lands, the standard remark idiom works:

```js
// User mdast plugin
return (tree) =>
  visit(tree, (node) => {
    if (node.type === "containerDirective" && node.name === "note") {
      node.data ??= {};
      node.data.hName = "aside";
      node.data.hProperties = { className: ["note"] };
    }
  });
```

…and satteri renders `<aside class="note">…</aside>` without any further wiring.

## Reference: what `mdast-util-to-hast` actually does

From `node_modules/.pnpm/mdast-util-to-hast*/lib/state.js` (`applyData`):

- **If no `hName`, `hProperties`, or `hChildren`:** the default/registered handler runs as usual.
- **`hName` only:** the result node is re-cast to `{ type: "element", tagName: hName, properties: {}, children: <original children> }` — the handler's own element shape is discarded but its children are kept.
- **`hProperties`:** merged on top of whatever properties the handler produced (or an empty object if we synthesised the element from `hName`). Later keys win.
- **`hChildren`:** **replaces** the result's children array outright. The children are _hast_ nodes, not mdast — they are inlined as-is.
- **Unknown node types (no handler):** the `defaultUnknownHandler` looks at the node. If it has a `value` and no `hProperties`/`hChildren`, it becomes a `text` node. Otherwise it becomes `<div>` with children from `state.all(node)`. Then `applyData` still runs on top, so `hName` overrides the `div`.

Edge cases to mirror:

- `hName` with `hChildren` = element with that tag and those (already-hast) children.
- `hName` with `hChildren = []` = empty element.
- `hProperties` with `null`/`undefined` values = property stripped.
- Node with only `hChildren` (no `hName`) on a type that has a default handler: the handler runs, its element is kept, children are replaced.

## Scope

**In scope**

- Rust converter honors the three `data.h*` fields on every mdast node type it processes (including the ones it currently drops: directives, unknown).
- JS plugin API to set those fields on materialized mdast nodes.
- Command-buffer sync so those mutations reach the Rust arena for the conversion pass.

**Out of scope (for this plan)**

- `handlers` option (JS-callable per-node hook).
- `passThrough` option (already handled separately for MDX nodes).
- Supporting arbitrary hast subtrees from `hChildren` beyond element/text nodes (we can start with a subset and expand).
- Rehype-side plugins that invent new hast node types.

## Phase 1 — storage

Currently mdast nodes in the arena don't carry `data` at all. We need a per-node optional slot for the three h\* fields. Two possible shapes:

### Option A — typed slot on every node

Add a `HData` struct stored separately, keyed by node id:

```rust
struct HData {
    h_name: Option<StringRef>,
    h_properties: Option<PropertiesRef>,   // same encoding as hast element props
    h_children: Option<HastNodesRef>,      // serialized hast subtree
}
```

Stored in a side map (`FxHashMap<u32, HData>`) rather than inline, so nodes without any h\* fields pay nothing. Arena already has this pattern for auxiliary data.

### Option B — extend the command buffer JsNode

The `JsNode` in `commands.rs` already carries `properties`, `tag_name` for HAST nodes. Extend with `h_name`, `h_properties`, `h_children` for MDAST nodes. When the command is applied, we either write them into the arena (Option A) or — if we want to keep the arena unchanged — we fold them directly into a converted node on the fly.

**Recommended: Option A.** The converter pass happens _after_ all JS plugins have run, so by the time we enter `mdast_arena_to_hast_arena`, the h\* data must already be in the arena. Inline storage in a side map is the cleanest match for that lifecycle.

### Reuse existing encodings

- `hProperties` encoding already exists on the hast side — `encode_element_data_into` in `hast/codec.rs`. Store the exact same bytes in the mdast side map, then pass through directly when emitting.
- `hName` is a plain string — same as hast `tagName` storage.
- `hChildren` is the hardest. Option: serialize as a list of hast node descriptors (type + tagName + props + children), recursively. Alternatively, require `hChildren` to contain only `{type:"text", value:"..."}` for now and widen later. Start with a subset; document the limitation.

## Phase 2 — converter plumbing

In `convert.rs`, every place that currently calls `open_element(builder, "...")` needs to go through a new helper:

```rust
fn open_converted_element(
    builder: &mut ArenaBuilder,
    view: &Arena,
    node_id: u32,
    default_tag: &str,
    default_props: &[PropData],
) -> ElementOutcome;
```

Behavior:

1. Look up `HData` for `node_id`.
2. Effective tag = `h_name.unwrap_or(default_tag)`.
3. Effective props = merge(`default_props`, `h_properties`). Later wins; null values strip.
4. Open the element.
5. If `h_children` is set, emit those children and return `ElementOutcome::ChildrenReplaced` (caller skips its own child emission). Otherwise `ElementOutcome::EmitChildren`.

The directive branch (currently a no-op) becomes:

```rust
Some(MdastNodeType::ContainerDirective | LeafDirective | TextDirective) => {
    if let Some(h) = h_data_for(node_id) {
        // Emit using h.h_name / h_properties / h_children (fallback to <div>?
        // or just skip if no h_name — open for decision).
    }
    // else: skip (today's behavior)
}
```

Design question: **what's the default when a directive has no `hName`?** Two choices:

- **Drop it** (today's behavior, forces users to opt in explicitly).
- **Fall back to the `mdast-util-to-hast` default** (`<div>` with processed children). More convenient but silently renders nothing useful.

Recommendation: **drop**. Users who want a default handler install a plugin. Keeps the architecture coherent with "Rust doesn't invent semantics for unknown node types."

## Phase 3 — JS mutation path

Two things to wire up:

1. **Materialization**: the JS-side mdast reader should expose `data.hName`/`hProperties`/`hChildren` as ordinary fields on nodes (consistent with mdast spec).
2. **Commands**: an edit that sets `data.hName` on a node needs to produce a command that the Rust side replays into the `HData` side map.

The existing `JsNode` already has a `properties` field (HAST-only today). Add:

```rust
pub h_name: Option<String>,
pub h_properties: Option<serde_json::Map<String, serde_json::Value>>,
pub h_children: Option<Vec<JsNode>>, // with _hast: true
```

Add a new command kind `SetHData(node_id, h_data)` or piggyback on the existing node-replace commands. The latter is simpler; the former is cheaper if plugins only mutate `data` without touching children.

## Phase 4 — testing

Add a conformance suite `test/conformance/hdata.test.ts` that covers:

1. `hName` only on a directive → correct tag, children preserved.
2. `hName` + `hProperties` → tag + properties merged onto default.
3. `hProperties` on a known type (e.g., `paragraph`) → merged onto `<p>`.
4. `hChildren` on a known type → default handler's children replaced.
5. `hName` on `root`, `listItem`, headings (various default-handler cases).
6. Setting a property to `null`/`undefined` → property dropped.
7. `data` carried through the JS plugin API round-trip (visit → mutate → re-serialize → rebuilt hast matches).

Each case: reference side uses `toHast` with the same `data` on the mdast; satteri side goes through its own pipeline with a plugin that sets the same fields.

Also: re-run `docs-check.mjs` with a plugin that rewrites `:::note` → `<div class="note">` via `hName`/`hProperties` and confirm the Astro docs render the same HTML on both sides. This is the end-to-end integration check.

## Phase 5 — docs

- README section: "Customizing hast output" with the canonical directive-handler example.
- Note the remaining gap: no `handlers` option; users who need truly imperative per-node logic either (a) transform nodes in an mdast plugin before conversion, or (b) post-process the hast tree with a hast plugin.

## Open questions

1. **Should `hChildren` support arbitrary hast types**, or start with a safe subset (`element`, `text`, `comment`, `doctype`, `raw`) and expand? Subset is easier and matches 99% of plugin usage in the wild.
2. **Serialization format for `hChildren`** — JSON through the command buffer is easiest but allocates; a binary codec mirroring the hast codec is faster but doubles the work. Start with JSON; switch if it shows up in benchmarks.
3. **Does an mdast plugin mutating `data.hName` on a node that otherwise has no type-specific handler** (e.g. an arbitrary custom node type someone invented) need to just work? If yes, the converter needs a generic "unknown mdast node with h-data" path. If no, we restrict this to the known node types. The generic path is ~10 extra lines and is worth it.
4. **Interaction with existing `remarkMarkAndUnravel`**-style post-processing passes in the arena: do they need to copy `HData` across when unraveling? Probably yes — a paragraph that gets unwrapped should carry its `hName` override forward to the surviving node, or we should document that `hName` on an unwrappable paragraph is ignored.

## Estimated size

- Phase 1 storage: ~100 LOC in `satteri-ast` + `satteri-arena`.
- Phase 2 converter: ~80 LOC refactor (touch every `open_element` call site, ~15 of them).
- Phase 3 JS/command: ~60 LOC in `commands.rs` + `satteri-plugin-api`.
- Phase 4 tests: ~200 LOC across one new test file + docs-check extension.
- Phase 5 docs: small.

Total: a focused day of work, gated on deciding the open questions above.
