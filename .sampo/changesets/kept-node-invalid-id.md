---
cargo/satteri-ast: patch
cargo/satteri-plugin-api: patch
npm/satteri: patch
---

Fixed edits to a node kept from a previous compile silently changing an unrelated node instead of failing with `invalid node id`.
