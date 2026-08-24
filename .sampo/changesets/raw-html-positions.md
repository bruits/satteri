---
cargo/satteri-ast: patch
npm/satteri: patch
---

Fixes `rawHtml` dropping node positions, so nodes that came from Markdown keep their source positions like `rehype-raw` does.
