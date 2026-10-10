---
cargo/satteri-pulldown-cmark: patch
npm/satteri: patch
---

Fixed several CommonMark and GFM parsing differences: `*` emphasis next to `*` or `_` now follows the flanking rules (`*_*a` stays text), a task list marker followed by a CRLF is recognized, a tab after a list marker that opens indented code no longer yields a task item, and `![^1]` followed by an unresolved `[` keeps the footnote reference.
