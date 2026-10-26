---
cargo/satteri-pulldown-cmark: patch
npm/satteri: patch
---

Fixed a brace inside a quoted attribute value, such as `{title="a{b"}`, stopping the attributes from applying to a fenced code block, paragraph or table cell.
