---
cargo/satteri-plugin-api: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed raw MDAST content adding paragraph wrappers in phrasing-content mutations. Raw block content is rejected in phrasing slots instead of creating invalid trees.
