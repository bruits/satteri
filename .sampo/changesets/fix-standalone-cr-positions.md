---
cargo/satteri-arena: patch
cargo/satteri-ast: patch
cargo/satteri-pulldown-cmark: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed documents that use standalone carriage returns (`\r`) as line endings parsing differently from documents that use `\n`. Values such as inline code and definition titles now keep the document's own line endings instead of always reporting `\n`.
