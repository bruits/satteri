---
cargo/satteri-ast: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed React-cased SVG property names like `strokeLinecap` and `strokeLinejoin` leaking into HTML output as-is instead of serializing as `stroke-linecap` / `stroke-linejoin`. Attribute-name lookups are now case-insensitive, so every React casing of a known SVG attribute converts.
