---
cargo/satteri-pulldown-cmark: patch
npm/satteri: patch
---

With directives enabled, a `:`-directive fragment inside a code span — like `` `:foo[` `` — is now treated as literal code. Previously the directive was recognized and its label scan reached past the code span's backticks, collapsing a following inline code span into the first one and dropping a backtick. Code spans now correctly bind tighter than text directives.
