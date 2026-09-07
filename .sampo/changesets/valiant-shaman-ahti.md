---
cargo/satteri-ast: patch
cargo/satteri-mdxjs: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed SVG script and style text being corrupted and elements with HTML void-element names losing children or absorbing siblings during HTML serialization and rawHtml reparsing. HTML content inside SVG integration points keeps its normal serialization rules, including with optimizeStatic.
