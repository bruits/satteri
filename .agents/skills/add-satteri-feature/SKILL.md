---
name: add-satteri-feature
description: "Workflow for adding features to Satteri. Use when adding a new node type (MDAST or HAST), a new Markdown or MDX extension, a new plugin capability or visitor method, a new context mutation method, a new public API function, or any feature that spans multiple crates or the Rust-to-JS boundary. Also use when the user asks how to extend the Satteri pipeline, add support for new syntax, or expose new functionality to JavaScript."
---

# Add Satteri Feature

A checklist-driven workflow for adding features to the Satteri Markdown/MDX pipeline. Features in this codebase typically span multiple crates and the Rust/TypeScript boundary, so a systematic approach prevents missed steps.

If you are not already familiar with the codebase, load the `navigate-satteri` skill first. For test guidance, read `.agents/skills/navigate-satteri/references/test-guide.md`.

## Step 1: Scope the Change

Before writing any code, identify which layers of the pipeline are affected.

### Questions to answer

1. **Does this change the parser?** If it adds new syntax (e.g., a new Markdown extension), the pulldown-cmark fork needs modification.
2. **Does this add or change a node type?** If yes, the change spans the full stack: enum variant, codec, arena build, conversion, walk, rebuild, commands, TS types, TS reader, TS materializer, TS visitor, plugin definition, tests.
3. **Does this cross the Rust/JS boundary?** If a new function needs to be callable from JS, it needs a NAPI binding and a TS wrapper.
4. **Does this affect both MDAST and HAST?** Many features (new node types, new visitor methods, codec changes) require parallel work in both MDAST and HAST layers.
5. **Does this change a binary format?** If type_data layout, walk result format, or command buffer encoding changes, every consumer of that format must be updated.

### Implementation order

Work from the bottom of the stack up:

```
1. Rust types and codecs (satteri-arena, satteri-ast)
2. Parser changes (satteri-pulldown-cmark) if applicable
3. Conversion (convert.rs) if the feature affects MDAST-to-HAST
4. Walk/rebuild/commands (satteri-ast, satteri-plugin-api) if plugins interact with it
5. NAPI binding (satteri-napi-binding) if JS needs access
6. TypeScript types, reader, materializer, visitor
7. Public API surface (index.ts) if user-facing
8. Tests at each layer
```

This order ensures that dependencies are satisfied as you go. Rust types first, TS last.

## Step 2: Choose the Right Checklist

Detailed per-feature-type checklists are in `.agents/skills/add-satteri-feature/references/checklists.md`. Read the one that matches your feature type:

- **New MDAST node type** -- Full stack from enum to TS visitor (15 steps)
- **New HAST node type** -- Similar but without parser/conversion steps
- **New Markdown extension** -- Parser options, implementation, arena bridge, spec tests
- **New JS API function** -- Rust implementation, NAPI binding, TS wrapper, export
- **New Rust plugin visitor method** -- Plugin trait, typed node view, runner dispatch
- **New plugin context mutation method** -- Command enum, buffer encoding/decoding, rebuild handling
- **New HAST element property handling** -- Property mapping, render changes, reader changes

## Step 3: Implement

Work through the checklist for your feature type. Key principles:

### Match existing patterns

Before writing new code, read an existing example of the same kind of change. For example:
- Adding a new MDAST node type? Read how `Math` (type 27) or `ContainerDirective` (type 30) is implemented across the stack.
- Adding a new NAPI function? Read how `text_content_handle` is exposed: Rust implementation, `#[napi]` attribute, TS binding re-export.
- Adding a new visitor method? Read how `heading` is handled: trait method in `plugin.rs`, dispatch in `runner.rs`, typed node in `typed_nodes.rs`.

### The MDAST/HAST symmetry rule

If your feature adds something for MDAST, check whether a parallel addition is needed for HAST (and vice versa). This applies to:
- Node type enum variants (`mdast/node.rs` <-> `hast/node.rs`)
- Codec encode/decode functions (`mdast/codec.rs` <-> `hast/codec.rs`)
- Reader decoder methods (`mdast-reader.ts` <-> `hast-reader.ts`)
- Materializer type handlers (`mdast-materializer.ts` <-> `hast-materializer.ts`)
- Visitor interfaces and dispatch (`mdast-visitor.ts` <-> `hast-visitor.ts`)
- Walk serialization (`walk.rs` serialize_mdast_* <-> serialize_hast_*)
- Rebuild StringRef remapping (`rebuild.rs` remap_mdast_* <-> remap_hast_*)
- Command handling (`js_commands.rs` apply_mdast_* <-> apply_hast_*)

### Binary format consistency

When adding a new node type or changing type_data layout, these consumers must all agree on the format:

| Consumer | File | What it does |
|----------|------|-------------|
| Encoder | `mdast/codec.rs` or `hast/codec.rs` | Writes type_data bytes |
| Decoder | Same codec file | Reads type_data bytes |
| Walk serializer | `walk.rs` | Reads type_data for inline serialization |
| TS reader | `mdast-reader.ts` or `hast-reader.ts` | Reads binary buffer |
| StringRef remapper | `rebuild.rs` | Finds and offsets StringRefs in type_data |
| Set-property handler | `js_commands.rs` | Modifies type_data in-place |
| Node builder | `js_commands.rs` | Creates type_data from JsNode JSON |

### Do not add external dependencies without justification

Prefer the standard library and existing utilities. The project is deliberately minimal in dependencies. If a new dependency is genuinely needed, explain why existing utilities cannot cover the use case.

## Step 4: Test

Add tests at each layer affected by the feature. For test formats, patterns, and commands, read `.agents/skills/navigate-satteri/references/test-guide.md`.

### Test types by feature

| Feature type | Required tests |
|-------------|---------------|
| New MDAST node type | Spec tests + codec roundtrip + conversion test + JS conformance |
| New HAST node type | Codec roundtrip + render test + JS conformance |
| New Markdown extension | Spec tests + conformance tests + error/edge cases |
| New JS API function | JS integration test |
| New plugin capability | JS plugin/visitor test + Rust plugin test |
| New context method | JS plugin test showing the mutation takes effect in HTML output |

### Conformance testing

If the feature has a reference implementation in the remark/rehype ecosystem, add conformance tests that compare Satteri's output against the reference. Use the patterns in `test/conformance/helpers.ts`.

## Step 5: Verify

Run the full verification sequence:

```sh
cargo clippy --all --all-targets
cargo fmt --all --check
cargo test --all
cd packages/satteri && pnpm build && pnpm test
```

Additionally check:
- New public API is exported from `packages/satteri/src/index.ts`
- TypeScript compiles without errors
- No clippy warnings introduced
- Code is formatted correctly
