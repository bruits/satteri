---
cargo/satteri-ast: patch
npm/satteri: patch
---

Fixed `rawHtml` dropping the original source and root span. Markdown-derived nodes and self-contained raw blocks now retain their positions when their source association is unambiguous.
