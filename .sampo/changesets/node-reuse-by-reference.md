---
npm/satteri: patch
cargo/satteri-ast: patch
---

Fixed a node passed to `insertBefore`, `insertAfter`, `prependChild`, `appendChild` or `insertChildAt` losing changes other visitors made to it in the same pass, and fixed content disappearing when a plugin moved a node by inserting it elsewhere and then removing it. Two shapes that used to work by silently inserting a stale copy now report an error at the call instead: inserting a node into a position it contains, and reordering siblings by two inserts that each reuse the other's node, which is written as `setProperty(parent, "children", [...])`.
