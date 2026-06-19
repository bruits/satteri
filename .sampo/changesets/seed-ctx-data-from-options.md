---
npm/satteri: patch
---

Adds a `data` option to `markdownToHtml`, `mdxToJs`, and `CompileOptions` that seeds the document data bag before plugins run. The same object is surfaced to plugins as `ctx.data` and returned as `result.data`, so values can be passed both into and out of a compile.
