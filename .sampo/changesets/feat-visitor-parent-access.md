---
npm/satteri: patch
---

Add `ctx.parent(node)` and `ctx.indexOf(node)` to the MDAST and HAST plugin visitor contexts.

`parent()` returns a node's parent (or `undefined` at the root) and is climbable to reach any ancestor;

`indexOf()` returns a node's position within its parent's children. Together they make it possible to do operations depending on ancestry and siblings.
