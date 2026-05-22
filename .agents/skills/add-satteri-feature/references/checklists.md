# Feature Checklists

Detailed step-by-step checklists for common feature types. Each step includes the file to modify and what to do there.

## New MDAST Node Type

Adding a new node type to the Markdown AST (e.g., a new extension like directives, math, or custom syntax).

### Rust layer

1. **Enum variant** -- `crates/satteri-ast/src/mdast/node.rs`
   - Add a new variant to `MdastNodeType` with a unique `u8` discriminant
   - Add the variant to `from_u8()` match

2. **Codec** -- `crates/satteri-ast/src/mdast/codec.rs`
   - Define a `#[repr(C)]` data struct if the node has type-specific fields (use `StringRef` for strings)
   - Implement `encode_*_data()` function that writes the struct bytes
   - Implement `decode_*_data()` function(s) that read the struct bytes
   - Add codec roundtrip unit test in the `#[cfg(test)]` module

3. **Parser bridge** -- `crates/satteri-pulldown-cmark/src/arena_build.rs`
   - Map the pulldown-cmark event(s) to the new node type
   - Use `ArenaBuilder::open_node()` / `close_node()` for container nodes, `add_leaf()` for leaf nodes
   - Call the codec's `encode_*_data()` to produce the type_data bytes

4. **Conversion** -- `crates/satteri-ast/src/convert.rs`
   - Add a match arm in `convert_node()` for the new `MdastNodeType` variant
   - Map it to the appropriate HAST element (or skip it if it has no HTML representation)

5. **Walk serialization** -- `crates/satteri-ast/src/walk.rs`
   - Add the node type to `serialize_mdast_node_inline()` in the match
   - Serialize the type-specific data inline (position, children, then custom fields)

6. **Rebuild StringRef remapping** -- `crates/satteri-ast/src/rebuild.rs`
   - Add the node type to `remap_mdast_string_refs()` if its type_data contains any `StringRef` fields
   - List the byte offsets of each StringRef that needs remapping

7. **Text content** -- `crates/satteri-ast/src/text_content.rs` (via `mdast/mod.rs`)
   - If the node contributes text content (like Text or InlineCode), add its type_data offset to the `text_offset` closure in `mdast/mod.rs::text_content_with_options()`

8. **JS node builder** -- `crates/satteri-plugin-api/src/js_commands.rs`
   - Add the node type string to `name_to_node_type()` match
   - Add encoding logic in `encode_js_node_data()` for when JS sends this node type in a SERDE_JSON payload
   - If the node has properties that can be set via `setProperty`, add field resolution in `resolve_mdast_field()` and handle the set in `apply_mdast_set_property()`

### TypeScript layer

9. **Types** -- `packages/satteri/src/types.ts` (or `mdx-types.ts` / `directive-types.ts`)
   - Define the TypeScript interface for the node
   - Add module augmentation to register it in the mdast content maps

10. **Reader** -- `packages/satteri/src/mdast/mdast-reader.ts`
    - Add the node type number to the `NodeType` constant object and `NodeTypeName` reverse map
    - Add decoder method(s) for type-specific data (e.g., `getMyNodeData()`)

11. **Materializer** -- `packages/satteri/src/mdast/mdast-materializer.ts`
    - Add the type string to `TYPE_NAMES`
    - Add to `LEAF_TYPES` if the node never has children
    - Add a case in `addTypeProperties()` to attach lazy property getters using `lazyProp` / `lazyGroup`

12. **Visitor** -- `packages/satteri/src/mdast/mdast-visitor.ts`
    - Add the visitor method to `MdastPluginInstance` interface with the correct node type
    - Add subscription resolution in `resolveMdastSubscriptions()` (map method name to node type number)
    - Add the node decoding case in `readMdastMatchedNode()` (read from the walk binary buffer)

