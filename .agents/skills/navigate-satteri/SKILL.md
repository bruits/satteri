---
name: navigate-satteri
description: "Codebase navigation map for the Satteri monorepo. Use this skill when exploring the codebase, tracing how a feature flows through the pipeline, finding where something is defined or implemented, understanding the relationship between Rust crates and TypeScript packages, or when you need to know which files to read for a given area of the code. Also load this skill before working on bugs or features if you are not already familiar with the project structure."
---

# Navigate Satteri

Satteri is a Rust + TypeScript monorepo for high-performance Markdown/MDX processing. The core pipeline runs in Rust with an arena-allocated binary AST. JavaScript gets read-only views of the AST and sends mutations back as binary command buffers. This architecture means changes often span multiple crates and the TypeScript layer, and understanding the data flow is essential before modifying anything.

## Crate Map

Each crate has a specific role in the pipeline. The relationships form a DAG with `satteri-arena` at the bottom and `satteri-napi-binding` at the top.

### `satteri-arena` (crates/satteri-arena/)

The foundation. Defines the arena allocator, node layout, and binary wire format.

| File | Role |
|------|------|
| `node.rs` | `ArenaNode` (52-byte `#[repr(C)]` struct) and `StringRef` (8-byte offset+len into source). These are the atoms of the entire system. |
| `arena.rs` | `Arena<K>` -- the main container. Holds `nodes: Vec<ArenaNode>`, `children: Vec<u32>` (flat child array), `type_data: Vec<u8>` (packed per-node data), `source: String`, `node_data: FxHashMap<u32, Vec<u8>>` (plugin data blobs). |
| `builder.rs` | `ArenaBuilder<K>` -- SAX-style incremental builder with open/close stack. Used by parsers and the conversion pass. |
| `kind.rs` | Phantom type markers `Mdast` and `Hast` with `ArenaKind` trait. Prevents cross-kind arena misuse at compile time. |
| `raw_buffer.rs` | `Arena::to_raw_buffer()` -- serializes to flat bytes for NAPI transfer. 44-byte header + nodes + children + type_data + source. |
| `codec.rs` | `StringRef` encode/decode helpers for type_data. |
| `line_index.rs` | Byte offset to (line, column) mapping using `memchr`. |
| `mdx_types.rs` | MDX-specific types (`Point`, `Position`, `MdxSignal`, JSX attribute types). Used by the MDX compiler. |

### `satteri-ast` (crates/satteri-ast/)

Node types, codecs, conversion, rendering, tree walking, and arena reconstruction.

| File | Role |
|------|------|
| `mdast/node.rs` | `MdastNodeType` enum -- 33 variants (Root through MdxjsEsm). |
| `mdast/codec.rs` | Binary encode/decode for every MDAST node's `type_data`. Each node type has a `#[repr(C)]` data struct (e.g., `HeadingData { depth: u8 }`, `LinkData { url: StringRef, title: StringRef }`). 28 encode/decode function pairs. |
| `hast/node.rs` | `HastNodeType` enum -- 11 variants (Root through MdxTextExpression). |
| `hast/codec.rs` | Binary encode/decode for HAST `type_data`. Element layout: 16-byte header (tag + prop_count) + 20 bytes per property (name + type + value). |
| `hast/properties.rs` | JS property name to HTML/SVG attribute mapping (`className` -> `class`, ARIA, data-*, SVG). Port of `property-information`. |
| `hast/render.rs` | `hast_arena_to_html()` -- HAST arena to HTML string. Handles void elements, raw text elements, entity escaping, SVG context. |
| `convert.rs` | `mdast_arena_to_hast_arena()` -- the MDAST-to-HAST conversion (2228 lines). Port of `mdast-util-to-hast` / `remark-rehype`. Handles reference definitions, footnotes, GFM tables, `hName`/`hProperties`/`hChildren` overrides. |
| `rebuild.rs` | `rebuild()` -- applies `Patch` operations to produce a new arena. 7 patch types: Replace, Remove, InsertBefore, InsertAfter, Wrap, PrependChild, AppendChild. Includes `StringRef` remapping when splicing sub-arenas. |
| `walk.rs` | `walk_mdast()` / `walk_hast()` -- subscription-based DFS walker. Returns a flat binary buffer of matched nodes with inline-resolved type-specific data for zero-copy JS consumption. |
| `commands.rs` | `JsNode` (serde-deserializable struct for JS-originated nodes), `JsNodeAttribute`, `CommandError`. |
| `shared.rs` | Property type constants (`PROP_STRING`, `PROP_BOOL_TRUE`, etc.) and MDX attribute kind constants. Used by both MDAST and HAST codecs. |
| `text_content.rs` | Generic text extraction parameterized by arena kind. |

### `satteri-pulldown-cmark` (crates/satteri-pulldown-cmark/)

Vendored fork of pulldown-cmark with MDX extension support.

