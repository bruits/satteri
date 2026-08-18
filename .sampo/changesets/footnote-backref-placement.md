---
cargo/satteri-ast: patch
npm/satteri: patch
---

Fixed the footnote back-reference link landing inside an earlier paragraph when the definition ends in a list, code block, blockquote, table, or heading. It is now appended after that last block, as remark and GitHub do.
