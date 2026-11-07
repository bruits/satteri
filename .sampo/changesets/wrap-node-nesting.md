---
npm/satteri: patch
---

Changed `wrapNode` to wrap whatever replaced the node, and calling it twice on one node to nest both wrappers instead of keeping only the last.
