---
npm/satteri: minor
cargo/satteri-ast: patch
cargo/satteri-plugin-api: patch
cargo/satteri-napi: patch
---

Added support for user-defined MDAST node types. A plugin can create a node with any `type` string; it round-trips through the pipeline and, mirroring `mdast-util-to-hast`'s default handler, works as either shape:

- a **parent** with `children` renders to an element via `data.hName` (defaulting to `<div>`), with `data.hProperties` merged in and its children rendered;
- a **leaf** with a `value` (and no `children` or `data.h*`) renders to an HTML text node.

Create one from any visitor:

```js
ctx.replaceNode(node, {
  type: "section",
  data: { hName: "section" },
  children: node.children,
});
```

Subscribe to every user-defined node with the `custom` visitor key and discriminate on `node.type`:

```js
const inspect = defineMdastPlugin({
  name: "inspect",
  custom(node) {
    if (node.type === "section") {
      /* ... */
    }
  },
});
```
