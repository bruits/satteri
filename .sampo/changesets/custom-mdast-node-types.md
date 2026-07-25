---
npm/satteri: minor
cargo/satteri-ast: patch
cargo/satteri-plugin-api: patch
cargo/satteri-napi: patch
---

Adds user-defined MDAST node types. A plugin can create a node with any `type` string, render it as an element through `data.hName` (or as text from a `value`), and reach every one of them from the new `custom` visitor key. Content nested inside a custom node stays visible to other plugins and to the HTML output.