| File | Role |
|------|------|
| `lib.rs` | `Options` bitflags, `Event` enum, `Parser` type. The public parsing API. |
| `arena_build.rs` | `parse()` -- bridges pulldown-cmark events into an `Arena<Mdast>` (3270 lines). This is where markdown source becomes a tree. |
| `parse.rs` | Core parser (`ParserInner`). |
| `firstpass.rs` | Block-level first pass. |
| `mdx.rs` | MDX JSX/expression parsing. |
| `scanners.rs` | Inline scanning and pattern matching. |
| `specs/` | 10 spec test files (table, footnotes, math, etc.). |
| `build.rs` | Generates Rust test files from spec `.txt` files. |

**Guardrail:** This crate intentionally diverges from upstream pulldown-cmark. Do not "update" it to match upstream without explicit instruction.

### `satteri-mdxjs-rs` (crates/satteri-mdxjs-rs/)

MDX-to-JavaScript compiler. Fork of mdxjs-rs, adapted for OXC.

| File | Role |
|------|------|
| `lib.rs` | `compile()` and `compile_hast_arena()` entry points. |
| `hast_util_to_oxc.rs` | HAST arena to OXC AST conversion. |
| `mdx_plugin_recma_document.rs` | MDX document structure transform. |
| `mdx_plugin_recma_jsx_rewrite.rs` | JSX to function call rewriting. |
| `oxc_util_build_jsx.rs` | JSX AST building utilities. |
| `configuration.rs` | `Options`, `JsxRuntime`, `OutputFormat`. |

### `satteri-plugin-api` (crates/satteri-plugin-api/)

The Rust plugin system.

| File | Role |
|------|------|
| `plugin.rs` | `Plugin` trait with 14 typed visitor methods, `VisitResult` enum, `NodeView`. |
| `runner.rs` | `PluginRunner` -- walks arena, dispatches to visitors, converts results to patches, rebuilds. |
| `commands.rs` | `Command` enum (Replace, Remove, Insert*, Wrap, *Child, SetData), `NewNode`, `NodeBuilder`. |
| `js_commands.rs` | Binary command buffer parser for JS-to-Rust mutations (1642 lines). `apply_mdast_commands()` and `apply_hast_commands()`. The wire format constants here must match `command-buffer.ts`. |
| `context.rs` | `PluginContext` -- arena access, data maps, mutation methods, diagnostics. |
| `typed_nodes.rs` | Zero-copy typed views: `Heading`, `Text`, `Link`, `Image`, `Code`, `Paragraph`. |
| `data.rs` | `DataMap` (untyped key-value) and `TypedDataMap` (Rust-only typed storage). |

### `satteri-napi-binding` (crates/satteri-napi-binding/)

The NAPI boundary. Every function callable from JavaScript is defined in `src/lib.rs` (651 lines). 24 `#[napi]` functions including: `create_mdast_handle`, `walk_mdast_handle`, `apply_commands_to_mdast_handle`, `convert_mdast_to_hast_handle`, `walk_handle`, `apply_commands_to_handle`, `render_handle`, `compile_handle`, `serialize_handle`, `drop_handle`, etc.

Arenas never leave Rust memory -- only opaque `External<Mutex<Arena>>` handles cross NAPI. Binary data crosses in two directions:
- **Rust to JS:** `serialize_handle` / `walk_*_handle` produce `Uint8Array` buffers
- **JS to Rust:** `apply_commands_to_*_handle` accepts `Uint8Array` command buffers

### `satteri` (crates/satteri/)

Thin facade. `markdown_to_html()` and `compile_mdx()` for pure-Rust usage. 20 lines.

## TypeScript Layer (packages/satteri/)

| File | Role |
|------|------|
| `src/index.ts` | Public API surface. All exports go through here. |
| `src/compile.ts` | Pipeline orchestrator (577 lines). `markdownToHtml`, `mdxToJs`, `evaluate`, `markdownToMdast`, etc. Wires together parsing, plugins, conversion, rendering. Conditional sync/async return types based on plugin signatures. |
| `src/plugin.ts` | `defineMdastPlugin` / `defineHastPlugin` factory functions. |
| `src/types.ts` | `MdastNode`, `HastNode`, `BufferHeader`, module augmentation for custom node types. |
| `src/mdx-types.ts` | MDX JSX node type definitions (avoids transitive deps). |
| `src/directive-types.ts` | Directive node type definitions. |
| `src/command-buffer.ts` | `CommandBuffer` class -- binary encoder for JS-to-Rust mutations (247 lines). Constants must match `js_commands.rs`. |
| `src/lazy-props.ts` | `lazyProp` / `lazyGroup` utilities for deferred property resolution on materialized nodes. |
| `src/binding.ts` | Re-exports NAPI functions (Node.js target). |
| `src/binding.browser.ts` | Re-exports WASI functions (browser target). |
| `src/mdast/mdast-reader.ts` | `MdastReader` -- decodes binary MDAST wire format. 33 node types, type-specific decoders. |
| `src/mdast/mdast-materializer.ts` | `materializeMdastTree()` -- binary arena to lazy JS objects. |
| `src/mdast/mdast-visitor.ts` | MDAST plugin visitor pipeline (792 lines). `MdastVisitorContext`, `resolveMdastSubscriptions`, `visitMdastHandle`. |
| `src/hast/hast-reader.ts` | `HastReader` -- decodes binary HAST wire format. 11 node types. |
| `src/hast/hast-materializer.ts` | `materializeHastTree()` -- binary arena to lazy JS objects. |
| `src/hast/hast-visitor.ts` | HAST plugin visitor pipeline (792 lines). Filtered visitors for elements. `WalkElement` with prototype-based lazy getters. |

