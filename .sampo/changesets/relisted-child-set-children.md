---
npm/satteri: patch
cargo/satteri-ast: patch
---

Fixed a child removed and then listed again with `ctx.setProperty(node, "children", ...)` being lost instead of moved.
