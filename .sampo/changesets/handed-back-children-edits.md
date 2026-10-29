---
npm/satteri: patch
cargo/satteri-ast: patch
---

Fixed edits made to a node's children being ignored when a plugin returned that node with its children, as in `return { ...node, data }`.
