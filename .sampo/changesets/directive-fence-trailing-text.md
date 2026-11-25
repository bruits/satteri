---
npm/satteri: patch
cargo/satteri-pulldown-cmark: patch
---

Fixed container directive fence lines that have text after them: the line no longer closes or opens a directive and its text is no longer dropped, matching `remark-directive`.