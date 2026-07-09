---
cargo/satteri-napi: minor
npm/satteri: patch
---

Reduces plugin-path overhead: child stubs install their lazy fields with per-key `defineProperty` (the batched `Object.defineProperties` costs nearly double per stub), and the HAST-plugin path parses, extracts frontmatter, and converts to HAST in a single native call via the new `createHastHandleWithFrontmatter`/`createMdxHastHandleWithFrontmatter` bindings.
