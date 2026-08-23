---
cargo/satteri-ast: patch
npm/satteri: patch
---

Fixes `rawHtml` dropping every node's position, so nodes that came from Markdown keep their source positions like `rehype-raw`, and fixes MDX component overrides being ignored when `rawHtml` is enabled.
