---
name: fix-satteri-bug
description: "Workflow for diagnosing and fixing bugs in Satteri. Use when a test is failing, the user reports incorrect output (wrong HTML, wrong AST, wrong JS compilation), a plugin is not behaving correctly, there is a parsing issue, a conformance failure, a panic, or any other bug in the Satteri Markdown/MDX pipeline. Also use when investigating regressions or when conformance tests diverge from the remark/rehype reference implementations."
---

# Fix Satteri Bug

A staged workflow for diagnosing and fixing bugs in the Satteri Markdown/MDX pipeline. If you are not already familiar with the codebase, load the `navigate-satteri` skill first.

## Stage 1: Classify the Bug Layer

Before writing any code, determine which layer of the pipeline the bug is in. This narrows the search space and determines where to add the reproduction test.

### Decision tree

1. **Is the markdown source parsed incorrectly?** (wrong AST shape, missing nodes, wrong positions)
   - **Layer:** Parser (`crates/satteri-pulldown-cmark/`)
   - **Key files:** `src/arena_build.rs` (event-to-arena bridge), `src/parse.rs` (core parser), `src/firstpass.rs` (block-level pass), `src/mdx.rs` (MDX-specific parsing)
   - **Test location:** Add spec test in `specs/*.txt` or Rust integration test in `tests/`

2. **Is the MDAST correct but HAST conversion is wrong?** (wrong element mapping, missing attributes, footnote numbering)
   - **Layer:** Conversion (`crates/satteri-ast/src/convert.rs`)
   - **Key file:** `convert.rs` (2228 lines, handles all node type conversions)
   - **Test location:** JS conformance test in `test/conformance/hast.test.ts` or Rust unit test in `convert.rs`

3. **Is the HAST correct but HTML output is wrong?** (missing attributes, wrong escaping, void element issues)
   - **Layer:** Renderer (`crates/satteri-ast/src/hast/render.rs`)
   - **Key files:** `render.rs` (HTML serialization), `properties.rs` (property-to-attribute mapping)
   - **Test location:** JS conformance test in `test/conformance/html.test.ts` or Rust test in `crates/satteri-ast/tests/html.rs`

4. **Is the HAST correct but MDX compilation is wrong?** (wrong JS output, missing imports, broken JSX)
   - **Layer:** MDX compiler (`crates/satteri-mdxjs-rs/`)
   - **Key files:** `hast_util_to_oxc.rs`, `mdx_plugin_recma_document.rs`, `mdx_plugin_recma_jsx_rewrite.rs`
   - **Test location:** Rust test in `crates/satteri-mdxjs-rs/tests/test.rs` or JS conformance in `test/conformance/mdx.test.ts`

5. **Does a plugin visitor receive wrong data?** (wrong node properties, missing children, incorrect tag names)
   - **Layer:** Walker (`crates/satteri-ast/src/walk.rs`) or TS reader (`src/mdast/mdast-reader.ts`, `src/hast/hast-reader.ts`)
   - The walk binary format serializes node data inline -- a codec mismatch between the Rust serializer and TS reader is a common failure mode
   - **Test location:** JS visitor tests in `test/visitor.test.ts` or `test/hast-visitor.test.ts`

6. **Do plugin mutations not take effect?** (setProperty ignored, replaceNode not working, insertBefore in wrong position)
   - **Layer:** Command buffer (`src/command-buffer.ts` <-> `crates/satteri-plugin-api/src/js_commands.rs`) or rebuild (`crates/satteri-ast/src/rebuild.rs`)
   - Check that the wire format constants match between TS and Rust
   - **Test location:** JS plugin tests in `test/html-plugin.test.ts` or Rust tests in `js_commands.rs`

7. **Does the bug only occur through NAPI?** (works in pure Rust but fails from JS)
   - **Layer:** NAPI boundary (`crates/satteri-napi-binding/src/lib.rs`)
   - Common issues: handle lifetime problems, wrong buffer conversion, missing Mutex lock
   - **Test location:** JS tests

8. **Is there a codec mismatch between encoding and decoding?** (data corruption, wrong field values)
   - **Layer:** Codecs (`satteri-ast/src/mdast/codec.rs` or `hast/codec.rs`)
   - The binary type_data format is defined by the encode function and must be read identically by: Rust decode functions, `walk.rs` serialization, TS readers, `rebuild.rs` StringRef remapping, and `js_commands.rs` set-property handling
   - **Test location:** Unit tests in the codec file + integration tests

## Stage 2: Reproduce with a Test

Write a failing test before attempting a fix. The test type depends on the layer identified in Stage 1.

For detailed test formats, locations, and commands, read `.agents/skills/navigate-satteri/references/test-guide.md`.

### Quick reference

- **Parsing bug:** Add a spec test case in the appropriate `specs/*.txt` file. Use the 32-backtick fence format with markdown input and expected HTML separated by `.`.
- **Conversion/rendering bug:** Add a test case in `crates/satteri-ast/tests/html.rs` using `mdast_to_html()`, or a JS conformance test comparing against remark/rehype.
- **Plugin bug:** Add a test in `test/html-plugin.test.ts` or `test/visitor.test.ts` that exercises the specific mutation.
- **MDX bug:** Add a test in `crates/satteri-mdxjs-rs/tests/test.rs` (Rust) or `test/conformance/mdx.test.ts` (JS).
- **Conformance regression:** Add a case in the appropriate `test/conformance/*.test.ts` file.

