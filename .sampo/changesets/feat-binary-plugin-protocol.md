---
cargo/satteri-ast: minor
cargo/satteri-plugin-api: minor
npm/satteri: minor
---

Plugin mutations now compile to a binary op-stream instead of JSON, making structural edits (replace, insert, wrap, append) substantially faster. Node layouts shared between Rust and TypeScript are generated from a single registry.

Breaking change in the low-level `MdastReader` API: the per-type getters `getHeadingDepth`, `getLinkData`, `getImageData`, `getCodeData`, `getMathData`, `getDefinitionData`, `getReferenceData`, `getImageReferenceData`, `getFootnoteDefinitionData`, and `getExpressionValue` were removed. Materialized nodes now carry these fields directly (see `materializeMdastTree`); use `getTypeData` for raw access.

Behavior change: `ctx.setProperty` with an array value on an MDX JSX element now space-joins the entries, matching how list-valued properties like `className` are encoded on regular elements. It previously comma-joined them.
