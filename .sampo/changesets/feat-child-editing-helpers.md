---
npm/satteri: patch
---

Adds `ctx.insertChildAt(node, index, child)` and `ctx.removeChildAt(node, index)` for editing a node's children by position.

`insertBefore`, `insertAfter`, `prependChild`, `appendChild`, and `insertChildAt` now also accept an array of nodes, so you can insert several at once.
