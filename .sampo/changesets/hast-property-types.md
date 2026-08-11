---
npm/satteri: patch
---

Improved HAST property types in plugins: `node.properties.href`, `className`, `start` and every other known property are now typed individually, so reading one no longer needs a `typeof` guard to narrow it.

```ts
element: {
  filter: ["a"],
  visit(node, ctx) {
    if (node.properties.href?.startsWith("http")) {
      // ...
    }
  },
}
```
