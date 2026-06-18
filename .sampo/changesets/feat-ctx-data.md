---
npm/satteri: patch
---

Add `ctx.data`, a document-scoped data bag shared across every plugin in the compile.

Writes from one plugin are visible to later plugins, and the bag persists across the mdast→hast boundary, so hast plugins can read what mdast plugins wrote. After compilation the final state is returned on `result.data`. The bag lives entirely on the JS side, so any value is allowed (functions, class instances, `Map`/`Set`) and references are preserved, much like `vfile.data`. Specific keys can be typed by augmenting the `DataMap` interface via `declare module "satteri"`.