13. **Plugin definition** -- `packages/satteri/src/plugin.ts`
    - The `MdastPluginDefinition` type picks up the new visitor from `MdastPluginInstance` automatically

14. **Public export** -- `packages/satteri/src/index.ts`
    - Export the new type if it's part of the public API

### Tests

15. **Tests at each layer:**
    - Spec test in `crates/satteri-pulldown-cmark/specs/*.txt`
    - Codec roundtrip in `crates/satteri-ast/src/mdast/codec.rs`
    - Conversion test (if applicable) in `crates/satteri-ast/src/convert.rs` or `crates/satteri-ast/tests/html.rs`
    - JS conformance test in `packages/satteri/test/conformance/`
    - JS visitor test showing the plugin can visit and mutate the new node type

---

## New HAST Node Type

Adding a new node type to the HTML AST.

1. **Enum variant** -- `crates/satteri-ast/src/hast/node.rs`
   - Add variant to `HastNodeType` with unique `u8` discriminant, add to `from_u8()`

2. **Codec** -- `crates/satteri-ast/src/hast/codec.rs`
   - Define type_data layout, implement encode/decode functions

3. **Conversion** -- `crates/satteri-ast/src/convert.rs`
   - Produce this node type from the appropriate MDAST source node(s)

4. **Render** -- `crates/satteri-ast/src/hast/render.rs`
   - Add a match arm in `render_node()` to serialize this node type to HTML

5. **Walk** -- `crates/satteri-ast/src/walk.rs`
   - Add to `serialize_hast_node_inline()` match

6. **Rebuild** -- `crates/satteri-ast/src/rebuild.rs`
   - Add to `remap_hast_string_refs()` if type_data contains StringRefs

7. **JS node builder** -- `crates/satteri-plugin-api/src/js_commands.rs`
   - Add to `name_to_hast_type()`, `encode_hast_js_node_data()`, and `apply_hast_set_property()` as needed

8. **TS types** -- `packages/satteri/src/types.ts`

9. **TS reader** -- `packages/satteri/src/hast/hast-reader.ts`
   - Add node type constant, add decoder method

10. **TS materializer** -- `packages/satteri/src/hast/hast-materializer.ts`
    - Add case in `materializeHastNode()`

11. **TS visitor** -- `packages/satteri/src/hast/hast-visitor.ts`
    - Add to `HastVisitorInstance`, subscription resolution, and match reading

12. **Tests** at each layer

---

## New Markdown Extension

Adding parser support for new syntax (e.g., a new pulldown-cmark extension).

1. **Option flag** -- `crates/satteri-pulldown-cmark/src/lib.rs`
   - Add a new bit to the `Options` bitflags

2. **Parser implementation** -- one or more of:
   - `src/firstpass.rs` for block-level constructs
   - `src/parse.rs` / `src/scanners.rs` for inline constructs
   - New dedicated module (e.g., `src/my_extension.rs`) for complex extensions

3. **Arena bridge** -- `crates/satteri-pulldown-cmark/src/arena_build.rs`
   - Map parser events to MDAST node type(s)
   - Usually requires a new MDAST node type (see that checklist)

4. **Feature toggle** -- `crates/satteri-napi-binding/src/lib.rs`
   - Add the option to `JsFeatures` struct and the options-building logic

5. **TS feature** -- `packages/satteri/src/compile.ts`
   - Add to `Features` interface and the features-to-options mapping

6. **Spec tests** -- `crates/satteri-pulldown-cmark/specs/`
   - Create a new spec file (or add to an existing one)
   - Add extension-specific test flag if needed (in `build.rs` `base_options_for_spec()`)

7. **Conformance tests** -- `packages/satteri/test/conformance/`
   - If a reference implementation exists (remark plugin), add conformance tests

---

## New JS API Function

Exposing a new function to JavaScript users.

1. **Rust implementation** -- `crates/satteri/src/lib.rs` or the relevant crate
   - Implement the core logic in Rust

