---
cargo/satteri-pulldown-cmark: patch
cargo/satteri-ast: patch
npm/satteri: patch
---

Fixed several CommonMark and GFM parsing differences: `*` emphasis next to `*` or `_` now follows the flanking rules (`*_*a` stays text), a task list marker followed by a CRLF is recognized, a tab after a list marker that opens indented code no longer yields a task item, `![^1]` followed by an unresolved `[` keeps the footnote reference, a footnote definition that starts inside another one nests in it as in remark-gfm, a footnote's backref stays in its last paragraph when a definition follows it, and a literal autolink's domain ends at Unicode punctuation, so `www.—_→_` links `www.—_→`.
