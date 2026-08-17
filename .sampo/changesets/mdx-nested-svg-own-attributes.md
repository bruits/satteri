---
cargo/satteri-mdxjs: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed `elementAttributeNameCase: "html"` leaving a nested `<svg>` element's own React-cased attributes (like `strokeWidth`) unconverted on the MDX compile path; the SVG schema now covers the `<svg>` element itself, not just its descendants.