2. **NAPI binding** -- `crates/satteri-napi-binding/src/lib.rs`
   - Add a `#[napi]` function that wraps the Rust implementation
   - Define JS-facing input/output types (use `#[napi(object)]` for structs)
   - Handle errors by converting `Result` to NAPI errors

3. **TS binding** -- `packages/satteri/src/binding.ts`
   - The NAPI-generated `index.js` auto-exports the function
   - Re-export it in `binding.ts` (and `binding.browser.ts` for WASM if applicable)

4. **TS wrapper** -- `packages/satteri/src/compile.ts` or a new file
   - Add a TypeScript wrapper function with proper types
   - Handle options, defaults, and type narrowing

5. **Public export** -- `packages/satteri/src/index.ts`
   - Export the function and its associated types

6. **Tests** -- `packages/satteri/test/`
   - Add integration tests exercising the new function

---

## New Rust Plugin Visitor Method

Adding a new typed visitor to the Rust `Plugin` trait.

1. **Typed node view** -- `crates/satteri-plugin-api/src/typed_nodes.rs`
   - Add a new struct (e.g., `MyNode<'a>`) with arena-backed accessors
   - Implement type-specific data decoding (call codec decode functions)

2. **Plugin trait** -- `crates/satteri-plugin-api/src/plugin.rs`
   - Add `visit_my_node(&mut self, node: &MyNode, ctx: &mut PluginContext) -> VisitResult` with a default no-op implementation

3. **Runner dispatch** -- `crates/satteri-plugin-api/src/runner.rs`
   - Add a match arm in `dispatch_visitor()` that constructs the typed node view and calls the visitor method

4. **Tests** -- `crates/satteri-plugin-api/tests/`
   - Add a test plugin that implements the new visitor and verify it receives the correct node data

---

## New Plugin Context Mutation Method

Adding a new way for plugins to mutate the AST (e.g., a new structural operation).

### Rust side

1. **Command variant** -- `crates/satteri-plugin-api/src/commands.rs`
   - Add a new variant to the `Command` enum

2. **Context method** -- `crates/satteri-plugin-api/src/context.rs`
   - Add a method to `PluginContext` that pushes the new command

3. **Runner patch conversion** -- `crates/satteri-plugin-api/src/runner.rs`
   - Add conversion from `Command` to `Patch` in `commands_to_patches()`

4. **Rebuild handling** -- `crates/satteri-ast/src/rebuild.rs`
   - Add a `Patch` variant if needed, and handle it in `copy_node()`

### JS side

5. **Command buffer encoding** -- `packages/satteri/src/command-buffer.ts`
   - Add a command byte constant (must not collide with existing ones)
   - Add an encoding method to `CommandBuffer` class

6. **JS command parsing** -- `crates/satteri-plugin-api/src/js_commands.rs`
   - Add the command byte constant (must match TS)
   - Add parsing logic in the command dispatch loop

7. **Visitor context** -- `packages/satteri/src/mdast/mdast-visitor.ts` and/or `src/hast/hast-visitor.ts`
   - Add the method to `MdastVisitorContext` / `HastVisitorContextImpl`

8. **Tests** -- JS plugin test + Rust command test

---

## New HAST Element Property Handling

If a new HTML/SVG attribute needs special handling in rendering.

1. **Property mapping** -- `crates/satteri-ast/src/hast/properties.rs`
   - Add the property to the appropriate resolution path in `property_to_attribute()`
   - If it's an SVG attribute, add to `svg_attribute_for()`
   - If it's a new HTML boolean attribute, add to the known-property lists

2. **Render handling** -- `crates/satteri-ast/src/hast/render.rs`
   - The renderer already handles property types generically (string, bool, space-sep, etc.)
   - Special handling is only needed for new value type semantics

3. **Tests** -- Unit test in `properties.rs`, render test in `crates/satteri-ast/tests/html.rs`
