---
cargo/satteri-ast: patch
cargo/satteri-plugin-api: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed nodes created from raw string splices reporting garbage positions; they now report no position, like other plugin-created nodes.
