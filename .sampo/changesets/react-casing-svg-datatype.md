---
cargo/satteri-property-info: patch
cargo/satteri-ast: patch
cargo/satteri-mdxjs: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed the default `elementAttributeNameCase: "react"` compiling the SVG `datatype` attribute to `data-type`. It now emits `datatype`, while custom `data-*` attributes on SVG and HTML elements are still kebab-cased.
