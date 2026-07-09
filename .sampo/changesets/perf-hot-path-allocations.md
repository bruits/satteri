---
cargo/satteri-arena: minor
cargo/satteri-pulldown-cmark: patch
npm/satteri: patch
---

Improves parsing performance by right-sizing hot buffers from measured ratios, merging adjacent text nodes in place in the string pool, deriving code-point offsets from already-computed line/column data, and replacing scalar line-start scans with SIMD searches. Instruction counts drop ~10% on Markdown parsing, ~7.5% on Markdown to HTML, and ~2.4% on MDX compilation.
