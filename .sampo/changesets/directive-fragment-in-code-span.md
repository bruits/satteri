---
cargo/satteri-pulldown-cmark: patch
npm/satteri: patch
---

Fixes inline code being mangled when it contains directive-like syntax. With directives enabled, writing something like `` `:foo[` `` followed by more inline code no longer merges the two code spans or drops a backtick — a `:` inside a code span is now treated as literal text, so you can safely show directive syntax in code.
