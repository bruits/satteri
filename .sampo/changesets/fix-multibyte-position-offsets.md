---
cargo/satteri-arena: patch
cargo/satteri-ast: patch
cargo/satteri-pulldown-cmark: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed `position` offsets and columns for documents with multibyte unicode characters. Node positions handed to plugin visitors reported byte offsets, so `ctx.source.slice(start.offset, end.offset)` drifted right of the node whenever multibyte characters preceded it. Offsets and columns now count UTF-16 code units — the unit JS strings index by — so they slice `ctx.source` correctly and match remark everywhere, including astral characters like emoji.
