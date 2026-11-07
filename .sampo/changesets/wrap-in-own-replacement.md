---
npm/satteri: patch
cargo/satteri-ast: patch
---

Fixed a compile error when a visitor wraps a node in a replacement that names it, or surrounds it with raw tag halves, while the same plugin edits something inside that node.
