---
npm/satteri: patch
cargo/satteri-ast: patch
cargo/satteri-plugin-api: patch
---

Fixed a child removed with `ctx.removeNode` coming back when its parent was returned as a copy, as in `return { ...node, data }`.
