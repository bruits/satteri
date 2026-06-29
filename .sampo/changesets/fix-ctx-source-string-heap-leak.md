---
cargo/satteri-arena: patch
cargo/satteri-ast: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixes plugin `ctx.source` being polluted with duplicated, concatenated content appended after the original document.
