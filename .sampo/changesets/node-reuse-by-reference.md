---
npm/satteri: minor
cargo/satteri-ast: patch
---

A node read from the tree and handed back to `insertBefore`, `insertAfter`, `prependChild`, `appendChild`, `insertChildAt` or `replaceNode`, or returned from a visitor, is now that node rather than a snapshot of how it looked when you read it, so it arrives carrying every change the pass made to it. This is how reused children already behaved when nested inside new content, and it means an inserted node no longer silently misses a transform another visitor queued on it in the same pass. Pass `structuredClone(node)` when you want a detached copy instead.

Two shapes have no answer under that rule and now report an error at the call that caused them, instead of quietly inserting a stale copy: inserting a node inside itself, as `insertAfter(node, ctx.parent(node))` does, and swapping or sorting siblings with a pair of inserts that each name the other's node. Reorder by handing the parent the order you want, with `setProperty(parent, "children", [...])`.