## Pipeline Trace: `markdownToHtml`

This is the full data flow for a `markdownToHtml(source, { mdastPlugins, hastPlugins })` call:

```
1. createMdMdastHandle(source, features)
   TS: compile.ts -> NAPI: create_mdast_handle()
   Rust: pulldown-cmark parser -> arena_build.rs -> Arena<Mdast>
   Returns: opaque MdastHandle (External<Mutex<Arena<Mdast>>>)

2. For each MDAST plugin (sequential):
   a. resolveMdastSubscriptions(plugin) -> [{nodeType, visitFn}]
      TS: mdast-visitor.ts (maps method names to node type numbers)
   
   b. walkMdastHandle(handle, subscriptions) -> Uint8Array
      NAPI -> Rust: walk.rs::walk_mdast() (DFS, subscription filter, inline serialization)
   
   c. For each matched node in the binary buffer:
      TS: readMdastMatchedNode() decodes inline binary data
      TS: dispatches to visitor function
      TS: classifies return value -> CommandBuffer mutations
   
   d. Merge context commands + return-value commands -> single Uint8Array
   
   e. applyCommandsToMdastHandle(handle, buffer)
      NAPI -> Rust: js_commands.rs::apply_mdast_commands()
      -> SET_PROPERTY: mutates type_data in-place
      -> Structural commands: collected as Patch objects
      -> rebuild.rs::rebuild() produces new Arena<Mdast>

3. readFrontmatter(handle) -> extract YAML/TOML from MDAST
   NAPI: get_mdast_frontmatter()

4. convertMdastToHastHandle(handle)
   NAPI -> Rust: convert.rs::mdast_arena_to_hast_arena()
   Consumes MdastHandle, returns HastHandle (External<Mutex<Arena<Hast>>>)

5. For each HAST plugin (same walk/dispatch/apply cycle as step 2):
   resolveSubscriptions() includes tag filters for element visitors
   walkHandle() filters by tag name in Rust (only matching elements cross NAPI)
   HastVisitorContextImpl handles element property mutations

6. renderHandle(handle)
   NAPI -> Rust: hast/render.rs::hast_arena_to_html()
   Returns: HTML string

7. dropHandle(handle) -- releases Rust memory
```

For `mdxToJs`, step 6 becomes `compileHandle()` which calls `satteri-mdxjs-rs::compile_hast_arena()` (HAST -> OXC AST -> JS string).

## MDAST/HAST Parallel Structure

These file pairs mirror each other. When modifying one side, check whether the same change applies to the other:

| MDAST | HAST |
|-------|------|
| `satteri-ast/src/mdast/node.rs` | `satteri-ast/src/hast/node.rs` |
| `satteri-ast/src/mdast/codec.rs` | `satteri-ast/src/hast/codec.rs` |
| `satteri-ast/src/mdast/mod.rs` | `satteri-ast/src/hast/mod.rs` |
| `src/mdast/mdast-reader.ts` | `src/hast/hast-reader.ts` |
| `src/mdast/mdast-materializer.ts` | `src/hast/hast-materializer.ts` |
| `src/mdast/mdast-visitor.ts` | `src/hast/hast-visitor.ts` |

The walk, rebuild, and js_commands modules are generic or branched internally -- they handle both kinds via `ArenaKind` or explicit match arms.

## Guardrails

From AGENTS.md -- these apply to all agents:

- Do not create new documentation files to explain implementation.
- Do not add external dependencies without justification. Prefer the standard library and existing utilities.
- Match the current project structure, naming, and style; do not create parallel patterns.
- All code, comments, documentation, commit messages, and user-facing output must be in English.
- The vendored `pulldown-cmark` intentionally diverges from upstream -- do not "update" to match upstream without explicit instruction.
- If an optimization or pattern is applied for MDAST, it should most likely also be applied for HAST, unless there is a specific reason not to.

## References

For deeper information:
- Read `.agents/skills/navigate-satteri/references/architecture.md` for file-by-file breakdowns, binary format layouts, and key data structures.
- Read `.agents/skills/navigate-satteri/references/test-guide.md` for all testing information: test locations, how to add tests, spec test format, conformance patterns, and commands.
