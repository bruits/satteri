---
cargo/satteri-ast: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed `rawHtml` losing the SVG attribute schema for raw HTML inside a JSX `<svg>` element, so `fill-rule` now maps to `fillRule` instead of passing through as an unknown property.
