---
npm/satteri: minor
---

`ctx.replaceNode(node, newNode)` now accepts an array of nodes as well as a single node, matching `insertBefore`, `insertAfter`, `prependChild`, `appendChild` and `insertChildAt`. The nodes take the target's place in order, so `ctx.replaceNode(node, [a, b])` leaves `a` and `b` where `node` was. This works on both the MDAST and HAST visitor contexts.
