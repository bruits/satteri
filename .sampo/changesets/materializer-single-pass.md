---
npm/satteri: patch
---

Improved `markdownToMdast` and `markdownToHast` performance by reading each node from the wire buffer once while building the tree.
