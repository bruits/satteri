---
cargo/satteri-pulldown-cmark: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed a `{` inside an MDX link destination or title raising a parse error when the tail holds an escaped or quoted `)`, as in `[a](\){)`, and stopped a `[` that is backslash-escaped or inside a code span from opening a link tail.
