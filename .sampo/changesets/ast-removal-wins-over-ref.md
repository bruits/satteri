---
cargo/satteri-ast: patch
---

Changed patch application so a node removed in the same batch stays removed when content replacing one of its ancestors, or setting an ancestor's children, refers to it.
