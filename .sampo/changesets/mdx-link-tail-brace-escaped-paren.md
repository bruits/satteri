---
cargo/satteri-pulldown-cmark: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed a `{` inside an MDX link destination or title raising a parse error when the tail also holds an escaped or quoted `)`, as in `[a](\){)`.
