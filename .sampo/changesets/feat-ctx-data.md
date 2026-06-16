---
npm/satteri: patch
---

Add `ctx.data`, a document-scoped data bag shared across every plugin in the compile.

Writes from one plugin are visible to later plugins, and the bag persists across the mdast→hast boundary, so hast plugins can read what mdast plugins wrote. After compilation the final state is returned on `result.data` (or `null` if nothing was written). Values must be JSON-serializable, as the bag round-trips through Rust between plugin passes.