Run only the relevant test subset first to confirm the failure before proceeding.

## Stage 3: Diagnose

### Trace the data flow

Follow the pipeline trace for the relevant layer. For a `markdownToHtml` call:

```
source -> pulldown-cmark parser -> arena_build.rs -> Arena<Mdast>
       -> MDAST plugin walk/dispatch/rebuild cycle
       -> convert.rs -> Arena<Hast>
       -> HAST plugin walk/dispatch/rebuild cycle
       -> hast/render.rs -> HTML string
```

Read the specific files at the layer where the bug occurs. Use `cargo test` with `--nocapture` to see debug output, or add temporary `eprintln!` statements in the Rust code.

### Common failure patterns

**Codec mismatches:**
The most common cross-layer bug. A field is encoded with one byte layout but decoded with a different assumption. Check that:
- `mdast/codec.rs` `encode_*` and `decode_*` functions agree on field order and sizes
- `walk.rs` `serialize_mdast_node_inline()` / `serialize_hast_node_inline()` reads type_data at the correct offsets
- `mdast-reader.ts` / `hast-reader.ts` decode functions read the same offsets
- `rebuild.rs` `remap_mdast_string_refs()` / `remap_hast_string_refs()` knows the correct StringRef positions
- `js_commands.rs` `apply_mdast_set_property()` / `apply_hast_set_property()` modifies the correct bytes

**StringRef offset errors:**
When `rebuild.rs` splices sub-arenas, all StringRefs in the sub-arena must be remapped (source strings are concatenated). If a new node type's type_data contains StringRefs that `remap_*_string_refs()` doesn't know about, the resolved strings will point to wrong bytes.

**Children range off-by-one:**
`ArenaNode::children_start` and `children_count` index into the flat `Arena::children` array. An off-by-one here can cause nodes to appear as children of the wrong parent or be missing entirely.

**Missing node type dispatch:**
When a new node type is added, every switch/match on `MdastNodeType` or `HastNodeType` needs a new arm. Forgetting one in `walk.rs`, `rebuild.rs`, `js_commands.rs`, or the TS visitor files causes the node to be silently skipped or fall through to a default case.

**Property type tag mismatch:**
HAST element properties use type tags (`PROP_STRING=0`, `PROP_BOOL_TRUE=1`, etc.) defined in `shared.rs`. The same constants must be used in `hast/codec.rs`, `hast/render.rs`, `hast-reader.ts`, `command-buffer.ts`, and `js_commands.rs`. A mismatch causes property values to be misinterpreted.

**Command buffer wire format:**
The command byte constants (`CMD_REMOVE=0x01`, `CMD_REPLACE=0x0B`, etc.) and payload type constants (`PAYLOAD_RAW_MARKDOWN=0x10`, `PAYLOAD_SERDE_JSON=0x12`, etc.) must match exactly between `command-buffer.ts` and `js_commands.rs`. A mismatch causes command parsing failures or wrong mutations.

## Stage 4: Fix

### Apply the fix

Fix the root cause in the identified layer. Keep changes minimal and focused.

### The MDAST/HAST mirror check

After fixing, check whether the same issue exists in the parallel layer. The MDAST and HAST codepaths share structural patterns:

| If you fixed something in... | Check the same pattern in... |
|-----|------|
| `mdast/codec.rs` | `hast/codec.rs` |
| `mdast-reader.ts` | `hast-reader.ts` |
| `mdast-materializer.ts` | `hast-materializer.ts` |
| `mdast-visitor.ts` | `hast-visitor.ts` |
| `walk.rs` `serialize_mdast_*` | `walk.rs` `serialize_hast_*` |
| `rebuild.rs` `remap_mdast_*` | `rebuild.rs` `remap_hast_*` |
| `js_commands.rs` `apply_mdast_*` | `js_commands.rs` `apply_hast_*` |

### Cross-layer consistency

If the fix changes a binary format (type_data layout, walk result format, command buffer encoding), verify that all consumers of that format are updated:

**Type data changes:** `codec.rs` (encode + decode) + `walk.rs` (inline serialization) + reader TS (decode) + `rebuild.rs` (StringRef remapping) + `js_commands.rs` (set-property)

**Walk result changes:** `walk.rs` (Rust serializer) + visitor TS (JS reader in `readMdastMatchedNode` / `readMatchedNode`)

**Command buffer changes:** `command-buffer.ts` (JS encoder) + `js_commands.rs` (Rust parser)

### Quality checklist

From CONTRIBUTING.md:
- Self-documenting code. Comments explain _why_, not _how_.
- Tests assert observable behavior, not implementation details.
- Rust: typed error enums, `?` propagation, `.expect()`/`.unwrap()` only for programmer bugs.
- TypeScript: strict types, no `any`.
- Clarity over cleverness. Small focused functions. Avoid duplication.

## Stage 5: Verify

Run the verification sequence appropriate to the scope of the change. For test commands and scope-based test selection, read `.agents/skills/navigate-satteri/references/test-guide.md`.

Minimum verification:
```sh
cargo clippy --all --all-targets
cargo fmt --all --check
cargo test --all
cd packages/satteri && pnpm build && pnpm test
```

Confirm:
1. The originally failing test now passes
2. No other tests have regressed
3. Clippy reports no new warnings
4. Code is formatted correctly
