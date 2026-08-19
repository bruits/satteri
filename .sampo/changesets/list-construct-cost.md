---
cargo/satteri-ast: patch
cargo/satteri-pulldown-cmark: patch
npm/satteri: patch
---

Improved Markdown parsing and rendering speed on list-heavy documents. Tight lists no longer slow down quadratically with item count — a 1000-item list renders about 4.5x faster — and line-dense documents parse roughly 6-9% faster overall.
