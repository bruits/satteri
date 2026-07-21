---
cargo/satteri-ast: patch
npm/satteri: patch
---

Fixes footnotes being ignored inside directives. A footnote reference nested in a rendered directive (e.g. `:::note … [^id] … :::`) now works like anywhere else — it renders as a footnote link and its definition appears in the footnotes section — instead of being left as literal `[^id]` text.
