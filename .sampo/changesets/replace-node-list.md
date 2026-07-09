---
npm/satteri: minor
---

`ctx.replaceNode(node, newNode)` now accepts an array of nodes as well as a single node, matching `insertBefore`, `insertAfter`, `prependChild`, `appendChild` and `insertChildAt`. The nodes take the target's place in order, so `ctx.replaceNode(node, [a, b])` leaves `a` and `b` where `node` was. This works on both the MDAST and HAST visitor contexts.

Passing an empty array throws. The other list-taking methods read `[]` as "insert nothing", so letting it mean "delete the target" here would make one empty array delete content while five ignore it. Call `ctx.removeNode(node)` to drop a node.
