---
cargo/satteri-ast: minor
cargo/satteri-plugin-api: minor
npm/satteri: minor
---

Plugin mutations now compile to a binary op-stream, the single structural encoding — the legacy JSON node-tree path has been removed entirely, making structural edits (replace, insert, wrap, append) substantially faster. Node layouts shared between Rust and TypeScript are generated from a single registry. Declarative content that can't be encoded (a bare `root`/`doctype` handed in as replacement content, or an out-of-range numeric field) now throws a clear error rather than silently falling back; the `{raw}`/`{rawHtml}` escape hatches are unaffected.

Breaking change for Rust consumers of `satteri-ast`: the `JsNode`, `JsNodeAttribute`, and `JsNodeAttributes` types and `shared::encode_js_jsx_attrs` are removed along with the JSON path. The in-memory Rust plugin API (`PluginRunner`, `NodeBuilder`) is unaffected.

Visitor passes got faster across the board: walk decoding avoids per-string native calls, matched nodes defer child decoding, and `node.children` now returns lazy stubs that only materialize when a field is read — plugins that pass children through (e.g. replace-keeping-children) skip the arena snapshot entirely. Plugin-built nodes passed to mutation methods now throw a clear error instead of silently corrupting the tree, and a node retained past its visitor pass fails loudly on first access instead of reading stale data.

Breaking change in the low-level `MdastReader` API: the per-type getters `getHeadingDepth`, `getLinkData`, `getImageData`, `getCodeData`, `getMathData`, `getDefinitionData`, `getReferenceData`, `getImageReferenceData`, `getFootnoteDefinitionData`, and `getExpressionValue` were removed. Materialized nodes now carry these fields directly (see `materializeMdastTree`); use `getTypeData` for raw access.

Behavior change: `ctx.setProperty` with an array value on an MDX JSX element now space-joins the entries, matching how list-valued properties like `className` are encoded on regular elements. It previously comma-joined them.

Fix: a plugin-built replacement node now reads back with no `position` (it has no source range), instead of an occasional degenerate `{ line: 0, column: 0 }` position with a non-zero offset.
