---
npm/satteri: patch
---

Changed inserting an existing node, or listing it in a replacement, to move it instead of copying it, so a node is only ever in one place; insert `structuredClone(node)` for a copy.
