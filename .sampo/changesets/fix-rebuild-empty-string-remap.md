---
cargo/satteri-ast: patch
npm/satteri: patch
---

Fixes a crash when a plugin replaces a node with a tree containing an empty text node in a document that has non-ASCII characters (e.g. `é`).
