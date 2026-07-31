---
cargo/satteri-arena: patch
cargo/satteri-ast: patch
cargo/satteri-pulldown-cmark: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed `position` line and column numbers in documents containing standalone carriage returns (`\r`); other standalone-`\r` parsing is unchanged.
