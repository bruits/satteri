---
npm/satteri: minor
cargo/satteri-ast: patch
cargo/satteri-plugin-api: patch
cargo/satteri-napi: patch
---

Added support for user-defined MDAST node types. A plugin can create a node with any `type` string; it round-trips through the pipeline and renders to HTML via `data.hName` (defaulting to `<div>`), with `data.hProperties` merged in and its children rendered. Subscribe to every user-defined node with the `custom` visitor key and discriminate on `node.type`.

```js
ctx.replaceNode(node, {
  type: "section",
  data: { hName: "section" },
  children: node.children,
});
```
