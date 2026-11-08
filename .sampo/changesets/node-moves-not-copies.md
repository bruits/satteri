---
npm/satteri: patch
---

Changed inserting an existing node, or listing it in a replacement, to move it instead of copying it, and listing a node twice, inserting it next to or into itself, or passing a node from the tree as a `wrapNode` wrapper to throw; insert `structuredClone(node)` for a copy.
