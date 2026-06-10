---
cargo/satteri-ast: minor
cargo/satteri-plugin-api: minor
npm/satteri: minor
---

Plugin mutations now compile to a binary op-stream instead of JSON, making structural edits (replace, insert, wrap, append) substantially faster. Node layouts shared between Rust and TypeScript are generated from a single registry.

Visitor passes got faster across the board: walk decoding avoids per-string native calls, matched nodes defer child decoding, and `node.children` now returns lazy stubs that only materialize when a field is read — plugins that pass children through (e.g. replace-keeping-children) skip the arena snapshot entirely. Plugin-built nodes passed to mutation methods now throw a clear error instead of silently corrupting the tree, and a node retained past its visitor pass fails loudly on first access instead of reading stale data.

Breaking change in the low-level `MdastReader` API: the per-type getters `getHeadingDepth`, `getLinkData`, `getImageData`, `getCodeData`, `getMathData`, `getDefinitionData`, `getReferenceData`, `getImageReferenceData`, `getFootnoteDefinitionData`, and `getExpressionValue` were removed. Materialized nodes now carry these fields directly (see `materializeMdastTree`); use `getTypeData` for raw access.

Behavior change: `ctx.setProperty` with an array value on an MDX JSX element now space-joins the entries, matching how list-valued properties like `className` are encoded on regular elements. It previously comma-joined them.
