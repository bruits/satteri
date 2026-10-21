---
cargo/satteri-pulldown-cmark: minor
npm/satteri: minor
---

Added blocks to the `attributes` feature: a `{...}` ending a paragraph or table cell goes on that block, or on the list item, list or blockquote around it, and a paragraph holding only one goes on the list, table or thematic break before it, as markdown-it-attrs places them.
