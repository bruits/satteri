---
cargo/satteri-pulldown-cmark: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed a `{` inside an MDX link destination or title raising a parse error when the link tail spans more than one line, as in `[a](/u\n"ti{tle")`.
